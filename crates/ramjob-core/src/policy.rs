//! System Armed/Disarmed pressure policy (SPEC §4.1 / M2).

use std::time::{Duration, Instant};

pub const ARM_DWELL: Duration = Duration::from_secs(20);
pub const DISARM_DWELL: Duration = Duration::from_secs(60);
pub const HARD_FAULT_ARM_THRESHOLD: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemArm {
    Disarmed,
    Armed,
}

#[derive(Debug, Clone, Copy)]
pub struct PressureSample {
    pub low_memory: bool,
    pub high_memory: bool,
    pub hard_faults_per_sec: f64,
    pub now: Instant,
}

#[derive(Debug)]
pub struct PolicyState {
    pub arm: SystemArm,
    arm_candidate_since: Option<Instant>,
    disarm_candidate_since: Option<Instant>,
}

impl PolicyState {
    pub fn new() -> Self {
        Self {
            arm: SystemArm::Disarmed,
            arm_candidate_since: None,
            disarm_candidate_since: None,
        }
    }

    pub fn update(&mut self, sample: PressureSample) -> SystemArm {
        let arm_ok =
            sample.low_memory && sample.hard_faults_per_sec > HARD_FAULT_ARM_THRESHOLD;
        let disarm_ok = sample.high_memory;

        match self.arm {
            SystemArm::Disarmed => {
                self.disarm_candidate_since = None;
                if arm_ok {
                    let since = self.arm_candidate_since.get_or_insert(sample.now);
                    if sample.now.duration_since(*since) >= ARM_DWELL {
                        self.arm = SystemArm::Armed;
                        self.arm_candidate_since = None;
                    }
                } else {
                    self.arm_candidate_since = None;
                }
            }
            SystemArm::Armed => {
                self.arm_candidate_since = None;
                if disarm_ok {
                    let since = self.disarm_candidate_since.get_or_insert(sample.now);
                    if sample.now.duration_since(*since) >= DISARM_DWELL {
                        self.arm = SystemArm::Disarmed;
                        self.disarm_candidate_since = None;
                    }
                } else {
                    self.disarm_candidate_since = None;
                }
            }
        }
        self.arm
    }
}

impl Default for PolicyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(now: Instant, low: bool, high: bool, faults: f64) -> PressureSample {
        PressureSample {
            low_memory: low,
            high_memory: high,
            hard_faults_per_sec: faults,
            now,
        }
    }

    #[test]
    fn stays_disarmed_without_dwell() {
        let t0 = Instant::now();
        let mut p = PolicyState::new();
        p.update(sample(t0, true, false, 40.0));
        assert_eq!(p.arm, SystemArm::Disarmed);
        p.update(sample(t0 + Duration::from_secs(10), true, false, 40.0));
        assert_eq!(p.arm, SystemArm::Disarmed);
    }

    #[test]
    fn arms_after_20s_low_and_faults() {
        let t0 = Instant::now();
        let mut p = PolicyState::new();
        p.update(sample(t0, true, false, 40.0));
        assert_eq!(
            p.update(sample(t0 + Duration::from_secs(20), true, false, 40.0)),
            SystemArm::Armed
        );
    }

    #[test]
    fn disarms_after_60s_high() {
        let t0 = Instant::now();
        let mut p = PolicyState::new();
        p.update(sample(t0, true, false, 40.0));
        p.update(sample(t0 + Duration::from_secs(20), true, false, 40.0));
        assert_eq!(p.arm, SystemArm::Armed);
        p.update(sample(t0 + Duration::from_secs(21), false, true, 0.0));
        assert_eq!(
            p.update(sample(t0 + Duration::from_secs(21 + 60), false, true, 0.0)),
            SystemArm::Disarmed
        );
    }

    #[test]
    fn twitchy_low_without_faults_does_not_arm() {
        let t0 = Instant::now();
        let mut p = PolicyState::new();
        p.update(sample(t0, true, false, 0.0));
        assert_eq!(
            p.update(sample(t0 + Duration::from_secs(30), true, false, 0.0)),
            SystemArm::Disarmed
        );
    }
}
