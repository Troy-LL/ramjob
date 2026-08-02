//! Soft-trim pass for an [`AppGroup`] (SPEC §4.2 / gaps §3.1).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Memory::{
    SetProcessWorkingSetSizeEx, QUOTA_LIMITS_HARDWS_MIN_DISABLE,
};
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::grouper::{AppGroup, GroupMember};
use crate::scanner::{enumerate_processes_with_cache, PathCache};

/// One soft-trim pass per group per this interval (SPEC §4.2).
pub const TRIM_RATE_LIMIT: Duration = Duration::from_secs(20);

/// Process-wide: only one trim in flight (SPEC §2.3). Intentional global.
static TRIM_LOCK: Mutex<()> = Mutex::new(());

/// `(pid, create_time)` identity used for ΔGF intersection.
pub type MemberKey = (u32, i64);

/// Failure opening a process or issuing a working-set trim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimError(pub String);

/// Whether soft-trim skips foreground / visible top-level owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionPolicy {
    /// Daemon default: protect interactive owners.
    ProtectInteractive,
    /// Bench / gate: trim every member.
    None,
}

/// Injectable OS surface for unit tests.
pub trait TrimHooks {
    fn foreground_pid(&self) -> Option<u32>;
    fn visible_toplevel_owner_pids(&self) -> HashSet<u32>;
    fn trim_working_set(&self, pid: u32) -> Result<(), TrimError>;
    /// Private WS for keys still alive; omit exited or replaced PIDs.
    fn sample_private_ws(&self, keys: &[MemberKey]) -> Result<HashMap<MemberKey, u64>, TrimError>;
}

/// Caller-owned trim inputs. Rate map is not process-global (lesson: no premature cache).
pub struct TrimContext<'a> {
    pub hooks: &'a dyn TrimHooks,
    pub rate_limits: &'a mut HashMap<String, Instant>,
    pub now: Instant,
    pub exclusion: ExclusionPolicy,
}

/// Result of one soft-trim attempt (trim-only; ΔGF belongs to the measurement owner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimOutcome {
    pub trimmed_pids: Vec<u32>,
    pub excluded_pids: Vec<u32>,
    pub rate_limited: bool,
    pub trim_errors: Vec<(u32, String)>,
}

pub fn member_key(m: &GroupMember) -> MemberKey {
    (m.pid, m.create_time)
}

/// Sum private WS only for keys present in both maps (SPEC §2.3).
pub fn intersect_private_ws(
    before: &HashMap<MemberKey, u64>,
    after: &HashMap<MemberKey, u64>,
) -> (u64, u64) {
    let mut gf0 = 0u64;
    let mut gf1 = 0u64;
    for (key, &wb) in before {
        if let Some(&wa) = after.get(key) {
            gf0 = gf0.saturating_add(wb);
            gf1 = gf1.saturating_add(wa);
        }
    }
    (gf0, gf1)
}

/// Hold process-wide `TRIM_LOCK` for the duration of `f` (SPEC §2.3).
pub fn with_trim_lock<R>(f: impl FnOnce() -> R) -> R {
    let _guard = TRIM_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f()
}

/// Soft-trim without acquiring `TRIM_LOCK`. Caller must hold it (or accept races).
pub fn soft_trim_group_unlocked(group: &AppGroup, ctx: &mut TrimContext<'_>) -> TrimOutcome {
    if let Some(last) = ctx.rate_limits.get(&group.group_key) {
        if ctx.now.saturating_duration_since(*last) < TRIM_RATE_LIMIT {
            return TrimOutcome {
                trimmed_pids: Vec::new(),
                excluded_pids: Vec::new(),
                rate_limited: true,
                trim_errors: Vec::new(),
            };
        }
    }

    let mut excluded = Vec::new();
    let mut targets: Vec<&GroupMember> = Vec::new();
    match ctx.exclusion {
        ExclusionPolicy::None => {
            targets.extend(group.members.iter());
        }
        ExclusionPolicy::ProtectInteractive => {
            let fg = ctx.hooks.foreground_pid();
            let visible = ctx.hooks.visible_toplevel_owner_pids();
            for m in &group.members {
                let skip = fg == Some(m.pid) || visible.contains(&m.pid);
                if skip {
                    excluded.push(m.pid);
                } else {
                    targets.push(m);
                }
            }
        }
    }
    targets.sort_by(|a, b| {
        b.private_working_set_bytes
            .cmp(&a.private_working_set_bytes)
            .then_with(|| a.pid.cmp(&b.pid))
    });

    let mut trimmed_pids = Vec::new();
    let mut trim_errors = Vec::new();
    for m in &targets {
        match ctx.hooks.trim_working_set(m.pid) {
            Ok(()) => trimmed_pids.push(m.pid),
            Err(e) => trim_errors.push((m.pid, e.0)),
        }
    }

    ctx.rate_limits
        .insert(group.group_key.clone(), ctx.now);

    TrimOutcome {
        trimmed_pids,
        excluded_pids: excluded,
        rate_limited: false,
        trim_errors,
    }
}

/// Soft-trim group members under `TRIM_LOCK` (daemon path).
pub fn soft_trim_group(group: &AppGroup, ctx: &mut TrimContext<'_>) -> TrimOutcome {
    with_trim_lock(|| soft_trim_group_unlocked(group, ctx))
}

/// Live Win32 hooks: foreground window, visible top-level owners, EmptyWorkingSet path.
pub struct LiveTrimHooks;

impl TrimHooks for LiveTrimHooks {
    fn foreground_pid(&self) -> Option<u32> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                None
            } else {
                Some(pid)
            }
        }
    }

    fn visible_toplevel_owner_pids(&self) -> HashSet<u32> {
        let mut owners = HashSet::new();
        unsafe {
            let _ = EnumWindows(
                Some(enum_visible_owners),
                LPARAM(&mut owners as *mut HashSet<u32> as isize),
            );
        }
        owners
    }

    fn trim_working_set(&self, pid: u32) -> Result<(), TrimError> {
        unsafe {
            let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, pid)
                .map_err(|e| TrimError(format!("OpenProcess({pid}): {e}")))?;
            let set_result = SetProcessWorkingSetSizeEx(
                handle,
                usize::MAX,
                usize::MAX,
                QUOTA_LIMITS_HARDWS_MIN_DISABLE,
            );
            let empty_result = EmptyWorkingSet(handle);
            let _ = CloseHandle(handle);
            match (set_result, empty_result) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(_)) => Ok(()),
                (Err(_), Ok(())) => Ok(()),
                (Err(e), Err(_)) => Err(TrimError(format!("trim({pid}): {e}"))),
            }
        }
    }

    fn sample_private_ws(&self, keys: &[MemberKey]) -> Result<HashMap<MemberKey, u64>, TrimError> {
        let want: HashSet<MemberKey> = keys.iter().copied().collect();
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).map_err(|s| {
            TrimError(format!("NtQuerySystemInformation sample failed ({s:?})"))
        })?;
        Ok(procs
            .into_iter()
            .filter_map(|p| {
                let key = (p.pid, p.create_time);
                if want.contains(&key) {
                    Some((key, p.private_working_set_bytes))
                } else {
                    None
                }
            })
            .collect())
    }
}

unsafe extern "system" fn enum_visible_owners(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let owners = &mut *(lparam.0 as *mut HashSet<u32>);
    if IsWindowVisible(hwnd).as_bool() {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
            owners.insert(pid);
        }
    }
    BOOL(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    struct MockHooks {
        fg: Option<u32>,
        visible: HashSet<u32>,
        trimmed: RefCell<Vec<u32>>,
        after: HashMap<MemberKey, u64>,
        sample_err: Option<&'static str>,
    }

    impl TrimHooks for MockHooks {
        fn foreground_pid(&self) -> Option<u32> {
            self.fg
        }

        fn visible_toplevel_owner_pids(&self) -> HashSet<u32> {
            self.visible.clone()
        }

        fn trim_working_set(&self, pid: u32) -> Result<(), TrimError> {
            self.trimmed.borrow_mut().push(pid);
            Ok(())
        }

        fn sample_private_ws(&self, keys: &[MemberKey]) -> Result<HashMap<MemberKey, u64>, TrimError> {
            if let Some(msg) = self.sample_err {
                return Err(TrimError(msg.into()));
            }
            Ok(keys
                .iter()
                .filter_map(|k| self.after.get(k).map(|&v| (*k, v)))
                .collect())
        }
    }

    fn member(pid: u32, ctime: i64, ws: u64) -> GroupMember {
        GroupMember {
            pid,
            create_time: ctime,
            private_working_set_bytes: ws,
            private_usage_bytes: ws,
        }
    }

    fn group(key: &str, members: Vec<GroupMember>) -> AppGroup {
        AppGroup {
            group_key: key.into(),
            members,
        }
    }

    fn ctx<'a>(
        hooks: &'a MockHooks,
        rates: &'a mut HashMap<String, Instant>,
        now: Instant,
        exclusion: ExclusionPolicy,
    ) -> TrimContext<'a> {
        TrimContext {
            hooks,
            rate_limits: rates,
            now,
            exclusion,
        }
    }

    #[test]
    fn excludes_foreground_and_visible_owners_trims_rest_by_ws_desc() {
        let g = group(
            "app",
            vec![
                member(1, 10, 300),
                member(2, 20, 100),
                member(3, 30, 500),
                member(4, 40, 200),
            ],
        );
        let hooks = MockHooks {
            fg: Some(3),
            visible: HashSet::from([2]),
            trimmed: RefCell::new(Vec::new()),
            after: HashMap::new(),
            sample_err: None,
        };
        let mut rates = HashMap::new();
        let mut trim_ctx = ctx(
            &hooks,
            &mut rates,
            Instant::now(),
            ExclusionPolicy::ProtectInteractive,
        );
        let out = soft_trim_group(&g, &mut trim_ctx);
        assert_eq!(out.trimmed_pids, vec![1, 4]);
        assert_eq!(
            {
                let mut e = out.excluded_pids.clone();
                e.sort_unstable();
                e
            },
            vec![2, 3]
        );
        assert!(!out.rate_limited);
        assert_eq!(hooks.trimmed.borrow().as_slice(), &[1, 4]);
    }

    #[test]
    fn exclusion_none_trims_all_members() {
        let g = group("app", vec![member(1, 10, 100), member(2, 20, 200)]);
        let hooks = MockHooks {
            fg: Some(1),
            visible: HashSet::from([2]),
            trimmed: RefCell::new(Vec::new()),
            after: HashMap::new(),
            sample_err: None,
        };
        let mut rates = HashMap::new();
        let mut trim_ctx = ctx(&hooks, &mut rates, Instant::now(), ExclusionPolicy::None);
        let out = soft_trim_group(&g, &mut trim_ctx);
        assert_eq!(out.trimmed_pids, vec![2, 1]);
        assert!(out.excluded_pids.is_empty());
    }

    #[test]
    fn rate_limit_blocks_second_pass_within_20s() {
        let g = group("app", vec![member(1, 1, 100)]);
        let hooks = MockHooks {
            fg: None,
            visible: HashSet::new(),
            trimmed: RefCell::new(Vec::new()),
            after: HashMap::new(),
            sample_err: None,
        };
        let t0 = Instant::now();
        let mut rates = HashMap::new();
        let mut trim_ctx = ctx(
            &hooks,
            &mut rates,
            t0,
            ExclusionPolicy::ProtectInteractive,
        );
        let first = soft_trim_group(&g, &mut trim_ctx);
        assert!(!first.rate_limited);
        assert_eq!(first.trimmed_pids, vec![1]);

        trim_ctx.now = t0 + Duration::from_secs(19);
        let second = soft_trim_group(&g, &mut trim_ctx);
        assert!(second.rate_limited);
        assert!(second.trimmed_pids.is_empty());
        assert_eq!(hooks.trimmed.borrow().len(), 1);

        trim_ctx.now = t0 + TRIM_RATE_LIMIT;
        let third = soft_trim_group(&g, &mut trim_ctx);
        assert!(!third.rate_limited);
        assert_eq!(third.trimmed_pids, vec![1]);
        assert_eq!(hooks.trimmed.borrow().len(), 2);
    }

    #[test]
    fn intersect_skips_exited_and_replaced_pids() {
        let before = HashMap::from([
            ((1, 100), 1000u64),
            ((2, 200), 2000u64),
            ((3, 300), 3000u64),
        ]);
        let after = HashMap::from([((1, 100), 400u64), ((3, 999), 50u64)]);
        let (gf0, gf1) = intersect_private_ws(&before, &after);
        assert_eq!(gf0, 1000);
        assert_eq!(gf1, 400);
    }

    #[test]
    fn sample_private_ws_propagates_errors() {
        let hooks = MockHooks {
            fg: None,
            visible: HashSet::new(),
            trimmed: RefCell::new(Vec::new()),
            after: HashMap::new(),
            sample_err: Some("sweep failed"),
        };
        let err = hooks.sample_private_ws(&[(1, 1)]).unwrap_err();
        assert_eq!(err.0, "sweep failed");
    }

    #[test]
    fn trim_lock_serializes_concurrent_passes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        fn spawn_trim(
            key: &str,
            pid: u32,
            active: Arc<AtomicUsize>,
            max_active: Arc<AtomicUsize>,
        ) -> thread::JoinHandle<()> {
            let key = key.to_string();
            thread::spawn(move || {
                struct CountingHooks {
                    active: Arc<AtomicUsize>,
                    max_active: Arc<AtomicUsize>,
                    pid: u32,
                }
                impl TrimHooks for CountingHooks {
                    fn foreground_pid(&self) -> Option<u32> {
                        None
                    }
                    fn visible_toplevel_owner_pids(&self) -> HashSet<u32> {
                        HashSet::new()
                    }
                    fn trim_working_set(&self, pid: u32) -> Result<(), TrimError> {
                        let n = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                        self.max_active.fetch_max(n, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(40));
                        self.active.fetch_sub(1, Ordering::SeqCst);
                        assert_eq!(pid, self.pid);
                        Ok(())
                    }
                    fn sample_private_ws(
                        &self,
                        keys: &[MemberKey],
                    ) -> Result<HashMap<MemberKey, u64>, TrimError> {
                        Ok(keys.iter().map(|k| (*k, 1u64)).collect())
                    }
                }
                let hooks = CountingHooks {
                    active,
                    max_active,
                    pid,
                };
                let g = group(&key, vec![member(pid, pid as i64, 100)]);
                let mut rates = HashMap::new();
                let mut trim_ctx = TrimContext {
                    hooks: &hooks,
                    rate_limits: &mut rates,
                    now: Instant::now(),
                    exclusion: ExclusionPolicy::ProtectInteractive,
                };
                soft_trim_group(&g, &mut trim_ctx);
            })
        }

        let t1 = spawn_trim("a", 1, Arc::clone(&active), Arc::clone(&max_active));
        let t2 = spawn_trim("b", 2, Arc::clone(&active), Arc::clone(&max_active));
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(
            max_active.load(Ordering::SeqCst),
            1,
            "trim_lock must keep concurrent soft_trim_group calls from overlapping"
        );
    }

    #[test]
    fn trim_rate_limit_constant_is_20_seconds() {
        assert_eq!(TRIM_RATE_LIMIT, Duration::from_secs(20));
    }

    #[test]
    fn live_foreground_visible_apis_smoke() {
        let hooks = LiveTrimHooks;
        let _fg = hooks.foreground_pid();
        let owners = hooks.visible_toplevel_owner_pids();
        assert!(
            !owners.is_empty() || _fg.is_some(),
            "expected a foreground PID or visible top-level owner on an interactive session"
        );
    }

    #[test]
    #[ignore = "needs PROCESS_SET_QUOTA on a willing target; privileges often block"]
    fn live_trim_ignored_without_privileges() {
        let hooks = LiveTrimHooks;
        let self_pid = std::process::id();
        let _ = hooks.trim_working_set(self_pid);
    }
}
