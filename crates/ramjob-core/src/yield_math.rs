//! Yield ratios Ry_bench / Ry_live (SPEC §2.3).
//!
//! Pure arithmetic over synthetic or measured deltas. CompressStore is the
//! working set of the Memory Compression system process from an NtQSI sweep.

use std::collections::HashMap;

use crate::enforcer::{intersect_private_ws, MemberKey};
use crate::scanner::ProcessRecord;

/// NtQSI / Task Manager image name for the Memory Compression system process.
pub const MEMORY_COMPRESSION_IMAGE: &str = "Memory Compression";

/// `Ry_live = (ΔGF − ΔCompressStore) / ΔGF`.
///
/// Returns `None` when `ΔGF == 0` (no division by zero).
pub fn ry_live(delta_gf: i64, delta_compress_store: i64) -> Option<f64> {
    if delta_gf == 0 {
        None
    } else {
        Some((delta_gf - delta_compress_store) as f64 / delta_gf as f64)
    }
}

/// `Ry_bench = Δ(Available) / Δ(GF)`.
///
/// Both arguments must use the same unit (bytes or MiB). Returns `None` when
/// `ΔGF == 0`.
pub fn ry_bench(delta_available: i64, delta_gf: i64) -> Option<f64> {
    if delta_gf == 0 {
        None
    } else {
        Some(delta_available as f64 / delta_gf as f64)
    }
}

/// SPEC §9.2 Pass line on Ry_bench.
pub const RY_BENCH_PASS: f64 = 0.5;
/// SPEC §9.2 Marginal floor on Ry_bench (inclusive).
pub const RY_BENCH_MARGINAL: f64 = 0.3;

/// Classification of a measured `Ry_bench` (SPEC §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    /// `Ry_bench ≥ 0.5`
    Pass,
    /// `0.3 ≤ Ry_bench < 0.5`
    Marginal,
    /// `Ry_bench < 0.3`
    Fail,
}

impl GateVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "Pass",
            Self::Marginal => "Marginal",
            Self::Fail => "Fail",
        }
    }
}

/// Classify Ry_bench against SPEC §9.2 thresholds.
pub fn classify_ry_bench(ry: f64) -> GateVerdict {
    if ry >= RY_BENCH_PASS {
        GateVerdict::Pass
    } else if ry >= RY_BENCH_MARGINAL {
        GateVerdict::Marginal
    } else {
        GateVerdict::Fail
    }
}

/// `ΔCompressStore = cs1 − cs0` (growth of the compression store).
pub fn delta_compress_store(cs0: u64, cs1: u64) -> i64 {
    cs1 as i64 - cs0 as i64
}

/// Intersected private-WS ΔGF: `gf0 − gf1` for members present in both maps
/// keyed by `(pid, create_time)`.
pub fn delta_gf_intersected(
    before: &HashMap<MemberKey, u64>,
    after: &HashMap<MemberKey, u64>,
) -> i64 {
    let (gf0, gf1) = intersect_private_ws(before, after);
    gf0 as i64 - gf1 as i64
}

/// Ry_live from paired pre/post samples (`gf0/gf1` already intersected).
pub fn measure_ry_live(gf0: u64, gf1: u64, cs0: u64, cs1: u64) -> Option<f64> {
    ry_live(gf0 as i64 - gf1 as i64, delta_compress_store(cs0, cs1))
}

/// Ry_bench from paired Available and GF samples (same units).
pub fn measure_ry_bench(available0: u64, available1: u64, gf0: u64, gf1: u64) -> Option<f64> {
    let delta_available = available1 as i64 - available0 as i64;
    let delta_gf = gf0 as i64 - gf1 as i64;
    ry_bench(delta_available, delta_gf)
}

/// Working set of Memory Compression when present in an NtQSI sweep.
pub fn compress_store_ws(procs: &[ProcessRecord]) -> Option<u64> {
    procs.iter().find_map(|p| {
        if eq_ignore_ascii_case(&p.image_name, MEMORY_COMPRESSION_IMAGE) {
            Some(p.working_set_bytes)
        } else {
            None
        }
    })
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.eq_ignore_ascii_case(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{enumerate_processes_with_cache, PathCache};

    fn approx_eq(got: f64, want: f64) {
        assert!(
            (got - want).abs() < 1e-9,
            "got {got}, want {want}"
        );
    }

    fn proc(image_name: &str, working_set_bytes: u64) -> ProcessRecord {
        ProcessRecord {
            pid: 1,
            ppid: 0,
            session_id: 0,
            image_name: image_name.into(),
            private_working_set_bytes: 0,
            private_usage_bytes: 0,
            working_set_bytes,
            create_time: 1,
            image_path: None,
        }
    }

    #[test]
    fn ry_live_spec_example_numbers() {
        // Trim removed 2 GB GF; CompressStore grew 1.3 GB → Ry_live = 0.35
        let delta_gf = 2_000_000_000i64;
        let delta_cs = 1_300_000_000i64;
        approx_eq(ry_live(delta_gf, delta_cs).unwrap(), 0.35);
    }

    #[test]
    fn ry_live_from_paired_samples_matches_formula() {
        let gf0 = 5_000u64;
        let gf1 = 3_000u64;
        let cs0 = 100u64;
        let cs1 = 700u64;
        // (2000 − 600) / 2000 = 0.7
        approx_eq(measure_ry_live(gf0, gf1, cs0, cs1).unwrap(), 0.7);
        approx_eq(
            ry_live(gf0 as i64 - gf1 as i64, delta_compress_store(cs0, cs1)).unwrap(),
            0.7,
        );
    }

    #[test]
    fn ry_live_perfect_yield_when_compress_store_unchanged() {
        approx_eq(ry_live(1_000, 0).unwrap(), 1.0);
    }

    #[test]
    fn ry_live_zero_when_all_reclaimed_bytes_enter_compress_store() {
        approx_eq(ry_live(800, 800).unwrap(), 0.0);
    }

    #[test]
    fn ry_live_negative_when_compress_store_grows_more_than_gf() {
        approx_eq(ry_live(100, 150).unwrap(), -0.5);
    }

    #[test]
    fn ry_live_none_when_delta_gf_is_zero() {
        assert_eq!(ry_live(0, 0), None);
        assert_eq!(ry_live(0, 50), None);
        assert_eq!(measure_ry_live(100, 100, 10, 20), None);
    }

    #[test]
    fn ry_bench_available_over_gf() {
        // Available rose 500 MiB; GF fell 1000 MiB → 0.5
        approx_eq(ry_bench(500, 1000).unwrap(), 0.5);
        approx_eq(
            measure_ry_bench(
                2_000 * 1024 * 1024,
                2_500 * 1024 * 1024,
                4_000 * 1024 * 1024,
                3_000 * 1024 * 1024,
            )
            .unwrap(),
            0.5,
        );
    }

    #[test]
    fn ry_bench_none_when_delta_gf_is_zero() {
        assert_eq!(ry_bench(100, 0), None);
        assert_eq!(measure_ry_bench(100, 200, 50, 50), None);
    }

    #[test]
    fn ry_bench_pass_marginal_fail_thresholds() {
        approx_eq(ry_bench(600, 1000).unwrap(), 0.6); // pass ≥ 0.5
        approx_eq(ry_bench(400, 1000).unwrap(), 0.4); // marginal
        approx_eq(ry_bench(200, 1000).unwrap(), 0.2); // fail
        assert_eq!(classify_ry_bench(0.6), GateVerdict::Pass);
        assert_eq!(classify_ry_bench(0.5), GateVerdict::Pass);
        assert_eq!(classify_ry_bench(0.4), GateVerdict::Marginal);
        assert_eq!(classify_ry_bench(0.3), GateVerdict::Marginal);
        assert_eq!(classify_ry_bench(0.299), GateVerdict::Fail);
    }

    #[test]
    fn delta_gf_intersected_uses_pid_ctime_private_ws_only() {
        let mut before = HashMap::new();
        before.insert((1, 10), 1_000u64);
        before.insert((2, 20), 2_000u64);
        before.insert((3, 30), 3_000u64); // exits

        let mut after = HashMap::new();
        after.insert((1, 10), 400u64);
        after.insert((2, 20), 1_500u64);
        after.insert((3, 99), 50u64); // same pid, new create_time → not intersected
        after.insert((4, 40), 9_000u64); // spawn

        // Only (1,10) and (2,20): ΔGF = (1000+2000) − (400+1500) = 1100
        assert_eq!(delta_gf_intersected(&before, &after), 1_100);
    }

    #[test]
    fn delta_gf_intersected_empty_intersection_is_zero() {
        let mut before = HashMap::new();
        before.insert((1, 1), 100u64);
        let mut after = HashMap::new();
        after.insert((2, 2), 50u64);
        assert_eq!(delta_gf_intersected(&before, &after), 0);
        assert_eq!(
            ry_live(delta_gf_intersected(&before, &after), 10),
            None
        );
    }

    #[test]
    fn compress_store_ws_finds_memory_compression_by_image_name() {
        let procs = vec![
            proc("chrome.exe", 100),
            proc("Memory Compression", 1_234_567),
            proc("svchost.exe", 200),
        ];
        assert_eq!(compress_store_ws(&procs), Some(1_234_567));
    }

    #[test]
    fn compress_store_ws_case_insensitive() {
        let procs = vec![proc("memory compression", 42)];
        assert_eq!(compress_store_ws(&procs), Some(42));
    }

    #[test]
    fn compress_store_ws_absent_returns_none() {
        let procs = vec![proc("notepad.exe", 10)];
        assert_eq!(compress_store_ws(&procs), None);
        assert_eq!(compress_store_ws(&[]), None);
    }

    #[test]
    fn compress_store_ws_uses_total_working_set_not_private() {
        let mut p = proc("Memory Compression", 999);
        p.private_working_set_bytes = 1;
        assert_eq!(compress_store_ws(&[p]), Some(999));
    }

    #[test]
    fn live_ntqsi_finds_memory_compression_when_present() {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        let Some(ws) = compress_store_ws(&procs) else {
            return;
        };
        let row = procs
            .iter()
            .find(|p| eq_ignore_ascii_case(&p.image_name, MEMORY_COMPRESSION_IMAGE))
            .expect("name match");
        assert_eq!(row.working_set_bytes, ws);
    }
}
