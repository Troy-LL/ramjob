//! Job Object backstop orchestration for [`Runtime`](crate::runtime::Runtime).

use crate::accountant::group_commit_charge;
use crate::commit_ratio::{ratchet_limit, translate_job_limit};
use crate::config::GroupConfig;
use crate::fsm::GroupPhase;
use crate::grouper::AppGroup;
use crate::job_backstop::JobLimitState;
use crate::policy::SystemArm;
use crate::runtime::Runtime;
impl Runtime {
    pub(super) fn clear_backstop_on_disarm(&mut self, system: SystemArm) {
        if system != SystemArm::Disarmed || !self.backstop.any_limited() {
            return;
        }
        match self.backstop.clear_all_limits() {
            Ok(()) => self.diagnostics.push("BACKSTOP disarm".to_string()),
            Err(e) => self
                .diagnostics
                .push(format!("BACKSTOP disarm err: {}", e.0)),
        }
    }

    pub(super) fn sample_commit_ratio_if_pressure(&mut self, key: &str, app: &AppGroup, gf: u64) {
        if !matches!(
            self.groups.get(key).map(|f| f.phase),
            Some(GroupPhase::Pressure)
        ) {
            return;
        }
        let commit = group_commit_charge(app);
        self.commit_ratios
            .entry(key.to_string())
            .or_default()
            .sample(commit, gf);
    }

    /// Arm backstop when FSM requests it (direct action or post-trim follow).
    pub(crate) fn arm_backstop_if_ready(&mut self, gc: &GroupConfig, app: &AppGroup) {
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

        let ratio = cr.ratio();
        let already_limited = matches!(
            self.backstop.memory_limit(&gc.key),
            JobLimitState::Limited(_)
        );
        let reason = if already_limited { "ratchet" } else { "arm" };

        for member in &app.members {
            if let Err(e) = self.backstop.assign_pid(&gc.key, member.pid) {
                self.diagnostics.push(format!(
                    "{} BACKSTOP degrade nested pid={}: {}",
                    gc.key, member.pid, e.0
                ));
            }
        }

        self.apply_backstop_limit(gc, app, ratio, reason, already_limited);
    }

    pub(crate) fn track_cap_change(&mut self, gc: &GroupConfig, app: &AppGroup) {
        let prev_cap = self.last_caps.get(&gc.key).copied();
        if let Some(prev) = prev_cap {
            if gc.cap_bytes < prev {
                if let JobLimitState::Limited(current) = self.backstop.memory_limit(&gc.key) {
                    if let Some(cr) = self.commit_ratios.get(&gc.key) {
                        if cr.ready() {
                            let commit = group_commit_charge(app);
                            let target = translate_job_limit(gc.cap_bytes, cr.ratio());
                            let limit = ratchet_limit(target, commit);
                            if limit < current {
                                self.apply_backstop_limit(
                                    gc,
                                    app,
                                    cr.ratio(),
                                    "ratchet cap_decrease",
                                    true,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.last_caps.insert(gc.key.clone(), gc.cap_bytes);
    }

    fn apply_backstop_limit(
        &mut self,
        gc: &GroupConfig,
        app: &AppGroup,
        ratio: f64,
        reason: &str,
        ratchet_from_commit: bool,
    ) {
        let commit = group_commit_charge(app);
        let target = translate_job_limit(gc.cap_bytes, ratio);
        let limit = if ratchet_from_commit {
            ratchet_limit(target, commit)
        } else {
            target
        };

        match self.backstop.set_memory_limit(&gc.key, limit) {
            Ok(()) => self.diagnostics.push(format!(
                "{} BACKSTOP {reason} limit={limit}",
                gc.key
            )),
            Err(e) => self.diagnostics.push(format!(
                "{} BACKSTOP degrade {reason}: {}",
                gc.key, e.0
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::commit_ratio::translate_job_limit;
    use crate::config::RamjobConfig;
    use crate::fsm::{FsmAction, GroupFsmInput, TRIM_TARGET_RATIO};
    use crate::gate::{GateMeasurement, GATE_SETTLE};
    use crate::grouper::{AppGroup, GroupMember};
    use crate::job_backstop::mock::MockBackstopHooks;
    use crate::job_backstop::JobBackstopStore;
    use crate::runtime::set_trim_measurement_stub;

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
            ..Default::default()
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

    fn ineffective_trim_measurement(
        key: &str,
        pid: u32,
        cap: u64,
        gf: u64,
    ) -> GateMeasurement {
        let target = (TRIM_TARGET_RATIO * cap as f64) as u64;
        GateMeasurement {
            group_key: key.into(),
            target_pids: vec![pid],
            trimmed_pids: vec![pid],
            excluded_pids: vec![],
            rate_limited: false,
            gf0: gf,
            gf1: target.saturating_add(1).max(gf),
            available0: 0,
            available1: 0,
            cs0: Some(0),
            cs1: Some(0),
            ry_bench: None,
            ry_live: Some(0.5),
            verdict: None,
            settle: GATE_SETTLE,
            trim_errors: vec![],
        }
    }

    #[test]
    fn tick_with_groups_soft_trim_follow_backstop_arms_mock_store() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let pid = 42u32;
        let key = "hog";
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, key, commit, gf);
        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![hog_group(cap)],
            pause_all: false,
            ..Default::default()
        };
        let app = app(key, pid, gf, commit);
        let t0 = Instant::now();

        for (i, offset) in [(0u64, 0), (1, 21), (2, 42)] {
            set_trim_measurement_stub(Ok(ineffective_trim_measurement(
                key, pid, cap, gf,
            )));
            let out = rt
                .tick_with_groups(
                    &cfg,
                    SystemArm::Armed,
                    &[app.clone()],
                    t0 + Duration::from_secs(offset),
                )
                .unwrap();
            assert_eq!(out.trims_attempted, 1, "tick {i} should SoftTrim");
        }

        let store = rt.backstop_store();
        assert!(store.has_group(key));
        assert!(store.assigned_pids(key).unwrap().contains(&pid));
        let ratio = commit as f64 / gf as f64;
        let expected_limit = translate_job_limit(cap, ratio);
        assert_eq!(
            store.memory_limit(key),
            JobLimitState::Limited(expected_limit)
        );
        let diag = rt.diagnostics.lines().join("\n");
        assert!(
            diag.contains("follow=Backstop"),
            "expected observe_post_trim follow=Backstop, got:\n{diag}"
        );
        assert!(
            diag.contains("BACKSTOP arm limit="),
            "expected arm diagnostic, got:\n{diag}"
        );
    }

    #[test]
    fn post_trim_follow_backstop_arms_mock_job_store() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
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

        rt.arm_backstop_if_ready(&gc, &app);

        let store = rt.backstop_store();
        assert!(store.has_group("hog"));
        assert!(store.assigned_pids("hog").unwrap().contains(&42));
        let ratio = commit as f64 / gf as f64;
        let expected_limit = translate_job_limit(cap, ratio);
        assert_eq!(
            store.memory_limit("hog"),
            JobLimitState::Limited(expected_limit)
        );
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
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 7, gf, commit);

        rt.arm_backstop_if_ready(&gc, &app);

        let store = rt.backstop_store();
        assert!(store.has_group("hog"));
        assert!(store.assigned_pids("hog").unwrap().contains(&7));
        assert!(matches!(
            store.memory_limit("hog"),
            JobLimitState::Limited(_)
        ));
    }

    #[test]
    fn nested_assign_failure_degrades_without_panic() {
        let cap = 100_000_000u64;
        let gf = 150_000_000u64;
        let commit = 200_000_000u64;
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::with_fail_assign(99),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 99, gf, commit);

        rt.arm_backstop_if_ready(&gc, &app);

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
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 1, gf, commit);
        rt.arm_backstop_if_ready(&gc, &app);
        assert!(matches!(
            rt.backstop_store().memory_limit("hog"),
            JobLimitState::Limited(_)
        ));

        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![gc],
            pause_all: false,
            ..Default::default()
        };
        rt.tick_with_groups(&cfg, SystemArm::Disarmed, &[app], Instant::now())
            .unwrap();

        assert_eq!(
            rt.backstop_store().memory_limit("hog"),
            JobLimitState::Unlimited
        );
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
        let mut rt = Runtime::new_with_backstop_store(JobBackstopStore::with_hooks(Box::new(
            MockBackstopHooks::new(),
        )));
        seed_commit_ratio(&mut rt, "hog", commit, gf);
        let gc = hog_group(cap);
        let app = app("hog", 1, gf, commit);
        rt.arm_backstop_if_ready(&gc, &app);
        let JobLimitState::Limited(armed) = rt.backstop_store().memory_limit("hog") else {
            panic!("expected limited backstop");
        };

        rt.last_caps.insert("hog".into(), cap);
        let lowered_gc = hog_group(lowered);
        rt.track_cap_change(&lowered_gc, &app);

        let JobLimitState::Limited(new_limit) = rt.backstop_store().memory_limit("hog") else {
            panic!("expected limited backstop after ratchet");
        };
        let ratio = commit as f64 / gf as f64;
        let target = translate_job_limit(lowered, ratio);
        let floor = ratchet_limit(target, commit);
        assert_eq!(new_limit, floor);
        assert!(new_limit < armed);
    }
}