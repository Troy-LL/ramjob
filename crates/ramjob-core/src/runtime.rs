//! One daemon pipeline tick (M2).

use std::collections::HashMap;
use std::time::Instant;

use crate::accountant::group_footprint;
use crate::config::RamjobConfig;
use crate::diagnostics::DiagnosticsRing;
use crate::enforcer::{
    ExclusionPolicy, LiveTrimHooks, TRIM_RATE_LIMIT, TrimContext,
};
use crate::adaptive::hottest_phase;
use crate::fsm::{FsmAction, GroupFsm, GroupFsmInput, GroupPhase};
use crate::gate::{run_gate_on_group, GateMeasurement, GATE_SETTLE};
use crate::grouper::AppGroup;
use crate::job_backstop::JobBackstopStore;
use crate::policy::{PolicyState, SystemArm};
use crate::pressure::PressureSource;

/// No `config` field here — the caller owns the single authoritative
/// `RamjobConfig` (CLI: a local `cfg`; app: `AppStateInner.panel.config`)
/// and passes it into `tick`/`tick_with_groups` each call, so there is
/// never a second copy to drift out of sync.
pub struct Runtime {
    pub policy: PolicyState,
    pub groups: HashMap<String, GroupFsm>,
    pub rates: HashMap<String, Instant>,
    pub diagnostics: DiagnosticsRing,
    path_cache: crate::scanner::PathCache,
    pub(crate) commit_ratios: HashMap<String, crate::commit_ratio::CommitRatio>,
    pub(crate) backstop: JobBackstopStore,
    pub(crate) last_caps: HashMap<String, u64>,
}

pub struct TickOutcome {
    pub system: SystemArm,
    pub trims_attempted: usize,
    /// Process groups from this tick's enumeration (panel snapshot input).
    pub apps: Vec<AppGroup>,
}

/// Post-trim observation derived from a gate measurement (SPEC §2.3 + §4.2).
struct PostTrimObservation {
    ry_live: Option<f64>,
    gf_after: u64,
    refault_hot: bool,
    trim_was_ineffective: bool,
}

fn apply_post_trim(measurement: &GateMeasurement, cap_bytes: u64) -> PostTrimObservation {
    let refault_hot =
        measurement.gf0 > 0 && measurement.gf1 as f64 >= 0.9 * measurement.gf0 as f64;
    let target = (crate::fsm::TRIM_TARGET_RATIO * cap_bytes as f64) as u64;
    let trim_was_ineffective = measurement.gf1 > target;
    PostTrimObservation {
        ry_live: measurement.ry_live,
        gf_after: measurement.gf1,
        refault_hot,
        trim_was_ineffective,
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            policy: PolicyState::new(),
            groups: HashMap::new(),
            rates: HashMap::new(),
            diagnostics: DiagnosticsRing::new(),
            path_cache: crate::scanner::PathCache::new(),
            commit_ratios: HashMap::new(),
            backstop: JobBackstopStore::new(),
            last_caps: HashMap::new(),
        }
    }

    /// Test-only constructor with injectable Job Object store.
    #[cfg(test)]
    pub fn new_with_backstop_store(backstop: JobBackstopStore) -> Self {
        Self {
            policy: PolicyState::new(),
            groups: HashMap::new(),
            rates: HashMap::new(),
            diagnostics: DiagnosticsRing::new(),
            path_cache: crate::scanner::PathCache::new(),
            commit_ratios: HashMap::new(),
            backstop,
            last_caps: HashMap::new(),
        }
    }

    /// Test-only read access to the backstop store.
    #[cfg(test)]
    pub fn backstop_store(&self) -> &JobBackstopStore {
        &self.backstop
    }

    pub fn force_arm_for_test(&mut self) {
        self.policy.arm = SystemArm::Armed;
    }

    /// Max FSM phase across configured groups (for adaptive sleep).
    pub fn hottest_group_phase(&self) -> Option<GroupPhase> {
        hottest_phase(self.groups.values().map(|g| g.phase))
    }

    /// Whether any Job Object backstop limit is armed.
    pub fn backstop_active(&self) -> bool {
        self.backstop.any_limited()
    }

    pub fn tick(
        &mut self,
        config: &RamjobConfig,
        pressure: &mut dyn PressureSource,
        now: Instant,
    ) -> Result<TickOutcome, String> {
        let mut sample = pressure.sample()?;
        sample.now = now;
        let system = self.policy.update(sample);
        self.diagnostics
            .push(format!("system={system:?}"));

        let procs = crate::scanner::enumerate_processes_with_cache(&mut self.path_cache)
            .map_err(|s| format!("enumerate: {s:?}"))?;
        let apps = crate::grouper::group_processes(&procs);
        self.tick_with_groups(config, system, &apps, now)
    }

    pub fn tick_with_groups(
        &mut self,
        config: &RamjobConfig,
        system: SystemArm,
        apps: &[AppGroup],
        now: Instant,
    ) -> Result<TickOutcome, String> {
        if config.pause_all {
            self.diagnostics.push("pause_all".to_string());
            return Ok(TickOutcome {
                system,
                trims_attempted: 0,
                apps: apps.to_vec(),
            });
        }

        self.clear_backstop_on_disarm(system);

        let by_key: HashMap<&str, &AppGroup> =
            apps.iter().map(|g| (g.group_key.as_str(), g)).collect();
        let mut trims_attempted = 0usize;
        let runaway = config.runaway_multiplier;

        for gc in &config.groups {
            let Some(app) = by_key.get(gc.key.as_str()) else {
                continue;
            };
            let gf = group_footprint(app);
            let action = {
                let fsm = self.groups.entry(gc.key.clone()).or_default();
                let input = GroupFsmInput {
                    gf,
                    cap_bytes: gc.cap_bytes,
                    system,
                    always_enforce: gc.always_enforce,
                    runaway_multiplier: runaway,
                    now,
                    last_ry_live: None,
                    refault_hot: false,
                    trim_was_ineffective: false,
                };
                fsm.step(input)
            };

            self.sample_commit_ratio_if_pressure(&gc.key, app, gf);

            let mut want_backstop = matches!(action, FsmAction::Backstop);

            match action {
                FsmAction::None => {}
                FsmAction::Backstop => {}
                FsmAction::SoftTrim => {
                    let rate_limited = self
                        .rates
                        .get(&gc.key)
                        .is_some_and(|last| now.duration_since(*last) < TRIM_RATE_LIMIT);
                    if rate_limited {
                        self.diagnostics.push(format!(
                            "{} SoftTrim skipped: group rate-limited (no trim)",
                            gc.key
                        ));
                    } else {
                        match measured_soft_trim(app, &mut self.rates, now) {
                            Ok(measurement) => {
                                trims_attempted += 1;
                                self.rates.insert(gc.key.clone(), now);
                                let post = apply_post_trim(&measurement, gc.cap_bytes);
                                let follow = {
                                    let fsm = self.groups.get_mut(&gc.key).unwrap();
                                    fsm.observe_post_trim(GroupFsmInput {
                                        gf: post.gf_after,
                                        cap_bytes: gc.cap_bytes,
                                        system,
                                        always_enforce: gc.always_enforce,
                                        runaway_multiplier: runaway,
                                        now,
                                        last_ry_live: post.ry_live,
                                        refault_hot: post.refault_hot,
                                        trim_was_ineffective: post.trim_was_ineffective,
                                    })
                                };
                                want_backstop |= follow == FsmAction::Backstop;
                                let phase = self.groups.get(&gc.key).unwrap().phase;
                                self.diagnostics.push(format!(
                                    "{} SoftTrim ry_live={:?} gf1={} refault={} ineffective={} follow={follow:?} phase={phase:?}",
                                    gc.key,
                                    post.ry_live,
                                    post.gf_after,
                                    post.refault_hot,
                                    post.trim_was_ineffective,
                                ));
                            }
                            Err(e) => {
                                self.diagnostics
                                    .push(format!("{} SoftTrim skipped: {e}", gc.key));
                            }
                        }
                    }
                }
            }

            if want_backstop {
                self.arm_backstop_if_ready(gc, app);
            }

            self.track_cap_change(gc, app);
        }

        Ok(TickOutcome {
            system,
            trims_attempted,
            apps: apps.to_vec(),
        })
    }
}

#[cfg(test)]
mod test_support {
    use std::cell::RefCell;

    use crate::gate::GateMeasurement;

    thread_local! {
        static TRIM_STUB: RefCell<Option<Result<GateMeasurement, String>>> = const {
            RefCell::new(None)
        };
    }

    pub fn set_trim_measurement_stub(m: Result<GateMeasurement, String>) {
        TRIM_STUB.with(|c| *c.borrow_mut() = Some(m));
    }

    pub fn take_trim_measurement_stub() -> Option<Result<GateMeasurement, String>> {
        TRIM_STUB.with(|c| c.borrow_mut().take())
    }
}

/// Inject a gate measurement for the next `tick_with_groups` SoftTrim (unit tests only).
#[cfg(test)]
pub fn set_trim_measurement_stub(m: Result<GateMeasurement, String>) {
    test_support::set_trim_measurement_stub(m);
}

/// Measured soft-trim via the shared gate §2.3 owner (`run_gate_on_group`).
fn measured_soft_trim(
    group: &AppGroup,
    rate_limits: &mut HashMap<String, Instant>,
    now: Instant,
) -> Result<GateMeasurement, String> {
    #[cfg(test)]
    if let Some(stub) = test_support::take_trim_measurement_stub() {
        return stub;
    }

    let hooks = LiveTrimHooks;
    let mut ctx = TrimContext {
        hooks: &hooks,
        rate_limits,
        now,
        exclusion: ExclusionPolicy::ProtectInteractive,
    };
    run_gate_on_group(group, &mut ctx, GATE_SETTLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::config::GroupConfig;
    use crate::fsm::{FsmAction, GroupPhase};
    use crate::gate::{GateMeasurement, GATE_SETTLE};
    use crate::pressure::SimulatedPressure;

    #[test]
    fn rate_limit_skips_second_trim() {
        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![GroupConfig {
                key: "hog".into(),
                cap_bytes: 100,
                always_enforce: false,
            }],
            pause_all: false,
        };
        let mut rt = Runtime::new();
        let now = Instant::now();
        rt.rates.insert("hog".into(), now);
        let apps = vec![crate::grouper::AppGroup {
            group_key: "hog".into(),
            members: vec![crate::grouper::GroupMember {
                pid: 1,
                create_time: 1,
                private_working_set_bytes: 500,
                private_usage_bytes: 500,
            }],
        }];
        let out = rt
            .tick_with_groups(&cfg, SystemArm::Armed, &apps, now + Duration::from_secs(1))
            .unwrap();
        assert_eq!(out.trims_attempted, 0);
    }

    #[test]
    fn always_enforce_fsm_wants_soft_trim() {
        let mut rt = Runtime::new();
        let now = Instant::now();
        let fsm = rt.groups.entry("hog".into()).or_default();
        let action = fsm.step(GroupFsmInput {
            gf: 500,
            cap_bytes: 100,
            system: SystemArm::Disarmed,
            always_enforce: true,
            runaway_multiplier: 3.0,
            now,
            last_ry_live: None,
            refault_hot: false,
            trim_was_ineffective: false,
        });
        assert_eq!(action, FsmAction::SoftTrim);
        assert_eq!(fsm.phase, GroupPhase::Trim);
    }

    #[test]
    fn apply_post_trim_refault_and_ineffective() {
        let m = GateMeasurement {
            group_key: "g".into(),
            target_pids: vec![1],
            trimmed_pids: vec![1],
            excluded_pids: vec![],
            rate_limited: false,
            gf0: 1000,
            gf1: 950,
            available0: 0,
            available1: 0,
            cs0: Some(0),
            cs1: Some(0),
            ry_bench: None,
            ry_live: Some(0.5),
            verdict: None,
            settle: GATE_SETTLE,
            trim_errors: vec![],
        };
        let post = apply_post_trim(&m, 100);
        assert!(post.refault_hot);
        assert!(post.trim_was_ineffective);
    }

    #[test]
    fn simulated_pressure_sample_ok() {
        let mut p = SimulatedPressure {
            low_memory: false,
            high_memory: true,
            hard_faults_per_sec: 0.0,
        };
        assert!(p.sample().is_ok());
    }

    #[test]
    fn pressure_phase_samples_commit_ratio() {
        let cap = 1_000u64;
        let gf = 900u64;
        let commit = 1_800u64;
        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![GroupConfig {
                key: "hog".into(),
                cap_bytes: cap,
                always_enforce: true,
            }],
            pause_all: false,
        };
        let mut rt = Runtime::new();
        let apps = vec![crate::grouper::AppGroup {
            group_key: "hog".into(),
            members: vec![crate::grouper::GroupMember {
                pid: 1,
                create_time: 1,
                private_working_set_bytes: gf,
                private_usage_bytes: commit,
            }],
        }];
        rt.tick_with_groups(&cfg, SystemArm::Armed, &apps, Instant::now())
            .unwrap();
        let cr = rt.commit_ratios.get("hog").expect("sampled");
        assert_eq!(cr.samples(), 1);
        assert!((cr.ratio() - 2.0).abs() < 1e-9);
    }
}
