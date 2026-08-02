//! M4 Task 5 — Job Object backstop integration verify.
//!
//! Live hog + Win32 paths where reliable; mock hooks for arm/drop proof.
//! Task-43 tick-path regression: `runtime::tests::tick_with_groups_soft_trim_follow_backstop_arms_mock_store`
//! (stub trim; arms on `observe_post_trim` follow, not `arm_backstop_if_ready_for_test`).
//! Drop without `TerminateJobObject`: `job_backstop::tests::drop_closes_jobs_without_terminate`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ramjob_core::commit_ratio::{translate_job_limit, MIN_SAMPLES};
use ramjob_core::config::{GroupConfig, RamjobConfig};
use ramjob_core::grouper::{group_processes, AppGroup, GroupMember};
use ramjob_core::job_backstop::{
    pack_job_memory_limit, BackstopError, BackstopHooks, FORBIDDEN_LIMIT_FLAGS,
    JobBackstopStore, JobHandle, PackedJobLimit,
};
use ramjob_core::policy::SystemArm;
use ramjob_core::runtime::Runtime;
use ramjob_core::scanner::{enumerate_processes_with_cache, PathCache};

fn workspace_debug_bin(name: &str) -> PathBuf {    let mut exe = name.to_string();
    if cfg!(windows) && !exe.ends_with(".exe") {
        exe.push_str(".exe");
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(td).join("debug").join(&exe);
    }
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.join("target").join("debug").join(exe)
}

fn spawn_hog(mb: u32, hold_secs: u64) -> Child {
    let bin = workspace_debug_bin("ramjob-hog");
    assert!(
        bin.exists(),
        "missing {}; run `cargo build -p ramjob-hog` first",
        bin.display()
    );
    Command::new(&bin)
        .args([
            "--mode",
            "forget",
            "--mb",
            &mb.to_string(),
            "--hold-secs",
            &hold_secs.to_string(),
        ])
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()))
}

fn wait_for_hog_group(pid: u32) -> AppGroup {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("enumerate");
        let _groups = group_processes(&procs);
        if let Some(g) = procs.iter().find(|p| p.pid == pid).map(|p| AppGroup {
            group_key: "image:ramjob-hog".into(),
            members: vec![GroupMember {
                pid: p.pid,
                create_time: p.create_time,
                private_working_set_bytes: p.private_working_set_bytes,
                private_usage_bytes: p.private_usage_bytes,
            }],
        }) {
            let gf: u64 = g.members.iter().map(|m| m.private_working_set_bytes).sum();
            let ws = procs
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.working_set_bytes)
                .unwrap_or(0);
            if gf > 1_000_000 || ws > 8_000_000 {
                let mut g = g;
                if gf <= 1_000_000 {
                    g.members[0].private_working_set_bytes = ws.max(gf);
                }
                return g;
            }
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for hog pid={pid}");
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn synthetic_group(key: &str, pid: u32, gf: u64, commit: u64) -> AppGroup {
    AppGroup {
        group_key: key.into(),
        members: vec![GroupMember {
            pid,
            create_time: 0,
            private_working_set_bytes: gf,
            private_usage_bytes: commit,
        }],
    }
}

// --- Mock hooks (integration-level arm/drop proof) ---

struct MockJob {
    memory_limit: Option<u64>,
    assigned: HashSet<u32>,
    closed: bool,
}

struct RecordingMockHooks {
    jobs: Arc<Mutex<HashMap<usize, MockJob>>>,
    next_id: Mutex<usize>,
}

impl RecordingMockHooks {
    fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            next_id: Mutex::new(1),
        }
    }

    fn job_id(handle: &JobHandle) -> usize {
        handle.0 .0 as usize
    }
}

impl BackstopHooks for RecordingMockHooks {
    fn create_job(&self) -> Result<JobHandle, BackstopError> {
        let mut next = self.next_id.lock().unwrap();
        let id = *next;
        *next += 1;
        self.jobs.lock().unwrap().insert(
            id,
            MockJob {
                memory_limit: None,
                assigned: HashSet::new(),
                closed: false,
            },
        );
        Ok(JobHandle(windows::Win32::Foundation::HANDLE(
            id as *mut core::ffi::c_void,
        )))
    }

    fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError> {
        let id = Self::job_id(job);
        self.jobs
            .lock()
            .unwrap()
            .get_mut(&id)
            .ok_or_else(|| BackstopError("unknown job".into()))?
            .assigned
            .insert(pid);
        Ok(())
    }

    fn apply_packed_limit(
        &self,
        job: &JobHandle,
        packed: PackedJobLimit,
    ) -> Result<(), BackstopError> {
        let id = Self::job_id(job);
        let mut jobs = self.jobs.lock().unwrap();
        let entry = jobs
            .get_mut(&id)
            .ok_or_else(|| BackstopError("unknown job".into()))?;
        entry.memory_limit = if packed.job_memory_limit > 0 {
            Some(packed.job_memory_limit)
        } else {
            None
        };
        Ok(())
    }

    fn close_job(&self, job: JobHandle) {
        let id = Self::job_id(&job);
        if let Some(entry) = self.jobs.lock().unwrap().get_mut(&id) {
            entry.closed = true;
        }
        std::mem::forget(job);
    }
}

#[test]
fn pack_never_sets_kill_on_job_close() {
    for limit in [None, Some(1), Some(4 * 1024 * 1024 * 1024)] {
        let packed = pack_job_memory_limit(limit);
        assert_eq!(
            packed.limit_flags & FORBIDDEN_LIMIT_FLAGS,
            0,
            "KILL_ON_JOB_CLOSE / BREAKAWAY_OK must stay off"
        );
    }
}

#[test]
fn mock_store_arm_assign_limit_and_drop() {
    let hooks = RecordingMockHooks::new();
    let jobs = Arc::clone(&hooks.jobs);

    let cap = 50_000_000u64;
    let ratio = 1.5;
    let limit = translate_job_limit(cap, ratio);
    let pid = 4242u32;
    let group = "image:test-app";

    {
        let mut store = JobBackstopStore::with_hooks(Box::new(hooks));
        store.assign_pid(group, pid).unwrap();
        store.set_memory_limit(group, limit).unwrap();
        assert!(store.has_group(group));
        assert!(store.assigned_pids(group).unwrap().contains(&pid));
        assert_eq!(store.memory_limit(group), Some(Some(limit)));
    }

    let guard = jobs.lock().unwrap();
    assert_eq!(guard.len(), 1);
    let job = guard.values().next().unwrap();
    assert!(job.closed, "Drop must close handle via close_job, not TerminateJobObject");
    assert!(job.assigned.contains(&pid));
    assert_eq!(job.memory_limit, Some(limit));
    drop(guard);
}

#[test]
fn runtime_pressure_ticks_sample_without_backstop_arm() {
    let cap = 1_000_000u64;
    let key = "hog:mock";
    let pid = 1u32;
    let cfg = RamjobConfig {
        version: 2,
        runaway_multiplier: 3.0,
        overall_limit_bytes: 0,
        groups: vec![GroupConfig {
            key: key.into(),
            cap_bytes: cap,
            always_enforce: true,
        }],
        pause_all: false,
    };
    let mut rt = Runtime::new();
    let t0 = Instant::now();

    // Pressure-range GF samples commit_ratio; no trim/backstop without over-cap escalation.
    for i in 0..MIN_SAMPLES {
        let gf = (0.90 * cap as f64) as u64;
        let apps = vec![synthetic_group(key, pid, gf, gf * 2)];
        rt.tick_with_groups(&cfg, SystemArm::Armed, &apps, t0 + Duration::from_secs(i as u64))
            .unwrap();
    }

    let diag = rt.diagnostics.lines().join("\n");
    assert!(
        !diag.contains("BACKSTOP arm"),
        "pressure-only ticks must not arm backstop:\n{diag}"
    );
}

#[test]
#[cfg(windows)]
fn live_hog_survives_backstop_store_drop() {
    let mut child = spawn_hog(16, 90);
    let pid = child.id();

    let hog = wait_for_hog_group(pid);
    let group = hog.group_key.as_str();
    let cap = 1_000_000u64;
    let limit = translate_job_limit(cap, 1.2);

    let mut store = JobBackstopStore::new();
    if let Err(e) = store.assign_pid(group, pid) {
        eprintln!(
            "SKIP live assign (nested job / CI): {:?}; hog drop survival not proven here",
            e
        );
        let _ = child.kill();
        let _ = child.wait();
        return;
    }
    store.set_memory_limit(group, limit).unwrap();
    drop(store);

    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        child.try_wait().unwrap(),
        None,
        "hog pid={pid} must survive JobBackstopStore Drop (KILL_ON_JOB_CLOSE off)"
    );

    let _ = child.kill();
    let _ = child.wait();
}
