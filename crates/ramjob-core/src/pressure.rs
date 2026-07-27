//! Pressure sampling: injectable source + Win32 adapter (M2).

use crate::policy::PressureSample;
use std::time::Instant;

pub trait PressureSource {
    fn sample(&mut self) -> Result<PressureSample, String>;
}

#[derive(Debug, Clone)]
pub struct SimulatedPressure {
    pub low_memory: bool,
    pub high_memory: bool,
    pub hard_faults_per_sec: f64,
}

impl PressureSource for SimulatedPressure {
    fn sample(&mut self) -> Result<PressureSample, String> {
        Ok(PressureSample {
            low_memory: self.low_memory,
            high_memory: self.high_memory,
            hard_faults_per_sec: self.hard_faults_per_sec,
            now: Instant::now(),
        })
    }
}

/// Win32 low/high memory-resource notifications.
/// Hard-fault rate is reported as 0.0 in M2; ARM still needs faults>30 from Simulated
/// tests. Live ARM uses notification + optional always_enforce/runaway without fault confirm
/// unless `force_faults` is set for demos.
pub struct WinPressure {
    low: windows::Win32::Foundation::HANDLE,
    high: windows::Win32::Foundation::HANDLE,
    /// When true, report faults at 40/s whenever low is signaled (dev override).
    pub assume_faults_when_low: bool,
}

impl WinPressure {
    pub fn new() -> Result<Self, String> {
        use windows::Win32::System::Memory::{
            CreateMemoryResourceNotification, LowMemoryResourceNotification,
            HighMemoryResourceNotification,
        };
        unsafe {
            let low = CreateMemoryResourceNotification(LowMemoryResourceNotification)
                .map_err(|e| format!("CreateMemoryResourceNotification(Low): {e}"))?;
            let high = CreateMemoryResourceNotification(HighMemoryResourceNotification)
                .map_err(|e| format!("CreateMemoryResourceNotification(High): {e}"))?;
            Ok(Self {
                low,
                high,
                assume_faults_when_low: false,
            })
        }
    }
}

impl PressureSource for WinPressure {
    fn sample(&mut self) -> Result<PressureSample, String> {
        use windows::Win32::System::Memory::QueryMemoryResourceNotification;
        use windows::Win32::Foundation::BOOL;
        unsafe {
            let mut low_signaled = BOOL(0);
            QueryMemoryResourceNotification(self.low, &mut low_signaled)
                .map_err(|e| format!("QueryMemoryResourceNotification(Low): {e}"))?;
            let mut high_signaled = BOOL(0);
            QueryMemoryResourceNotification(self.high, &mut high_signaled)
                .map_err(|e| format!("QueryMemoryResourceNotification(High): {e}"))?;
            let low = low_signaled.as_bool();
            let high = high_signaled.as_bool();
            let faults = if low && self.assume_faults_when_low {
                40.0
            } else {
                0.0
            };
            Ok(PressureSample {
                low_memory: low,
                high_memory: high,
                hard_faults_per_sec: faults,
                now: Instant::now(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{PolicyState, SystemArm, ARM_DWELL};

    #[test]
    fn simulated_feeds_policy_to_armed() {
        let mut sim = SimulatedPressure {
            low_memory: true,
            high_memory: false,
            hard_faults_per_sec: 40.0,
        };
        let mut policy = PolicyState::new();
        let s0 = sim.sample().unwrap();
        policy.update(s0);
        let mut s1 = sim.sample().unwrap();
        s1.now = s0.now + ARM_DWELL;
        assert_eq!(policy.update(s1), SystemArm::Armed);
    }
}
