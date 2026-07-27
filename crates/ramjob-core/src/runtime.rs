//! One daemon pipeline tick (M2).

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::accountant::group_footprint;
use crate::config::RamjobConfig;
use crate::diagnostics::DiagnosticsRing;
use crate::enforcer::{
    member_key, soft_trim_group_unlocked, with_trim_lock, ExclusionPolicy, LiveTrimHooks,
    TrimHooks, TRIM_RATE_LIMIT, TrimContext,
};
use crate::fsm::{FsmAction, GroupFsm, GroupFsmInput, GroupPhase, TRIM_TARGET_RATIO};
use crate::grouper::{group_processes, AppGroup, GroupMember};
use crate::policy::{PolicyState, SystemArm};
use crate::pressure::PressureSource;
use crate::scanner::{enumerate_processes_with_cache, PathCache, ProcessRecord};
use crate::yield_math::{compress_store_ws, measure_ry_live};

pub struct Runtime {
    pub config: RamjobConfig,
    pub policy: PolicyState,
    pub groups: HashMap<String, GroupFsm>,
    pub rates: HashMap<String, Instant>,
    pub diagnostics: DiagnosticsRing,
    path_cache: PathCache,
}

pub struct TickOutcome {
    pub system: SystemArm,
    pub trims_attempted: usize,
}

impl Runtime {
    pub fn from_config(config: RamjobConfig) -> Self {
        Self {
            config,
            policy: PolicyState::new(),
            groups: HashMap::new(),
            rates: HashMap::new(),
            diagnostics: DiagnosticsRing::new(),
            path_cache: PathCache::new(),
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

        let procs = enumerate_processes_with_cache(&mut self.path_cache)
            .map_err(|s| format!("enumerate: {s:?}"))?;
        let apps = group_processes(&procs);
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
        let cfg_groups = self.config.groups.clone();

        for gc in &cfg_groups {
            let Some(app) = by_key.get(gc.key.as_str()) else {
                continue;
            };
            let gf = group_footprint(app);
            let fsm = self.groups.entry(gc.key.clone()).or_default();
            let mut input = GroupFsmInput {
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
                            continue;
                        }
                    }
                    let ry = measured_soft_trim(app)?;
                    trims_attempted += 1;
                    self.rates.insert(gc.key.clone(), now);
                    let target = (TRIM_TARGET_RATIO * gc.cap_bytes as f64) as u64;
                    let gf_after = estimate_group_gf_after(app, &[]);
                    let ineffective = gf_after > target && gf_after > 0;
                    let _ = ineffective;
                    input.last_ry_live = ry;
                    input.trim_was_ineffective = ry.map(|v| v < 0.1).unwrap_or(false);
                    let fsm = self.groups.get_mut(&gc.key).unwrap();
                    let follow = fsm.step(input);
                    self.diagnostics.push(format!(
                        "{} SoftTrim ry_live={ry:?} follow={follow:?} phase={:?}",
                        gc.key, fsm.phase
                    ));
                }
            }
        }

        Ok(TickOutcome {
            system,
            trims_attempted,
        })
    }
}

fn measured_soft_trim(group: &AppGroup) -> Result<Option<f64>, String> {
    with_trim_lock(|| {
        let mut cache = PathCache::new();
        let procs0 = enumerate_processes_with_cache(&mut cache)
            .map_err(|s| format!("pre-sample: {s:?}"))?;
        let cs0 = compress_store_ws(&procs0).unwrap_or(0);
        let keys: Vec<_> = group.members.iter().map(member_key).collect();
        let hooks = LiveTrimHooks;
        let before = hooks
            .sample_private_ws(&keys)
            .map_err(|e| e.0.clone())?;
        let gf0: u64 = before.values().copied().sum();

        let mut rates = HashMap::new();
        let mut ctx = TrimContext {
            hooks: &hooks,
            rate_limits: &mut rates,
            now: Instant::now(),
            exclusion: ExclusionPolicy::None,
        };
        let _outcome = soft_trim_group_unlocked(group, &mut ctx);

        std::thread::sleep(Duration::from_secs(3));

        let procs1 = enumerate_processes_with_cache(&mut cache)
            .map_err(|s| format!("post-sample: {s:?}"))?;
        let cs1 = compress_store_ws(&procs1).unwrap_or(0);
        let after = hooks
            .sample_private_ws(&keys)
            .map_err(|e| e.0.clone())?;
        let gf1: u64 = keys
            .iter()
            .filter_map(|k| after.get(k).copied())
            .sum();
        let _ = procs1;
        Ok(measure_ry_live(gf0, gf1, cs0, cs1))
    })
}

fn estimate_group_gf_after(group: &AppGroup, procs: &[ProcessRecord]) -> u64 {
    if procs.is_empty() {
        return group_footprint(group);
    }
    let keys: HashSet<_> = group.members.iter().map(member_key).collect();
    procs
        .iter()
        .filter(|p| keys.contains(&(p.pid, p.create_time)))
        .map(|p| p.private_working_set_bytes)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GroupConfig;
    use crate::fsm::FsmAction;
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
    fn simulated_pressure_sample_ok() {
        let mut p = SimulatedPressure {
            low_memory: false,
            high_memory: true,
            hard_faults_per_sec: 0.0,
        };
        assert!(p.sample().is_ok());
    }
}
