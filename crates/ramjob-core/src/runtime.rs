//! One daemon pipeline tick (M2).

use std::collections::HashMap;
use std::time::Instant;

use crate::config::RamjobConfig;
use crate::diagnostics::DiagnosticsRing;
use crate::enforcer::{
    ExclusionPolicy, LiveTrimHooks, TRIM_RATE_LIMIT, TrimContext,
};
use crate::fsm::{FsmAction, GroupFsm, GroupFsmInput, TRIM_TARGET_RATIO};
use crate::gate::{run_gate_on_group, GateMeasurement, GATE_SETTLE};
use crate::grouper::AppGroup;
use crate::policy::{PolicyState, SystemArm};
use crate::pressure::PressureSource;

pub struct Runtime {
    pub config: RamjobConfig,
    pub policy: PolicyState,
    pub groups: HashMap<String, GroupFsm>,
    pub rates: HashMap<String, Instant>,
    pub diagnostics: DiagnosticsRing,
    path_cache: crate::scanner::PathCache,
}

pub struct TickOutcome {
    pub system: SystemArm,
    pub trims_attempted: usize,
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
    let target = (TRIM_TARGET_RATIO * cap_bytes as f64) as u64;
    let trim_was_ineffective = measurement.gf1 > target;
    PostTrimObservation {
        ry_live: measurement.ry_live,
        gf_after: measurement.gf1,
        refault_hot,
        trim_was_ineffective,
    }
}

impl Runtime {
    pub fn from_config(config: RamjobConfig) -> Self {
        Self {
            config,
            policy: PolicyState::new(),
            groups: HashMap::new(),
            rates: HashMap::new(),
            diagnostics: DiagnosticsRing::new(),
            path_cache: crate::scanner::PathCache::new(),
        }
    }

    pub fn force_arm_for_test(&mut self) {
        self.policy.arm = SystemArm::Armed;
    }

    pub fn tick<P: PressureSource>(
        &mut self,
        pressure: &mut P,
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
        self.tick_with_groups(system, &apps, now)
    }

    pub fn tick_with_groups(
        &mut self,
        system: SystemArm,
        apps: &[AppGroup],
        now: Instant,
    ) -> Result<TickOutcome, String> {
        let by_key: HashMap<&str, &AppGroup> =
            apps.iter().map(|g| (g.group_key.as_str(), g)).collect();
        let mut trims_attempted = 0usize;
        let runaway = self.config.runaway_multiplier;

        for gc in &self.config.groups {
            let Some(app) = by_key.get(gc.key.as_str()) else {
                continue;
            };
            let gf = crate::accountant::group_footprint(app);
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

            let action = fsm.step(input);
            match action {
                FsmAction::None => {}
                FsmAction::RecordWouldBackstop => {
                    self.diagnostics
                        .push(format!("{} WouldBackstop gf={gf}", gc.key));
                }
                FsmAction::SoftTrim => {
                    if let Some(last) = self.rates.get(&gc.key) {
                        if now.duration_since(*last) < TRIM_RATE_LIMIT {
                            self.diagnostics.push(format!(
                                "{} SoftTrim skipped: group rate-limited (no trim)",
                                gc.key
                            ));
                            continue;
                        }
                    }
                    match measured_soft_trim(app, &mut self.rates, now) {
                        Ok(measurement) => {
                            trims_attempted += 1;
                            self.rates.insert(gc.key.clone(), now);
                            let post = apply_post_trim(&measurement, gc.cap_bytes);
                            let fsm = self.groups.get_mut(&gc.key).unwrap();
                            let follow = fsm.observe_post_trim(GroupFsmInput {
                                gf: post.gf_after,
                                cap_bytes: gc.cap_bytes,
                                system,
                                always_enforce: gc.always_enforce,
                                runaway_multiplier: runaway,
                                now,
                                last_ry_live: post.ry_live,
                                refault_hot: post.refault_hot,
                                trim_was_ineffective: post.trim_was_ineffective,
                            });
                            self.diagnostics.push(format!(
                                "{} SoftTrim ry_live={:?} gf1={} refault={} ineffective={} follow={follow:?} phase={:?}",
                                gc.key,
                                post.ry_live,
                                post.gf_after,
                                post.refault_hot,
                                post.trim_was_ineffective,
                                fsm.phase
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

        Ok(TickOutcome {
            system,
            trims_attempted,
        })
    }
}

/// Measured soft-trim via the shared gate §2.3 owner (`run_gate_on_group`).
fn measured_soft_trim(
    group: &AppGroup,
    rate_limits: &mut HashMap<String, Instant>,
    now: Instant,
) -> Result<GateMeasurement, String> {
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
    use crate::grouper::GroupMember;
    use crate::pressure::SimulatedPressure;

    fn app(key: &str, gf: u64) -> AppGroup {
        AppGroup {
            group_key: key.into(),
            members: vec![GroupMember {
                pid: 1,
                create_time: 1,
                private_working_set_bytes: gf,
            }],
        }
    }

    #[test]
    fn rate_limit_skips_second_trim() {
        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            groups: vec![GroupConfig {
                key: "hog".into(),
                cap_bytes: 100,
                always_enforce: false,
            }],
        };
        let mut rt = Runtime::from_config(cfg);
        let now = Instant::now();
        rt.rates.insert("hog".into(), now);
        let apps = vec![app("hog", 500)];
        let out = rt
            .tick_with_groups(SystemArm::Armed, &apps, now + Duration::from_secs(1))
            .unwrap();
        assert_eq!(out.trims_attempted, 0);
    }

    #[test]
    fn always_enforce_fsm_wants_soft_trim() {
        let mut rt = Runtime::from_config(RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            groups: vec![GroupConfig {
                key: "hog".into(),
                cap_bytes: 100,
                always_enforce: true,
            }],
        });
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
}
