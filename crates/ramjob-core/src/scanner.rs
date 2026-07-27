//! Process enumeration via NtQuerySystemInformation(SystemProcessInformation).

use std::collections::HashMap;
use std::path::PathBuf;

use windows::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessInformation,
};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, NTSTATUS, STATUS_INFO_LENGTH_MISMATCH, STATUS_INVALID_PARAMETER,
    STATUS_SUCCESS, UNICODE_STRING,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// One process from a SystemProcessInformation sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRecord {
    pub pid: u32,
    pub ppid: u32,
    pub session_id: u32,
    pub image_name: String,
    pub private_working_set_bytes: u64,
    /// Total working set (SPI `WorkingSetSize`), used for CompressStore.
    pub working_set_bytes: u64,
    /// FILETIME ticks (100ns since 1601-01-01 UTC).
    pub create_time: i64,
    pub image_path: Option<PathBuf>,
}

/// `(pid, create_time)` → resolved image path (`None` means resolve failed once).
pub type PathCacheKey = (u32, i64);
pub type PathCache = HashMap<PathCacheKey, Option<PathBuf>>;

/// Enumerate with a caller-owned path cache. Resolve runs once per new cache key.
pub fn enumerate_processes_with_cache(
    cache: &mut PathCache,
) -> Result<Vec<ProcessRecord>, NTSTATUS> {
    let buffer = query_system_process_information()?;
    let mut out = Vec::new();
    let entry_size = std::mem::size_of::<SystemProcessInformation>();

    unsafe {
        let mut offset = 0usize;
        loop {
            if offset.checked_add(entry_size).map_or(true, |end| end > buffer.len()) {
                return Err(STATUS_INVALID_PARAMETER);
            }
            let entry = &*buffer.as_ptr().add(offset).cast::<SystemProcessInformation>();
            let pid = handle_to_pid(entry.unique_process_id);
            let ppid = handle_to_pid(entry.inherited_from_unique_process_id);
            let create_time = entry.create_time;
            let image_name = unicode_to_string(&entry.image_name);
            let key = (pid, create_time);
            let image_path = if pid == 0 {
                None
            } else {
                cache
                    .entry(key)
                    .or_insert_with(|| resolve_image_path(pid))
                    .clone()
            };

            out.push(ProcessRecord {
                pid,
                ppid,
                session_id: entry.session_id,
                image_name,
                private_working_set_bytes: entry.working_set_private_size.max(0) as u64,
                working_set_bytes: entry.working_set_size as u64,
                create_time,
                image_path,
            });

            if entry.next_entry_offset == 0 {
                break;
            }
            let Some(next) = offset.checked_add(entry.next_entry_offset as usize) else {
                return Err(STATUS_INVALID_PARAMETER);
            };
            if next >= buffer.len() {
                return Err(STATUS_INVALID_PARAMETER);
            }
            offset = next;
        }
    }

    Ok(out)
}

fn query_system_process_information() -> Result<Vec<u8>, NTSTATUS> {
    let mut length = 0u32;
    unsafe {
        let status = NtQuerySystemInformation(SystemProcessInformation, std::ptr::null_mut(), 0, &mut length);
        if status != STATUS_INFO_LENGTH_MISMATCH && status != STATUS_SUCCESS {
            return Err(status);
        }
    }

    // Grow until the buffer fits; process list can change between calls.
    for _ in 0..8 {
        let mut buffer = vec![0u8; length as usize];
        let mut return_length = 0u32;
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                length,
                &mut return_length,
            )
        };
        if status == STATUS_SUCCESS {
            buffer.truncate(return_length as usize);
            return Ok(buffer);
        }
        if status != STATUS_INFO_LENGTH_MISMATCH {
            return Err(status);
        }
        length = return_length.max(length.saturating_mul(2)).max(length + 64 * 1024);
    }
    Err(STATUS_INFO_LENGTH_MISMATCH)
}

fn resolve_image_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let path = query_full_image_name(handle);
        let _ = CloseHandle(handle);
        path
    }
}

unsafe fn query_full_image_name(handle: HANDLE) -> Option<PathBuf> {
    let mut size = 260u32;
    for _ in 0..4 {
        let mut buf = vec![0u16; size as usize];
        let mut needed = size;
        match QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut needed)
        {
            Ok(()) => {
                let end = buf.iter().position(|&c| c == 0).unwrap_or(needed as usize);
                let s = String::from_utf16_lossy(&buf[..end]);
                if s.is_empty() {
                    return None;
                }
                return Some(PathBuf::from(s));
            }
            Err(_) => {
                if needed > size {
                    size = needed;
                } else {
                    size = size.saturating_mul(2);
                }
            }
        }
    }
    None
}

fn handle_to_pid(handle: HANDLE) -> u32 {
    handle.0 as usize as u32
}

fn unicode_to_string(us: &UNICODE_STRING) -> String {
    if us.Buffer.0.is_null() || us.Length == 0 {
        return String::new();
    }
    let len = (us.Length / 2) as usize;
    let slice = unsafe { std::slice::from_raw_parts(us.Buffer.0, len) };
    String::from_utf16_lossy(slice)
}

/// PHNT-accurate SystemProcessInformation. windows-rs 0.58 collapses the Vista+
/// prefix fields into Reserved blobs and omits WorkingSetPrivateSize / CreateTime.
#[repr(C)]
struct SystemProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UNICODE_STRING,
    base_priority: i32,
    unique_process_id: HANDLE,
    inherited_from_unique_process_id: HANDLE,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Threading::GetCurrentProcessId;

    #[test]
    fn current_process_appears_with_pid_and_private_ws() {
        let self_pid = unsafe { GetCurrentProcessId() };
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        let me = procs
            .iter()
            .find(|p| p.pid == self_pid)
            .expect("current process must appear in NtQSI sweep");
        assert!(me.pid > 0);
        assert!(
            me.private_working_set_bytes > 0,
            "current process private WS must be non-zero (SPI WorkingSetPrivateSize layout)"
        );
        assert!(
            me.create_time != 0,
            "current process create_time must be non-zero (SPI CreateTime layout)"
        );
        assert!(!me.image_name.is_empty());
    }

    #[test]
    fn session_zero_processes_are_retained() {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        assert!(
            procs.iter().any(|p| p.session_id == 0),
            "session 0 processes must not be filtered at enumeration"
        );
    }

    #[test]
    fn image_path_cache_resolves_once_per_pid_create_time() {
        use std::collections::HashSet;

        let self_pid = unsafe { GetCurrentProcessId() };
        let mut cache = PathCache::new();

        let first = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        let me_first = first.iter().find(|p| p.pid == self_pid).unwrap();
        assert!(
            me_first.image_path.is_some(),
            "current process image path should resolve"
        );
        let keys_first: HashSet<_> = first
            .iter()
            .filter(|p| p.pid != 0)
            .map(|p| (p.pid, p.create_time))
            .collect();
        let cache_len_after_first = cache.len();
        assert_eq!(
            cache_len_after_first,
            keys_first.len(),
            "one cache entry per non-zero PID+create-time key"
        );

        let second = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        let me_second = second.iter().find(|p| p.pid == self_pid).unwrap();
        assert_eq!(me_first.image_path, me_second.image_path);
        assert_eq!(me_first.create_time, me_second.create_time);

        let new_keys = second
            .iter()
            .filter(|p| p.pid != 0 && !keys_first.contains(&(p.pid, p.create_time)))
            .count();
        assert_eq!(
            cache.len() - cache_len_after_first,
            new_keys,
            "OpenProcess/path resolve must run only for newly seen PID+create-time keys"
        );
    }

    #[test]
    fn missing_path_processes_still_returned() {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        assert!(
            procs.iter().any(|p| p.image_path.is_none()),
            "processes that fail OpenProcess/path resolve must still be returned"
        );
    }
}
