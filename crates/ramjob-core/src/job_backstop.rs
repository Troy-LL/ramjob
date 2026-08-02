//! Job Object hard backstop store (SPEC §4.2).
//!
//! One job per group key; `KILL_ON_JOB_CLOSE` and `BREAKAWAY_OK` stay off.

use std::collections::{HashMap, HashSet};

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT,
    JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JobObjectExtendedLimitInformation,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
};

/// Limit flags that must never appear on a RamJob backstop job.
pub const FORBIDDEN_LIMIT_FLAGS: u32 =
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0 | JOB_OBJECT_LIMIT_BREAKAWAY_OK.0;

/// Packed limit state for unit tests and Win32 apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedJobLimit {
    pub limit_flags: u32,
    pub job_memory_limit: u64,
}

/// Build `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` limit fields for a memory cap.
///
/// `None` clears `JOB_OBJECT_LIMIT_JOB_MEMORY` (unlimited). Forbidden bits are never set.
pub fn pack_job_memory_limit(limit_bytes: Option<u64>) -> PackedJobLimit {
    let mut flags = JOB_OBJECT_LIMIT(0);
    let mut job_memory_limit = 0u64;
    if let Some(bytes) = limit_bytes {
        flags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
        job_memory_limit = bytes;
    }
    debug_assert_eq!(flags.0 & FORBIDDEN_LIMIT_FLAGS, 0);
    PackedJobLimit {
        limit_flags: flags.0,
        job_memory_limit,
    }
}

fn extended_limit_from_packed(packed: PackedJobLimit) -> JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT(packed.limit_flags);
    info.JobMemoryLimit = packed.job_memory_limit as usize;
    info
}

/// Job Object assign / limit failure (nested job → runtime soft-only degrade).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackstopError(pub String);

/// Injectable OS surface for unit tests.
pub trait BackstopHooks {
    fn create_job(&self) -> Result<JobHandle, BackstopError>;
    fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError>;
    fn apply_packed_limit(
        &self,
        job: &JobHandle,
        packed: PackedJobLimit,
    ) -> Result<(), BackstopError>;
    fn close_job(&self, job: JobHandle);
}

/// Owning job handle; closed on drop without `TerminateJobObject`.
#[derive(Debug)]
pub struct JobHandle(pub HANDLE);

impl Drop for JobHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

struct GroupJob {
    handle: JobHandle,
    assigned_pids: HashSet<u32>,
    memory_limit: Option<u64>,
}

/// Per-group Job Object store: create, assign, set/clear `JobMemoryLimit`.
pub struct JobBackstopStore {
    hooks: Box<dyn BackstopHooks>,
    groups: HashMap<String, GroupJob>,
}

impl JobBackstopStore {
    pub fn new() -> Self {
        Self::with_hooks(Box::new(Win32BackstopHooks))
    }

    pub fn with_hooks(hooks: Box<dyn BackstopHooks>) -> Self {
        Self {
            hooks,
            groups: HashMap::new(),
        }
    }

    /// Whether a group already has a job object.
    pub fn has_group(&self, group: &str) -> bool {
        self.groups.contains_key(group)
    }

    /// Current packed limit for a group (`None` = unlimited / no job).
    pub fn memory_limit(&self, group: &str) -> Option<Option<u64>> {
        self.groups.get(group).map(|g| g.memory_limit)
    }

    /// PIDs successfully assigned to the group's job.
    pub fn assigned_pids(&self, group: &str) -> Option<&HashSet<u32>> {
        self.groups.get(group).map(|g| &g.assigned_pids)
    }

    /// Set `JobMemoryLimit` for `group` (creates the job lazily).
    pub fn set_memory_limit(&mut self, group: &str, bytes: u64) -> Result<(), BackstopError> {
        self.ensure_group_job(group)?;
        let packed = pack_job_memory_limit(Some(bytes));
        let handle = self.groups.get(group).unwrap().handle.0;
        let wrapper = JobHandle(handle);
        self.hooks.apply_packed_limit(&wrapper, packed)?;
        std::mem::forget(wrapper);
        self.groups.get_mut(group).unwrap().memory_limit = Some(bytes);
        Ok(())
    }

    /// Raise limit to unlimited for `group` (job handle kept for reuse).
    pub fn clear_limit(&mut self, group: &str) -> Result<(), BackstopError> {
        let Some(job) = self.groups.get_mut(group) else {
            return Ok(());
        };
        let packed = pack_job_memory_limit(None);
        self.hooks.apply_packed_limit(&job.handle, packed)?;
        job.memory_limit = None;
        Ok(())
    }

    /// Assign `pid` to the group's job. Returns `Err` on nested-job failure.
    pub fn assign_pid(&mut self, group: &str, pid: u32) -> Result<(), BackstopError> {
        self.ensure_group_job(group)?;
        if self.groups.get(group).unwrap().assigned_pids.contains(&pid) {
            return Ok(());
        }
        let handle = self.groups.get(group).unwrap().handle.0;
        let wrapper = JobHandle(handle);
        self.hooks.assign_process(&wrapper, pid)?;
        std::mem::forget(wrapper);
        self.groups.get_mut(group).unwrap().assigned_pids.insert(pid);
        Ok(())
    }

    fn ensure_group_job(&mut self, group: &str) -> Result<&mut GroupJob, BackstopError> {
        if !self.groups.contains_key(group) {
            let handle = self.hooks.create_job()?;
            self.groups.insert(
                group.to_string(),
                GroupJob {
                    handle,
                    assigned_pids: HashSet::new(),
                    memory_limit: None,
                },
            );
        }
        Ok(self.groups.get_mut(group).expect("inserted above"))
    }
}

impl Default for JobBackstopStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for JobBackstopStore {
    fn drop(&mut self) {
        while let Some((_, job)) = self.groups.drain().next() {
            self.hooks.close_job(job.handle);
        }
    }
}

struct Win32BackstopHooks;

impl BackstopHooks for Win32BackstopHooks {
    fn create_job(&self) -> Result<JobHandle, BackstopError> {
        unsafe {
            CreateJobObjectW(None, windows::core::PCWSTR::null())
                .map(JobHandle)
                .map_err(|e| BackstopError(format!("CreateJobObjectW: {e}")))
        }
    }

    fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError> {
        unsafe {
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)
                .map_err(|e| BackstopError(format!("OpenProcess pid={pid}: {e}")))?;
            let result = AssignProcessToJobObject(job.0, process)
                .map_err(|e| BackstopError(format!("AssignProcessToJobObject pid={pid}: {e}")));
            let _ = CloseHandle(process);
            result
        }
    }

    fn apply_packed_limit(
        &self,
        job: &JobHandle,
        packed: PackedJobLimit,
    ) -> Result<(), BackstopError> {
        debug_assert_eq!(packed.limit_flags & FORBIDDEN_LIMIT_FLAGS, 0);
        let info = extended_limit_from_packed(packed);
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| BackstopError(format!("SetInformationJobObject: {e}")))
        }
    }

    fn close_job(&self, job: JobHandle) {
        drop(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockJob {
        memory_limit: Option<u64>,
        assigned: HashSet<u32>,
    }

    struct MockHooks {
        jobs: Arc<Mutex<HashMap<usize, MockJob>>>,
        next_id: Mutex<usize>,
        fail_assign_pids: HashSet<u32>,
    }

    impl MockHooks {
        fn new() -> Self {
            Self {
                jobs: Arc::new(Mutex::new(HashMap::new())),
                next_id: Mutex::new(1),
                fail_assign_pids: HashSet::new(),
            }
        }

        fn with_fail_assign(pid: u32) -> Self {
            let mut hooks = Self::new();
            hooks.fail_assign_pids.insert(pid);
            hooks
        }

        fn job_id(handle: &JobHandle) -> usize {
            handle.0 .0 as usize
        }
    }

    impl BackstopHooks for MockHooks {
        fn create_job(&self) -> Result<JobHandle, BackstopError> {
            let mut next = self.next_id.lock().unwrap();
            let id = *next;
            *next += 1;
            self.jobs.lock().unwrap().insert(
                id,
                MockJob {
                    memory_limit: None,
                    assigned: HashSet::new(),
                },
            );
            Ok(JobHandle(HANDLE(id as *mut core::ffi::c_void)))
        }

        fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError> {
            if self.fail_assign_pids.contains(&pid) {
                return Err(BackstopError(format!("nested job pid={pid}")));
            }
            let id = Self::job_id(job);
            let mut jobs = self.jobs.lock().unwrap();
            let entry = jobs
                .get_mut(&id)
                .ok_or_else(|| BackstopError("unknown job".into()))?;
            entry.assigned.insert(pid);
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
            entry.memory_limit = if packed.limit_flags & JOB_OBJECT_LIMIT_JOB_MEMORY.0 != 0 {
                Some(packed.job_memory_limit)
            } else {
                None
            };
            Ok(())
        }

        fn close_job(&self, job: JobHandle) {
            let id = Self::job_id(&job);
            self.jobs.lock().unwrap().remove(&id);
            std::mem::forget(job);
        }
    }

    #[test]
    fn pack_never_sets_forbidden_flags() {
        for limit in [None, Some(1), Some(4 * 1024 * 1024 * 1024)] {
            let packed = pack_job_memory_limit(limit);
            assert_eq!(packed.limit_flags & FORBIDDEN_LIMIT_FLAGS, 0);
        }
    }

    #[test]
    fn pack_sets_job_memory_flag_when_limited() {
        let packed = pack_job_memory_limit(Some(1_000_000));
        assert_ne!(packed.limit_flags & JOB_OBJECT_LIMIT_JOB_MEMORY.0, 0);
        assert_eq!(packed.job_memory_limit, 1_000_000);
    }

    #[test]
    fn pack_clears_job_memory_flag_when_unlimited() {
        let packed = pack_job_memory_limit(None);
        assert_eq!(packed.limit_flags & JOB_OBJECT_LIMIT_JOB_MEMORY.0, 0);
        assert_eq!(packed.job_memory_limit, 0);
    }

    #[test]
    fn store_one_job_per_group() {
        let mut store = JobBackstopStore::with_hooks(Box::new(MockHooks::new()));
        store.set_memory_limit("chrome", 100).unwrap();
        store.set_memory_limit("edge", 200).unwrap();
        assert!(store.has_group("chrome"));
        assert!(store.has_group("edge"));
        assert_eq!(store.memory_limit("chrome"), Some(Some(100)));
        assert_eq!(store.memory_limit("edge"), Some(Some(200)));
    }

    #[test]
    fn assign_pid_degrades_on_nested_failure() {
        let mut store = JobBackstopStore::with_hooks(Box::new(MockHooks::with_fail_assign(42)));
        let err = store.assign_pid("app", 42).unwrap_err();
        assert!(err.0.contains("nested job"));
        assert!(!store.assigned_pids("app").unwrap().contains(&42));
    }

    #[test]
    fn assign_pid_skips_duplicate() {
        let hooks = MockHooks::new();
        let mut store = JobBackstopStore::with_hooks(Box::new(hooks));
        store.assign_pid("app", 7).unwrap();
        store.assign_pid("app", 7).unwrap();
        assert_eq!(store.assigned_pids("app").unwrap().len(), 1);
    }

    #[test]
    fn clear_limit_returns_unlimited() {
        let mut store = JobBackstopStore::with_hooks(Box::new(MockHooks::new()));
        store.set_memory_limit("app", 500).unwrap();
        store.clear_limit("app").unwrap();
        assert_eq!(store.memory_limit("app"), Some(None));
        assert!(store.has_group("app"));
    }

    #[test]
    fn clear_limit_noop_without_job() {
        let mut store = JobBackstopStore::with_hooks(Box::new(MockHooks::new()));
        store.clear_limit("missing").unwrap();
        assert!(!store.has_group("missing"));
    }

    #[test]
    fn drop_closes_jobs_without_terminate() {
        let hooks = MockHooks::new();
        let jobs = Arc::clone(&hooks.jobs);
        {
            let mut store = JobBackstopStore::with_hooks(Box::new(hooks));
            store.set_memory_limit("app", 1).unwrap();
            assert_eq!(jobs.lock().unwrap().len(), 1);
        }
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[test]
    #[ignore = "requires Windows job APIs; run locally"]
    fn win32_create_set_clear_assign_current_process() {
        let mut store = JobBackstopStore::new();
        let pid = std::process::id();
        store.set_memory_limit("local", 8 * 1024 * 1024 * 1024).unwrap();
        // Assign may fail if already in a non-nestable job (e.g. CI agent).
        if store.assign_pid("local", pid).is_ok() {
            assert!(store.assigned_pids("local").unwrap().contains(&pid));
        }
        store.clear_limit("local").unwrap();
        assert_eq!(store.memory_limit("local"), Some(None));
    }
}
