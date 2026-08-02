//! Shared mock [`BackstopHooks`] for unit and integration tests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_JOB_MEMORY;

use super::{BackstopError, BackstopHooks, JobHandle, PackedJobLimit};

#[derive(Debug, Default, Clone)]
struct MockJob {
    memory_limit: Option<u64>,
    assigned: HashSet<u32>,
    closed: bool,
}

#[derive(Debug, Default)]
struct MockState {
    jobs: HashMap<usize, MockJob>,
    next_id: usize,
}

/// Injectable mock hooks; unlimited vs limited uses `JOB_OBJECT_LIMIT_JOB_MEMORY` flags.
#[derive(Clone)]
pub struct MockBackstopHooks {
    state: Arc<Mutex<MockState>>,
    fail_assign_pids: HashSet<u32>,
    fail_apply: bool,
}

impl MockBackstopHooks {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState::default())),
            fail_assign_pids: HashSet::new(),
            fail_apply: false,
        }
    }

    pub fn with_fail_assign(pid: u32) -> Self {
        let mut hooks = Self::new();
        hooks.fail_assign_pids.insert(pid);
        hooks
    }

    pub fn with_fail_apply() -> Self {
        let mut hooks = Self::new();
        hooks.fail_apply = true;
        hooks
    }

    pub fn job_closed(&self, handle: &JobHandle) -> bool {
        let id = Self::job_id(handle);
        self.state
            .lock()
            .unwrap()
            .jobs
            .get(&id)
            .is_some_and(|j| j.closed)
    }

    pub fn job_count(&self) -> usize {
        self.state.lock().unwrap().jobs.len()
    }

    /// Snapshot job state after store drop (closed flag, limit, assigned PIDs).
    pub fn job_snapshots(&self) -> Vec<(bool, Option<u64>, HashSet<u32>)> {
        self.state
            .lock()
            .unwrap()
            .jobs
            .values()
            .map(|j| (j.closed, j.memory_limit, j.assigned.clone()))
            .collect()
    }

    fn job_id(handle: &JobHandle) -> usize {
        handle.0 .0 as usize
    }
}

impl BackstopHooks for MockBackstopHooks {
    fn create_job(&self) -> Result<JobHandle, BackstopError> {
        let mut state = self.state.lock().unwrap();
        let id = state.next_id;
        state.next_id += 1;
        state.jobs.insert(id, MockJob::default());
        Ok(JobHandle(HANDLE(id as *mut core::ffi::c_void)))
    }

    fn assign_process(&self, job: &JobHandle, pid: u32) -> Result<(), BackstopError> {
        if self.fail_assign_pids.contains(&pid) {
            return Err(BackstopError(format!("nested job pid={pid}")));
        }
        let id = Self::job_id(job);
        let mut state = self.state.lock().unwrap();
        let entry = state
            .jobs
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
        if self.fail_apply {
            return Err(BackstopError("apply_packed_limit failed".into()));
        }
        let id = Self::job_id(job);
        let mut state = self.state.lock().unwrap();
        let entry = state
            .jobs
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
        if let Some(entry) = self.state.lock().unwrap().jobs.get_mut(&id) {
            entry.closed = true;
        }
        std::mem::forget(job);
    }
}
