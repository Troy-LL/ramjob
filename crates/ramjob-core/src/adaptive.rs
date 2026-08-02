//! Adaptive polling ladder (SPEC §6.1 / M5).

use std::time::Duration;

use crate::fsm::GroupPhase;
use crate::policy::SystemArm;

/// Relative enforcement heat for comparing group phases (higher = hotter).
pub fn phase_heat(phase: GroupPhase) -> u8 {
    match phase {
        GroupPhase::Idle => 0,
        GroupPhase::Pressure => 1,
        GroupPhase::Trim => 2,
        GroupPhase::LowYield => 3,
        GroupPhase::Thrashing => 4,
    }
}

/// Max phase across an iterator; `None` when no groups exist.
pub fn hottest_phase(phases: impl IntoIterator<Item = GroupPhase>) -> Option<GroupPhase> {
    phases
        .into_iter()
        .max_by_key(|p| phase_heat(*p))
}

/// Sleep until the next tick per SPEC §6.1.
///
/// `hottest_phase` is the max group FSM phase (or `None` when idle / no groups).
/// `backstop_active` forces the 1 s TRIM/BACKSTOP cadence even if phases are cooler.
pub fn next_sleep(
    arm: SystemArm,
    hottest_phase: Option<GroupPhase>,
    panel_open: bool,
    backstop_active: bool,
) -> Duration {
    if panel_open {
        return Duration::from_secs(1);
    }
    match arm {
        SystemArm::Disarmed => Duration::from_secs(120),
        SystemArm::Armed => {
            if backstop_active {
                return Duration::from_secs(1);
            }
            match hottest_phase.unwrap_or(GroupPhase::Idle) {
                GroupPhase::Idle => Duration::from_secs(15),
                GroupPhase::Pressure => Duration::from_secs(3),
                GroupPhase::Trim | GroupPhase::LowYield | GroupPhase::Thrashing => {
                    Duration::from_secs(1)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARMED: SystemArm = SystemArm::Armed;
    const DISARMED: SystemArm = SystemArm::Disarmed;

    #[test]
    fn panel_open_always_one_second() {
        for arm in [ARMED, DISARMED] {
            for phase in [
                None,
                Some(GroupPhase::Idle),
                Some(GroupPhase::Pressure),
                Some(GroupPhase::Trim),
            ] {
                assert_eq!(
                    next_sleep(arm, phase, true, false),
                    Duration::from_secs(1),
                    "arm={arm:?} phase={phase:?}"
                );
            }
        }
    }

    #[test]
    fn disarmed_panel_closed_one_twenty() {
        assert_eq!(
            next_sleep(DISARMED, None, false, false),
            Duration::from_secs(120)
        );
        assert_eq!(
            next_sleep(DISARMED, Some(GroupPhase::Thrashing), false, false),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn armed_panel_closed_idle_fifteen() {
        assert_eq!(
            next_sleep(ARMED, None, false, false),
            Duration::from_secs(15)
        );
        assert_eq!(
            next_sleep(ARMED, Some(GroupPhase::Idle), false, false),
            Duration::from_secs(15)
        );
    }

    #[test]
    fn armed_panel_closed_pressure_three() {
        assert_eq!(
            next_sleep(ARMED, Some(GroupPhase::Pressure), false, false),
            Duration::from_secs(3)
        );
    }

    #[test]
    fn armed_panel_closed_trim_family_one() {
        for phase in [
            GroupPhase::Trim,
            GroupPhase::LowYield,
            GroupPhase::Thrashing,
        ] {
            assert_eq!(
                next_sleep(ARMED, Some(phase), false, false),
                Duration::from_secs(1),
                "{phase:?}"
            );
        }
    }

    #[test]
    fn armed_backstop_active_one_even_when_idle() {
        assert_eq!(
            next_sleep(ARMED, Some(GroupPhase::Idle), false, true),
            Duration::from_secs(1)
        );
        assert_eq!(next_sleep(ARMED, None, false, true), Duration::from_secs(1));
    }

    #[test]
    fn hottest_phase_picks_max_heat() {
        assert_eq!(
            hottest_phase([
                GroupPhase::Idle,
                GroupPhase::Pressure,
                GroupPhase::Trim,
            ]),
            Some(GroupPhase::Trim)
        );
        assert_eq!(hottest_phase([] as [GroupPhase; 0]), None);
    }
}
