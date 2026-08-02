//! M1 compression gate harness (SPEC §2.3 protocol + §9.2 verdict).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::enforcer::{
    intersect_private_ws, member_key, soft_trim_group_unlocked, with_trim_lock, ExclusionPolicy,
    LiveTrimHooks, MemberKey, TrimContext, TrimOutcome,
};
use crate::grouper::{group_processes, AppGroup, GroupMember};
use crate::scanner::{enumerate_processes_with_cache, PathCache, ProcessRecord};
use crate::yield_math::{
    classify_ry_bench, compress_store_ws, measure_ry_bench, measure_ry_live, GateVerdict,
};

/// Default settle after trim before post-sample (SPEC §2.3).
pub const GATE_SETTLE: Duration = Duration::from_secs(3);

/// Who to measure: resolved from the gate's own pre-sample (one enumerate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateTarget {
    Image(String),
    Pid(u32),
}

/// One measured gate run against a single group.
#[derive(Debug, Clone)]
pub struct GateMeasurement {
    pub group_key: String,
    pub target_pids: Vec<u32>,
    pub trimmed_pids: Vec<u32>,
    pub excluded_pids: Vec<u32>,
    pub rate_limited: bool,
    pub gf0: u64,
    pub gf1: u64,
    pub available0: u64,
    pub available1: u64,
    pub cs0: Option<u64>,
    pub cs1: Option<u64>,
    pub ry_bench: Option<f64>,
    pub ry_live: Option<f64>,
    pub verdict: Option<GateVerdict>,
    pub settle: Duration,
    pub trim_errors: Vec<(u32, String)>,
}

impl GateMeasurement {
    /// Markdown body for `.superpowers/sdd/m1-gate-results.md`.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# M1 gate results\n\n");
        out.push_str(&format!(
            "**Classification (Ry_bench):** {}\n\n",
            self.verdict
                .map(|v| v.as_str().to_string())
                .unwrap_or_else(|| "n/a (ΔGF == 0)".into())
        ));
        out.push_str("## Synthetic hog run\n\n");
        out.push_str(&format!("- group_key: `{}`\n", self.group_key));
        out.push_str(&format!("- target_pids: {:?}\n", self.target_pids));
        out.push_str(&format!("- trimmed_pids: {:?}\n", self.trimmed_pids));
        out.push_str(&format!("- excluded_pids: {:?}\n", self.excluded_pids));
        out.push_str(&format!("- rate_limited: {}\n", self.rate_limited));
        out.push_str(&format!("- settle: {} s\n", self.settle.as_secs()));
        out.push_str(&format!(
            "- gf0 / gf1 (intersected private WS): {} / {} bytes\n",
            self.gf0, self.gf1
        ));
        out.push_str(&format!(
            "- available0 / available1: {} / {} bytes\n",
            self.available0, self.available1
        ));
        out.push_str(&format!(
            "- CompressStore cs0 / cs1: {} / {}\n",
            fmt_opt_u64(self.cs0),
            fmt_opt_u64(self.cs1)
        ));
        out.push_str(&format!("- **Ry_bench:** {}\n", fmt_opt_f64(self.ry_bench)));
        out.push_str(&format!("- **Ry_live:** {}\n", fmt_opt_f64(self.ry_live)));
        out.push_str("\n## Thresholds (SPEC §9.2)\n\n");
        out.push_str("- Pass: Ry_bench ≥ 0.5\n");
        out.push_str("- Marginal: 0.3 ≤ Ry_bench < 0.5\n");
        out.push_str("- Fail: Ry_bench < 0.3\n");
        out.push_str("\n## Product pivot\n\n");
        out.push_str(
            "This file reports only. Fail/Marginal does **not** silently change product shape.\n",
        );
        if !self.trim_errors.is_empty() {
            out.push_str("\n## Trim errors\n\n");
            for (pid, err) in &self.trim_errors {
                out.push_str(&format!("- pid {pid}: {err}\n"));
            }
        }
        out
    }
}

fn fmt_opt_u64(v: Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "n/a".into())
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|n| format!("{n:.4}"))
        .unwrap_or_else(|| "n/a".into())
}

/// Total and available physical bytes from `GlobalMemoryStatusEx`.
pub fn phys_memory() -> Result<(u64, u64), String> {
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        GlobalMemoryStatusEx(&mut status).map_err(|e| format!("GlobalMemoryStatusEx: {e}"))?;
        Ok((status.ullTotalPhys, status.ullAvailPhys))
    }
}

/// Physical available bytes from `GlobalMemoryStatusEx` (`ullAvailPhys`).
pub fn available_phys_bytes() -> Result<u64, String> {
    Ok(phys_memory()?.1)
}

/// Strip trailing `.exe` and compare case-insensitively.
pub fn image_name_matches(image_name: &str, needle: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim();
        let lower = s.to_ascii_lowercase();
        lower
            .strip_suffix(".exe")
            .unwrap_or(&lower)
            .to_string()
    };
    norm(image_name) == norm(needle)
}

/// Find the group that owns `pid`.
pub fn find_group_for_pid(groups: &[AppGroup], pid: u32) -> Option<&AppGroup> {
    groups.iter().find(|g| g.members.iter().any(|m| m.pid == pid))
}

/// Find the group containing a process whose image matches `image` (e.g. `ramjob-hog`).
pub fn find_group_for_image<'a>(
    groups: &'a [AppGroup],
    procs: &[ProcessRecord],
    image: &str,
) -> Option<&'a AppGroup> {
    let pid = procs
        .iter()
        .find(|p| image_name_matches(&p.image_name, image))
        .map(|p| p.pid)?;
    find_group_for_pid(groups, pid)
}

/// Refresh member private WS from a fresh sweep; drop exited/replaced members.
pub fn refresh_group_from_procs(group: &AppGroup, procs: &[ProcessRecord]) -> AppGroup {
    let by_key: HashMap<MemberKey, &ProcessRecord> = procs
        .iter()
        .map(|p| ((p.pid, p.create_time), p))
        .collect();
    let mut members: Vec<GroupMember> = group
        .members
        .iter()
        .filter_map(|m| {
            let p = by_key.get(&member_key(m))?;
            Some(GroupMember {
                pid: p.pid,
                create_time: p.create_time,
                private_working_set_bytes: p.private_working_set_bytes,
            })
        })
        .collect();
    members.sort_unstable_by_key(|m| m.pid);
    AppGroup {
        group_key: group.group_key.clone(),
        members,
    }
}

/// Pure assembly of a measurement from pre/post samples + trim outcome.
pub fn measurement_from_samples(
    group_key: String,
    target_pids: Vec<u32>,
    before: &HashMap<MemberKey, u64>,
    after: &HashMap<MemberKey, u64>,
    available0: u64,
    available1: u64,
    cs0: Option<u64>,
    cs1: Option<u64>,
    trim: &TrimOutcome,
    settle: Duration,
) -> GateMeasurement {
    let (gf0, gf1) = intersect_private_ws(before, after);
    let ry_bench = measure_ry_bench(available0, available1, gf0, gf1);
    let ry_live = match (cs0, cs1) {
        (Some(a), Some(b)) => measure_ry_live(gf0, gf1, a, b),
        _ => None,
    };
    let verdict = ry_bench.map(classify_ry_bench);
    GateMeasurement {
        group_key,
        target_pids,
        trimmed_pids: trim.trimmed_pids.clone(),
        excluded_pids: trim.excluded_pids.clone(),
        rate_limited: trim.rate_limited,
        gf0,
        gf1,
        available0,
        available1,
        cs0,
        cs1,
        ry_bench,
        ry_live,
        verdict,
        settle,
        trim_errors: trim.trim_errors.clone(),
    }
}

fn require_real_trim(trim: &TrimOutcome) -> Result<(), String> {
    if trim.rate_limited {
        return Err("gate skipped: group rate-limited (no trim)".into());
    }
    if trim.trimmed_pids.is_empty() {
        return Err("gate skipped: no PIDs trimmed".into());
    }
    Ok(())
}

fn measure_under_lock(
    fresh: AppGroup,
    ctx: &mut TrimContext<'_>,
    settle: Duration,
    available0: u64,
    cs0: Option<u64>,
    cache: &mut PathCache,
) -> Result<GateMeasurement, String> {
    if fresh.members.is_empty() {
        return Err("target group has no live members at pre-sample".into());
    }
    let before: HashMap<MemberKey, u64> = fresh
        .members
        .iter()
        .map(|m| (member_key(m), m.private_working_set_bytes))
        .collect();
    let target_pids = fresh.member_pids();

    let trim = soft_trim_group_unlocked(&fresh, ctx);
    require_real_trim(&trim)?;

    std::thread::sleep(settle);

    let procs1 = enumerate_processes_with_cache(cache)
        .map_err(|s| format!("NtQuerySystemInformation post-sample failed ({s:?})"))?;
    let available1 = available_phys_bytes()?;
    let cs1 = compress_store_ws(&procs1);
    let after: HashMap<MemberKey, u64> = procs1
        .into_iter()
        .filter_map(|p| {
            let key = (p.pid, p.create_time);
            if before.contains_key(&key) {
                Some((key, p.private_working_set_bytes))
            } else {
                None
            }
        })
        .collect();

    Ok(measurement_from_samples(
        fresh.group_key,
        target_pids,
        &before,
        &after,
        available0,
        available1,
        cs0,
        cs1,
        &trim,
        settle,
    ))
}

/// Run SPEC §2.3 sample → trim → settle → sample on `group` under `TRIM_LOCK`.
pub fn run_gate_on_group(
    group: &AppGroup,
    ctx: &mut TrimContext<'_>,
    settle: Duration,
) -> Result<GateMeasurement, String> {
    with_trim_lock(|| {
        let mut cache = PathCache::new();
        let procs0 = enumerate_processes_with_cache(&mut cache)
            .map_err(|s| format!("NtQuerySystemInformation pre-sample failed ({s:?})"))?;
        let available0 = available_phys_bytes()?;
        let cs0 = compress_store_ws(&procs0);
        let fresh = refresh_group_from_procs(group, &procs0);
        measure_under_lock(fresh, ctx, settle, available0, cs0, &mut cache)
    })
}

/// Resolve `target` from one pre-sample, then run the §2.3 protocol under `TRIM_LOCK`.
pub fn run_gate_for_target(
    target: &GateTarget,
    ctx: &mut TrimContext<'_>,
    settle: Duration,
) -> Result<GateMeasurement, String> {
    with_trim_lock(|| {
        let mut cache = PathCache::new();
        let procs0 = enumerate_processes_with_cache(&mut cache)
            .map_err(|s| format!("NtQuerySystemInformation pre-sample failed ({s:?})"))?;
        let available0 = available_phys_bytes()?;
        let cs0 = compress_store_ws(&procs0);
        let groups = group_processes(&procs0);
        let group = match target {
            GateTarget::Image(name) => find_group_for_image(&groups, &procs0, name),
            GateTarget::Pid(pid) => find_group_for_pid(&groups, *pid),
        };
        let Some(group) = group else {
            return Err(match target {
                GateTarget::Image(name) => format!("no group found for image '{name}'"),
                GateTarget::Pid(pid) => format!("no group found for pid {pid}"),
            });
        };
        // Groups were built from procs0; members already carry current private WS.
        let fresh = AppGroup {
            group_key: group.group_key.clone(),
            members: group.members.clone(),
        };
        measure_under_lock(fresh, ctx, settle, available0, cs0, &mut cache)
    })
}

/// Convenience: no exclusion + empty rate map + live hooks.
pub fn run_live_gate(target: GateTarget, settle: Duration) -> Result<GateMeasurement, String> {
    let hooks = LiveTrimHooks;
    let mut rates = HashMap::new();
    let mut ctx = TrimContext {
        hooks: &hooks,
        rate_limits: &mut rates,
        now: Instant::now(),
        exclusion: ExclusionPolicy::None,
    };
    run_gate_for_target(&target, &mut ctx, settle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(pid: u32, ctime: i64, ws: u64) -> GroupMember {
        GroupMember {
            pid,
            create_time: ctime,
            private_working_set_bytes: ws,
        }
    }

    fn group(key: &str, members: Vec<GroupMember>) -> AppGroup {
        AppGroup {
            group_key: key.into(),
            members,
        }
    }

    fn proc(pid: u32, image: &str, ctime: i64, private_ws: u64) -> ProcessRecord {
        ProcessRecord {
            pid,
            ppid: 0,
            session_id: 1,
            image_name: image.into(),
            private_working_set_bytes: private_ws,
            working_set_bytes: private_ws,
            create_time: ctime,
            image_path: None,
        }
    }

    #[test]
    fn image_name_matches_strips_exe_case_insensitive() {
        assert!(image_name_matches("ramjob-hog.exe", "ramjob-hog"));
        assert!(image_name_matches("RamJob-Hog", "ramjob-hog.exe"));
        assert!(!image_name_matches("chrome.exe", "ramjob-hog"));
    }

    #[test]
    fn find_group_for_image_and_pid() {
        let g = group("hog-key", vec![member(42, 9, 100)]);
        let groups = vec![g];
        let procs = vec![proc(42, "ramjob-hog.exe", 9, 100)];
        assert_eq!(
            find_group_for_image(&groups, &procs, "ramjob-hog")
                .unwrap()
                .group_key,
            "hog-key"
        );
        assert_eq!(
            find_group_for_pid(&groups, 42).unwrap().group_key,
            "hog-key"
        );
        assert!(find_group_for_pid(&groups, 99).is_none());
    }

    #[test]
    fn measurement_classifies_pass_from_synthetic_deltas() {
        let before = HashMap::from([((1, 1), 1_000u64)]);
        let after = HashMap::from([((1, 1), 0u64)]);
        let trim = TrimOutcome {
            trimmed_pids: vec![1],
            excluded_pids: vec![],
            rate_limited: false,
            trim_errors: vec![],
        };
        let m = measurement_from_samples(
            "g".into(),
            vec![1],
            &before,
            &after,
            100,
            700,
            Some(10),
            Some(10),
            &trim,
            GATE_SETTLE,
        );
        assert!((m.ry_bench.unwrap() - 0.6).abs() < 1e-9);
        assert_eq!(m.verdict, Some(GateVerdict::Pass));
        assert!((m.ry_live.unwrap() - 1.0).abs() < 1e-9);
        let md = m.to_markdown();
        assert!(md.contains("**Classification (Ry_bench):** Pass"));
        assert!(md.contains("does **not** silently change product shape"));
    }

    #[test]
    fn measurement_classifies_fail_and_marginal() {
        let before = HashMap::from([((1, 1), 1000u64)]);
        let after = HashMap::from([((1, 1), 0u64)]);
        let trim = TrimOutcome {
            trimmed_pids: vec![1],
            excluded_pids: vec![],
            rate_limited: false,
            trim_errors: vec![],
        };
        let fail = measurement_from_samples(
            "g".into(),
            vec![1],
            &before,
            &after,
            100,
            200,
            None,
            None,
            &trim,
            GATE_SETTLE,
        );
        assert_eq!(fail.verdict, Some(GateVerdict::Fail));

        let marg = measurement_from_samples(
            "g".into(),
            vec![1],
            &before,
            &after,
            100,
            500,
            None,
            None,
            &trim,
            GATE_SETTLE,
        );
        assert_eq!(marg.verdict, Some(GateVerdict::Marginal));
    }

    #[test]
    fn require_real_trim_fails_closed() {
        let rate = TrimOutcome {
            trimmed_pids: vec![],
            excluded_pids: vec![],
            rate_limited: true,
            trim_errors: vec![],
        };
        assert!(require_real_trim(&rate).unwrap_err().contains("rate-limited"));

        let empty = TrimOutcome {
            trimmed_pids: vec![],
            excluded_pids: vec![1],
            rate_limited: false,
            trim_errors: vec![],
        };
        assert!(require_real_trim(&empty).unwrap_err().contains("no PIDs trimmed"));

        let ok = TrimOutcome {
            trimmed_pids: vec![1],
            excluded_pids: vec![],
            rate_limited: false,
            trim_errors: vec![],
        };
        assert!(require_real_trim(&ok).is_ok());
    }

    #[test]
    fn refresh_group_updates_private_ws() {
        let g = group("k", vec![member(1, 5, 10)]);
        let procs = vec![proc(1, "hog.exe", 5, 999)];
        let refreshed = refresh_group_from_procs(&g, &procs);
        assert_eq!(refreshed.members[0].private_working_set_bytes, 999);
    }
}
