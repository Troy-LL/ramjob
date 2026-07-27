//! Per-group soft-trim FSM (SPEC §4.2 / M2). No Job Object.

use std::time::{Duration, Instant};

use crate::policy::SystemArm;

pub const RY_LIVE_CUTOFF: f64 = 0.35;
pub const IDLE_RATIO: f64 = 0.85;
pub const TRIM_TARGET_RATIO: f64 = 0.9;
const INEFFECTIVE_WINDOW: Duration = Duration::from_secs(60);
const INEFFECTIVE_NEEDED: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPhase {
    Idle,
    Pressure,
    Trim,
    LowYield,
    Thrashing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmAction {
    None,
    SoftTrim,
    RecordWouldBackstop,
}

#[derive(Debug, Clone)]
pub struct GroupFsm {
    pub phase: GroupPhase,
    low_yield_streak: u32,
    thrash_streak: u32,
    ineffective_times: Vec<Instant>,
    would_backstop_emitted: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct GroupFsmInput {
    pub gf: u64,
    pub cap_bytes: u64,
    pub system: SystemArm,
    pub always_enforce: bool,
    pub runaway_multiplier: f64,
    pub now: Instant,
    pub last_ry_live: Option<f64>,
    pub refault_hot: bool,
    pub trim_was_ineffective: bool,
}

impl GroupFsm {
    pub fn new() -> Self {
        Self {
            phase: GroupPhase::Idle,
            low_yield_streak: 0,
            thrash_streak: 0,
            ineffective_times: Vec::new(),
            would_backstop_emitted: false,
        }
    }

    pub fn is_active(&self, input: &GroupFsmInput) -> bool {
        if input.cap_bytes == 0 {
            return false;
        }
        if input.system == SystemArm::Armed || input.always_enforce {
            return true;
        }
        let thresh = input.runaway_multiplier * input.cap_bytes as f64;
        (input.gf as f64) >= thresh
    }

    /// Post-trim FSM feedback (Ry_live, refault, ineffective-trim). Same transition table as
    /// [`step`](Self::step); call only after a real measured trim with post-sample fields set.
    pub fn observe_post_trim(&mut self, input: GroupFsmInput) -> FsmAction {
        self.step(input)
    }

    pub fn step(&mut self, input: GroupFsmInput) -> FsmAction {
        if input.cap_bytes == 0 {
            self.phase = GroupPhase::Idle;
            return FsmAction::None;
        }

        if !self.is_active(&input) {
            self.phase = GroupPhase::Idle;
            return FsmAction::None;
        }

        if matches!(self.phase, GroupPhase::LowYield | GroupPhase::Thrashing) {
            return FsmAction::None;
        }

        if let Some(ry) = input.last_ry_live {
            if ry < RY_LIVE_CUTOFF {
                self.low_yield_streak += 1;
            } else {
                self.low_yield_streak = 0;
            }
            if self.low_yield_streak >= 2 {
                self.phase = GroupPhase::LowYield;
                return FsmAction::None;
            }
        }

        if input.refault_hot {
            self.thrash_streak += 1;
            if self.thrash_streak >= 2 {
                self.phase = GroupPhase::Thrashing;
                return FsmAction::None;
            }
        } else {
            self.thrash_streak = 0;
        }

        let cap = input.cap_bytes as f64;
        let idle_line = IDLE_RATIO * cap;
        let gf = input.gf as f64;

        if gf < idle_line {
            self.phase = GroupPhase::Idle;
            return FsmAction::None;
        }
        if gf < cap {
            self.phase = GroupPhase::Pressure;
            return FsmAction::None;
        }

        self.phase = GroupPhase::Trim;

        if input.trim_was_ineffective {
            self.ineffective_times.push(input.now);
            self.ineffective_times
                .retain(|t| input.now.duration_since(*t) <= INEFFECTIVE_WINDOW);
            if self.ineffective_times.len() as u32 >= INEFFECTIVE_NEEDED
                && !self.would_backstop_emitted
            {
                self.would_backstop_emitted = true;
                return FsmAction::RecordWouldBackstop;
            }
        }

        FsmAction::SoftTrim
    }
}

impl Default for GroupFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(
        gf: u64,
        cap: u64,
        system: SystemArm,
        always: bool,
        runaway: f64,
        now: Instant,
    ) -> GroupFsmInput {
        GroupFsmInput {
            gf,
            cap_bytes: cap,
            system,
            always_enforce: always,
            runaway_multiplier: runaway,
            now,
            last_ry_live: None,
            refault_hot: false,
            trim_was_ineffective: false,
        }
    }

    #[test]
    fn unlimited_cap_is_idle() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        assert_eq!(
            f.step(input(9_000_000_000, 0, SystemArm::Armed, false, 3.0, now)),
            FsmAction::None
        );
        assert_eq!(f.phase, GroupPhase::Idle);
    }

    #[test]
    fn idle_pressure_trim_when_armed() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        let cap = 1_000u64;
        assert_eq!(
            f.step(input(800, cap, SystemArm::Armed, false, 3.0, now)),
            FsmAction::None
        );
        assert_eq!(f.phase, GroupPhase::Idle);
        assert_eq!(
            f.step(input(900, cap, SystemArm::Armed, false, 3.0, now)),
            FsmAction::None
        );
        assert_eq!(f.phase, GroupPhase::Pressure);
        assert_eq!(
            f.step(input(1000, cap, SystemArm::Armed, false, 3.0, now)),
            FsmAction::SoftTrim
        );
        assert_eq!(f.phase, GroupPhase::Trim);
    }

    #[test]
    fn runaway_force_arm_while_disarmed() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        let cap = 100u64;
        assert_eq!(
            f.step(input(300, cap, SystemArm::Disarmed, false, 3.0, now)),
            FsmAction::SoftTrim
        );
    }

    #[test]
    fn disarmed_below_runaway_is_idle() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        assert_eq!(
            f.step(input(200, 100, SystemArm::Disarmed, false, 3.0, now)),
            FsmAction::None
        );
        assert_eq!(f.phase, GroupPhase::Idle);
    }

    #[test]
    fn always_enforce_trims_while_disarmed() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        assert_eq!(
            f.step(input(1000, 100, SystemArm::Disarmed, true, 3.0, now)),
            FsmAction::SoftTrim
        );
    }

    #[test]
    fn two_low_yield_stops() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        let mut inp = input(1000, 100, SystemArm::Armed, false, 3.0, now);
        inp.last_ry_live = Some(0.1);
        f.step(inp);
        inp.last_ry_live = Some(0.2);
        assert_eq!(f.step(inp), FsmAction::None);
        assert_eq!(f.phase, GroupPhase::LowYield);
    }

    #[test]
    fn two_refault_marks_thrashing() {
        let mut f = GroupFsm::new();
        let now = Instant::now();
        let mut inp = input(1000, 100, SystemArm::Armed, false, 3.0, now);
        inp.refault_hot = true;
        f.step(inp);
        assert_eq!(f.step(inp), FsmAction::None);
        assert_eq!(f.phase, GroupPhase::Thrashing);
    }

    #[test]
    fn three_ineffective_records_would_backstop() {
        let mut f = GroupFsm::new();
        let t0 = Instant::now();
        let mut inp = input(1000, 100, SystemArm::Armed, false, 3.0, t0);
        inp.trim_was_ineffective = true;
        assert_eq!(f.step(inp), FsmAction::SoftTrim);
        inp.now = t0 + Duration::from_secs(1);
        assert_eq!(f.step(inp), FsmAction::SoftTrim);
        inp.now = t0 + Duration::from_secs(2);
        assert_eq!(f.step(inp), FsmAction::RecordWouldBackstop);
    }
}
