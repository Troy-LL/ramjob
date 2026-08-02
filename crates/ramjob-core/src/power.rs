//! AC vs battery power probe (SPEC §6.1) and trim rate-limit selection.

use std::time::Duration;

use crate::enforcer::TRIM_RATE_LIMIT;

/// Soft-trim rate limit while on battery (SPEC §6.1).
pub const TRIM_RATE_LIMIT_BATTERY: Duration = Duration::from_secs(60);

/// Injectable AC-line probe for unit tests.
pub trait PowerSource {
    /// `true` when running on battery (AC offline).
    fn on_battery(&self) -> bool;
}

/// Live probe via `GetSystemPowerStatus`.
pub struct LivePowerSource;

impl PowerSource for LivePowerSource {
    fn on_battery(&self) -> bool {
        live_on_battery()
    }
}

/// Selected soft-trim rate limit for the current power source.
pub fn trim_rate_limit(on_battery: bool) -> Duration {
    if on_battery {
        TRIM_RATE_LIMIT_BATTERY
    } else {
        TRIM_RATE_LIMIT
    }
}

#[cfg(windows)]
fn live_on_battery() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is valid for write.
    if unsafe { GetSystemPowerStatus(&mut status).is_err() } {
        return false;
    }
    // ACLineStatus: 0 = offline (battery), 1 = online, 255 = unknown.
    status.ACLineStatus == 0
}

#[cfg(not(windows))]
fn live_on_battery() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPower {
        on_battery: bool,
    }

    impl PowerSource for MockPower {
        fn on_battery(&self) -> bool {
            self.on_battery
        }
    }

    #[test]
    fn trim_rate_limit_ac_is_20_seconds() {
        assert_eq!(trim_rate_limit(false), Duration::from_secs(20));
    }

    #[test]
    fn trim_rate_limit_battery_is_60_seconds() {
        assert_eq!(trim_rate_limit(true), Duration::from_secs(60));
    }

    #[test]
    fn mock_power_source_is_injectable() {
        let ac = MockPower { on_battery: false };
        let bat = MockPower { on_battery: true };
        assert_eq!(trim_rate_limit(ac.on_battery()), TRIM_RATE_LIMIT);
        assert_eq!(trim_rate_limit(bat.on_battery()), TRIM_RATE_LIMIT_BATTERY);
    }
}
