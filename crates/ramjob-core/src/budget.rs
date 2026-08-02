//! SPEC §6 idle budget sampling for CI harness.

use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::GetCurrentProcess;

/// SPEC §6 idle working-set ceiling (25 MB).
pub const IDLE_WS_CEILING_BYTES: u64 = 25 * 1024 * 1024;

/// SPEC §6 idle working-set target (12 MB) — informational; CI asserts ceiling only.
pub const IDLE_WS_TARGET_BYTES: u64 = 12 * 1024 * 1024;

/// SPEC §6 idle CPU ceiling (0.3%).
pub const IDLE_CPU_CEILING_PCT: f64 = 0.3;

/// Sample current process total working set (`PROCESS_MEMORY_COUNTERS::WorkingSetSize`).
pub fn sample_own_working_set_bytes() -> windows::core::Result<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut _ as *mut _,
            counters.cb,
        )?;
    }
    Ok(counters.WorkingSetSize as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use std::thread;
    use std::time::Duration;

    /// Let discovery backends and allocator settle before sampling.
    const SETTLE: Duration = Duration::from_millis(750);

    #[test]
    fn sample_own_working_set_is_nonzero() {
        let ws = sample_own_working_set_bytes().expect("GetProcessMemoryInfo");
        assert!(ws > 0, "working set must be non-zero");
    }

    /// SPEC §6 idle WS ceiling after a disarmed `Runtime` construct + brief settle.
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "debug test binaries exceed 25 MB WS; run `cargo test -p ramjob-core budget --release`"
    )]
    fn idle_runtime_working_set_within_ceiling() {
        let _rt = Runtime::new();
        thread::sleep(SETTLE);
        let ws = sample_own_working_set_bytes().expect("GetProcessMemoryInfo");
        assert!(
            ws <= IDLE_WS_CEILING_BYTES,
            "idle WS {ws} bytes ({:.2} MB) exceeds SPEC §6 ceiling {} bytes (25 MB); target is {} bytes (12 MB)",
            ws as f64 / (1024.0 * 1024.0),
            IDLE_WS_CEILING_BYTES,
            IDLE_WS_TARGET_BYTES,
        );
    }
}
