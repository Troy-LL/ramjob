//! One daemon pipeline tick (M2).

use std::collections::HashMap;
use std::time::Instant;

use crate::accountant::{group_commit_charge, group_footprint};
use crate::commit_ratio::{ratchet_limit, translate_job_limit, CommitRatio};
use crate::config::{GroupConfig, RamjobConfig};
use crate::diagnostics::DiagnosticsRing;
use crate::enforcer::{
    ExclusionPolicy, LiveTrimHooks, TRIM_RATE_LIMIT, TrimContext,
};
use crate::fsm::{FsmAction, GroupFsm, GroupFsmInput, GroupPhase, TRIM_TARGET_RATIO};
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
    commit_ratios: HashMap<String, CommitRatio>,
    backstop: JobBackstopStore,
    last_caps: HashMap<String, u64>,
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

    pub fn force_arm_for_test(&mut self) {
        self.policy.arm = SystemArm::Armed;
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

        if system == SystemArm::Disarmed && self.backstop.any_limited() {
            match self.backstop.clear_all_limits() {
                Ok(()) => self.diagnostics.push("BACKSTOP disarm".to_string()),
                Err(e) => self
                    .diagnostics
                    .push(format!("BACKSTOP disarm err: {}", e.0)),
            }
        }

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

            if matches!(
                self.groups.get(&gc.key).map(|f| f.phase),
                Some(GroupPhase::Pressure)
            ) {
                let commit = group_commit_charge(app);
                self.commit_ratios
                    .entry(gc.key.clone())
                    .or_default()
                    .sample(commit, gf);
            }

            match action {
                FsmAction::None => {}
                FsmAction::Backstop => {
                    self.arm_backstop_if_ready(gc, app, gf);
                }
                FsmAction::SoftTrim => {
                    if let Some(last) = self.rates.get(&gc.key) {
                        if now.duration_since(*last) < TRIM_RATE_LIMIT {
                            self.diagnostics.push(format!(
                                "{} SoftTrim skipped: group rate-limited (no trim)",
                                gc.key
                            ));
                            self.track_cap_change(gc, app);
                            continue;
                        }
                    }
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
                            if follow == FsmAction::Backstop {
                                self.arm_backstop_if_ready(gc, app, post.gf_after);
                            }
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

            self.track_cap_change(gc, app);
        }

        Ok(TickOutcome {
            system,
            trims_attempted,
            apps: apps.to_vec(),
        })
    }

    fn arm_backstop_if_ready(&mut self, gc: &GroupConfig, app: &AppGroup, _gf: u64) {
        if !gc.always_enforce {
            return;
        }
        let Some(cr) = self.commit_ratios.get(&gc.key) else {
            self.diagnostics
                .push(format!("{} BACKSTOP not ready samples=0", gc.key));
            return;
        };
        if !cr.ready() {
            self.diagnostics.push(format!(
                "{} BACKSTOP not ready samples={}",
                gc.key,
                cr.samples()
            ));
            return;
        }

        let commit = group_commit_charge(app);
        let ratio = cr.ratio();
        let already_limited = matches!(self.backstop.memory_limit(&gc.key), Some(Some(_)));
        let target = translate_job_limit(gc.cap_bytes, ratio);
        let limit = if already_limited {
            ratchet_limit(target, commit)
        } else {
            target
        };

        for member in &app.members {
            if let Err(e) = self.backstop.assign_pid(&gc.key, member.pid) {
                self.diagnostics.push(format!(
                    "{} BACKSTOP degrade nested pid={}: {}",
                    gc.key, member.pid, e.0
                ));
            }
        }

        match self.backstop.set_memory_limit(&gc.key, limit) {
            Ok(()) => self
                .diagnostics
                .push(format!("{} BACKSTOP arm limit={limit}", gc.key)),
            Err(e) => self.diagnostics.push(format!(
                "{} BACKSTOP degrade limit: {}",
                gc.key, e.0
            )),
        }
    }

    fn track_cap_change(&mut self, gc: &GroupConfig, app: &AppGroup) {
        let prev_cap = self.last_caps.get(&gc.key).copied();
        if let Some(prev) = prev_cap {
            if gc.cap_bytes < prev {
                if let Some(Some(current)) = self.backstop.memory_limit(&gc.key) {
                    if let Some(cr) = self.commit_ratios.get(&gc.key) {
                        if cr.ready() {
                            let commit = group_commit_charge(app);
                            let target = translate_job_limit(gc.cap_bytes, cr.ratio());
                            let limit = ratchet_limit(target, commit);
                            if limit < current {
                                match self.backstop.set_memory_limit(&gc.key, limit) {
                                    Ok(()) => self.diagnostics.push(format!(
                                        "{} BACKSTOP ratchet cap_decrease limit={limit}",
                                        gc.key
                                    )),
                                    Err(e) => self.diagnostics.push(format!(
                                        "{} BACKSTOP ratchet err: {}",
                                        gc.key, e.0
                                    )),
                                }
                            }
                        }
                    }
                }
            }
        }
        self.last_caps.insert(gc.key.clone(), gc.cap_bytes);
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
impl Runtime {
    fn with_backstop(backstop: JobBackstopStore) -> Self {
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

    fn backstop_store(&self) -> &JobBackstopStore {
        &self.backstop
    }

    fn arm_backstop_if_ready_for_test(
        &mut self,
        gc: &GroupConfig,
        app: &AppGroup,
        gf: u64,
    ) {
        self.arm_backstop_if_ready(gc, app, gf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::config::GroupConfig;
    use crate::fsm::{FsmAction, GroupPhase};
    use crate::grouper::GroupMember;
    use crate::job_backstop::{BackstopError, BackstopHooks, JobBackstopStore, JobHandle};
    use crate::pressure::SimulatedPressure;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_JOB_MEMORY;

    struct MockJob {
        memory_limit: Option<u64>,
        assigned: HashSet<u32>,
    }

    struct MockBackstopHooks {
        jobs: Arc<Mutex<HashMap<usize, MockJob>>>,
        next_id: Mutex<usize>,
        fail_assign_pids: HashSet<u32>,
    }

    impl MockBackstopHooks {
        fn new() -> Self {
            Self {
                jobs: Arc::new(Mutex::new(HashMap::new())),
                next_id: Mutex::new(1),
                fail_assign_pids: HashSet::new(),
            }
        }

        fn with_fail_assign(pid: u32) -> Self {
            let mut hooks = Self::new();
            hooks.fail_assign_pids.insert(pid);
            hooks
        }

        fn job_id(handle: &JobHandle) -> usize {
            handle.0 .0 as usize
        }
    }

    impl BackstopHooks for MockBackstopHooks {
        fn create_job(&self) -> Result<JobHandle, BackstopError> {
            let mut next = self.next_id.lock().unwrap();
            let id = *next;
            *next += 1;
            self.jobs.lock().unwrap().insert(
                id,
                MockJob {
                    memory_limit: None,
                    assigned: HashSet::new(),
                },
            );
            Ok(JobHandle(HANDLE(id as *mut core::ffi::c_void)))
        }

        fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError> {
            if self.fail_assign_pids.contains(&pid) {
                return Err(BackstopError(format!("nested job pid={pid}")));
            }
            let id = Self::job_id(job);
            self.jobs
                .lock()
                .unwrap()
                .get_mut(&id)
                .ok_or_else(|| BackstopError("unknown job".into()))?
                .assigned
                .insert(pid);
            Ok(())
        }

        fn apply_packed_limit(
            &self,
            job: &JobHandle,
            packed: crate::job_backstop::PackedJobLimit,
        ) -> Result<(), BackstopError> {
            let id = Self::job_id(job);
            self.jobs
                .lock()
                .unwrap()
                .get_mut(&id)
                .ok_or_else(|| BackstopError("unknown job".into()))?
                .memory_limit = if packed.limit_flags & JOB_OBJECT_LIMIT_JOB_MEMORY.0 != 0 {
                Some(packed.job_memory_limit)
            } else {
                None
            };
            Ok(())
        }

        fn close_job(&self, job: JobHandle) {
            let id = Self::job_id(&job);
            self.jobs.lock().unwrap().remove(&id);
            std::mem::forget(job);
        }
    }

    fn app(key: &str, pid: u32, ws: u64, usage: u64) -> AppGroup {
        AppGroup {
            group_key: key.into(),
            members: vec![GroupMember {
                pid,
                create_time: 1,
                private_working_set_bytes: ws,
                private_usage_bytes: usage,
            }],
        }
    }

    fn hog_group(cap: u64) -> GroupConfig {
        GroupConfig {
            key: "hog".into(),
            cap_bytes: cap,
            always_enforce: true,
        }
    }

    fn seed_commit_ratio(rt: &mut Runtime, key: &str, commit: u64, gf: u64) {
        for _ in 0..3 {
            rt.commit_ratios
                .entry(key.to_string())
                .or_default()
                .sample(commit, gf);
        }
    }

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
        let apps = vec![app("hog", 1, 500, 500)];
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
            groups: vec![hog_group(cap)],
            pause_all: false,
        };
        let mut rt = Runtime::new();
        let apps = vec![app("hog", 1, gf, commit)];
        rt.tick_with_groups(&cfg, SystemArm::Armed, &apps, Instant::now())
            .unwrap();
        let cr = rt.commit_ratios.get("hog").expect("sampled");
        assert_eq!(cr.samples(), 1);
        assert!((cr.ratio() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn post_trim_follow_backstop_arms_mock_job_store() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::with_backstop(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 42, gf, commit);

        let t0 = Instant::now();
        let fsm = rt.groups.entry("hog".into()).or_default();
        let mut inp = GroupFsmInput {
            gf,
            cap_bytes: cap,
            system: SystemArm::Armed,
            always_enforce: true,
            runaway_multiplier: 3.0,
            now: t0,
            last_ry_live: None,
            refault_hot: false,
            trim_was_ineffective: true,
        };
        assert_eq!(fsm.step(inp), FsmAction::SoftTrim);
        inp.now = t0 + Duration::from_secs(1);
        assert_eq!(fsm.observe_post_trim(inp), FsmAction::SoftTrim);
        inp.now = t0 + Duration::from_secs(2);
        let follow = fsm.observe_post_trim(inp);
        assert_eq!(follow, FsmAction::Backstop);

        rt.arm_backstop_if_ready_for_test(&gc, &app, gf);

        let store = rt.backstop_store();
        assert!(store.has_group("hog"));
        assert!(store.assigned_pids("hog").unwrap().contains(&42));
        let ratio = commit as f64 / gf as f64;
        let expected_limit = crate::commit_ratio::translate_job_limit(cap, ratio);
        assert_eq!(store.memory_limit("hog"), Some(Some(expected_limit)));
        assert!(
            rt.diagnostics
                .lines()
                .iter()
                .any(|l| l.contains("BACKSTOP arm limit=")),
            "expected arm diagnostic, got: {:?}",
            rt.diagnostics.lines()
        );
    }

    #[test]
    fn outer_backstop_action_arms_mock_job_store() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::with_backstop(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 7, gf, commit);

        rt.arm_backstop_if_ready_for_test(&gc, &app, gf);

        let store = rt.backstop_store();
        assert!(store.has_group("hog"));
        assert!(store.assigned_pids("hog").unwrap().contains(&7));
        assert!(store.memory_limit("hog").unwrap().is_some());
    }

    #[test]
    fn nested_assign_failure_degrades_without_panic() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::with_backstop(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::with_fail_assign(99),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 99, gf, commit);

        rt.arm_backstop_if_ready_for_test(&gc, &app, gf);

        assert!(
            rt.diagnostics
                .lines()
                .iter()
                .any(|l| l.contains("BACKSTOP degrade nested")),
        );
        assert!(!rt.backstop_store().assigned_pids("hog").unwrap().contains(&99));
    }

    #[test]
    fn disarm_clears_all_backstop_limits() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::with_backstop(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 1, gf, commit);
        rt.arm_backstop_if_ready_for_test(&gc, &app, gf);
        assert!(rt.backstop_store().memory_limit("hog").unwrap().is_some());

        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![gc],
            pause_all: false,
        };
        rt.tick_with_groups(&cfg, SystemArm::Disarmed, &[app], Instant::now())
            .unwrap();

        assert_eq!(rt.backstop_store().memory_limit("hog"), Some(None));
        assert!(
            rt.diagnostics
                .lines()
                .iter()
                .any(|l| l.contains("BACKSTOP disarm")),
        );
    }

    #[test]
    fn cap_decrease_ratchet_lowers_limit_not_below_commit_floor() {
        let cap = 200_000_000u64;
        let lowered = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 180_000_000u64;
        let mut rt = Runtime::with_backstop(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 1, gf, commit);
        rt.arm_backstop_if_ready_for_test(&gc, &app, gf);
        let armed = rt.backstop_store().memory_limit("hog").unwrap().unwrap();

        rt.last_caps.insert("hog".into(), cap);
        let lowered_gc = hog_group(lowered);
        rt.track_cap_change(&lowered_gc, &app);

        let new_limit = rt.backstop_store().memory_limit("hog").unwrap().unwrap();
        let ratio = commit as f64 / gf as f64;
        let target = translate_job_limit(lowered, ratio);
        let floor = ratchet_limit(target, commit);
        assert_eq!(new_limit, floor);
        assert!(new_limit < armed);
    }
}
