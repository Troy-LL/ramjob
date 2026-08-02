//! ETW backend: `Microsoft-Windows-Kernel-Process` start/stop → spawn/exit.
//!
//! Opens a real-time trace session and enables the kernel process provider. Live
//! events are delivered via an ETW callback into an internal queue; [`DiscoverySource::poll_events`]
//! drains that queue. If session open or consumer attach fails, [`EtwProcessSource::try_new`] returns
//! [`EtwOpenError`] so the caller can fall back to WMI / sweep.

use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::core::GUID;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS};
use windows::Win32::System::Diagnostics::Etw::{
    CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW,
    CONTROLTRACE_HANDLE, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_TRACE_CONTROL_STOP,
    EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, PROCESS_TRACE_MODE_EVENT_RECORD,
    PROCESS_TRACE_MODE_REAL_TIME, PROCESSTRACE_HANDLE, TRACE_LEVEL_INFORMATION,
    WNODE_FLAG_TRACED_GUID,
};
use windows::Win32::System::Diagnostics::Etw::{EVENT_RECORD, EVENT_TRACE_LOGFILEW};
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::{DiscoveryEvent, DiscoverySource};

/// Microsoft-Windows-Kernel-Process provider.
const KERNEL_PROCESS_PROVIDER: GUID =
    GUID::from_u128(0x22fb2cd6_0e7b_422b_a0c7_2fad1fd0e716);

/// `WINEVENT_KEYWORD_PROCESS` — process start/stop events only.
const KEYWORD_PROCESS: u64 = 0x10;

const EVENT_PROCESS_START: u16 = 1;
const EVENT_PROCESS_STOP: u16 = 2;

/// Failure to open or enable the ETW kernel-process session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtwOpenError {
    pub stage: &'static str,
    pub code: u32,
}

impl std::fmt::Display for EtwOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ETW open failed at {} (win32={})", self.stage, self.code)
    }
}

impl std::error::Error for EtwOpenError {}

/// One-shot diagnostic for degrade path (ETW → WMI → sweep).
pub fn etw_degrade_diagnostic(err: &EtwOpenError) -> String {
    format!(
        "discovery ETW unavailable at {} (win32={}): falling back",
        err.stage, err.code
    )
}

type EventQueue = Arc<Mutex<VecDeque<DiscoveryEvent>>>;
type QueuePtr = Mutex<VecDeque<DiscoveryEvent>>;

/// ETW-backed process discovery via `Microsoft-Windows-Kernel-Process`.
pub struct EtwProcessSource {
    queue: EventQueue,
    session: Option<EtwSession>,
    consumer: Option<JoinHandle<()>>,
}

struct EtwSession {
    trace_handle: CONTROLTRACE_HANDLE,
    properties_buf: Vec<u8>,
    logger_name: Vec<u16>,
}

impl Drop for EtwSession {
    fn drop(&mut self) {
        unsafe {
            let props = self.properties_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            let _ = ControlTraceW(
                self.trace_handle,
                windows::core::PCWSTR(self.logger_name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
    }
}

/// Shared queue pointer for the ETW callback (one active source per process).
static CALLBACK_QUEUE: AtomicPtr<QueuePtr> = AtomicPtr::new(std::ptr::null_mut());

impl EtwProcessSource {
    /// Open a real-time kernel-process ETW session. Returns `Err` when ETW is
    /// unavailable (access denied, session conflict, consumer attach failure, etc.)
    /// so callers degrade.
    pub fn try_new() -> Result<Self, EtwOpenError> {
        let queue: EventQueue = Arc::new(Mutex::new(VecDeque::new()));
        let session = open_kernel_process_session()?;
        match start_consumer_thread(&session.logger_name, &queue) {
            Ok(consumer) => Ok(Self {
                queue,
                session: Some(session),
                consumer: Some(consumer),
            }),
            Err(e) => {
                // Drop session to stop the trace before returning degrade Err.
                drop(session);
                Err(e)
            }
        }
    }

    /// Push events into the poll queue (unit tests / harness).
    pub fn inject_events(&mut self, events: impl IntoIterator<Item = DiscoveryEvent>) {
        if let Ok(mut q) = self.queue.lock() {
            q.extend(events);
        }
    }

    #[cfg(test)]
    pub(crate) fn new_inject_only() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            session: None,
            consumer: None,
        }
    }
}

impl DiscoverySource for EtwProcessSource {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.queue.lock() {
            while let Some(e) = q.pop_front() {
                out.push(e);
            }
        }
        out
    }
}

impl Drop for EtwProcessSource {
    fn drop(&mut self) {
        CALLBACK_QUEUE.store(std::ptr::null_mut(), Ordering::Release);
        // Stop trace before joining the consumer (ProcessTrace blocks until stop).
        self.session = None;
        if let Some(handle) = self.consumer.take() {
            let _ = handle.join();
        }
    }
}

fn open_kernel_process_session() -> Result<EtwSession, EtwOpenError> {
    let pid = unsafe { GetCurrentProcessId() };
    let name = format!("RamJobDiscovery-{pid}");
    let logger_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

    let props_size = size_of::<EVENT_TRACE_PROPERTIES>();
    let name_bytes = logger_name.len() * 2;
    let mut properties_buf = vec![0u8; props_size + name_bytes + 64];
    let properties = properties_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;

    unsafe {
        (*properties).Wnode.BufferSize = properties_buf.len() as u32;
        (*properties).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*properties).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*properties).LoggerNameOffset = props_size as u32;

        let name_ptr = properties_buf.as_mut_ptr().add(props_size) as *mut u16;
        std::ptr::copy_nonoverlapping(logger_name.as_ptr(), name_ptr, logger_name.len());

        let mut trace_handle = CONTROLTRACE_HANDLE::default();
        let status = StartTraceW(
            &mut trace_handle,
            windows::core::PCWSTR(name_ptr),
            properties,
        );

        if status == ERROR_ALREADY_EXISTS {
            let _ = ControlTraceW(
                CONTROLTRACE_HANDLE::default(),
                windows::core::PCWSTR(name_ptr),
                properties,
                EVENT_TRACE_CONTROL_STOP,
            );
            let retry = StartTraceW(&mut trace_handle, windows::core::PCWSTR(name_ptr), properties);
            if retry != ERROR_SUCCESS {
                return Err(EtwOpenError {
                    stage: "StartTraceW",
                    code: retry.0,
                });
            }
        } else if status != ERROR_SUCCESS {
            return Err(EtwOpenError {
                stage: "StartTraceW",
                code: status.0,
            });
        }

        let enable = EnableTraceEx2(
            trace_handle,
            &KERNEL_PROCESS_PROVIDER,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            TRACE_LEVEL_INFORMATION as u8,
            KEYWORD_PROCESS,
            0,
            0,
            None,
        );
        if enable != ERROR_SUCCESS {
            let _ = ControlTraceW(
                trace_handle,
                windows::core::PCWSTR(name_ptr),
                properties,
                EVENT_TRACE_CONTROL_STOP,
            );
            return Err(EtwOpenError {
                stage: "EnableTraceEx2",
                code: enable.0,
            });
        }

        Ok(EtwSession {
            trace_handle,
            properties_buf,
            logger_name,
        })
    }
}

fn start_consumer_thread(
    logger_name: &[u16],
    queue: &EventQueue,
) -> Result<JoinHandle<()>, EtwOpenError> {
    let name_copy: Vec<u16> = logger_name.to_vec();
    CALLBACK_QUEUE.store(Arc::as_ptr(queue) as *mut QueuePtr, Ordering::Release);

    let (ready_tx, ready_rx) = mpsc::channel();
    let handle = thread::Builder::new()
        .name("ramjob-etw-consumer".into())
        .spawn(move || run_consumer(&name_copy, ready_tx))
        .map_err(|e| {
            CALLBACK_QUEUE.store(std::ptr::null_mut(), Ordering::Release);
            EtwOpenError {
                stage: "spawn_consumer",
                code: e.raw_os_error().unwrap_or(1) as u32,
            }
        })?;

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(e)) => {
            CALLBACK_QUEUE.store(std::ptr::null_mut(), Ordering::Release);
            let _ = handle.join();
            Err(e)
        }
        Err(_) => {
            CALLBACK_QUEUE.store(std::ptr::null_mut(), Ordering::Release);
            let _ = handle.join();
            Err(EtwOpenError {
                stage: "OpenTraceW",
                code: unsafe { GetLastError().0 },
            })
        }
    }
}

fn open_consumer_trace(logger_name: &[u16]) -> Result<PROCESSTRACE_HANDLE, EtwOpenError> {
    unsafe {
        let mut logfile = EVENT_TRACE_LOGFILEW::default();
        logfile.LoggerName = windows::core::PWSTR(logger_name.as_ptr() as *mut u16);
        logfile.Anonymous1.ProcessTraceMode =
            PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
        logfile.Anonymous2.EventRecordCallback = Some(etw_event_callback);

        let trace_handle = OpenTraceW(&mut logfile);
        if trace_handle == PROCESSTRACE_HANDLE::default() || trace_handle.Value == u64::MAX {
            return Err(EtwOpenError {
                stage: "OpenTraceW",
                code: GetLastError().0,
            });
        }
        Ok(trace_handle)
    }
}

fn run_consumer(logger_name: &[u16], ready: mpsc::Sender<Result<(), EtwOpenError>>) {
    match open_consumer_trace(logger_name) {
        Ok(trace_handle) => {
            let _ = ready.send(Ok(()));
            unsafe {
                let _ = ProcessTrace(&[trace_handle], None, None);
                let _ = CloseTrace(trace_handle);
            }
        }
        Err(e) => {
            let _ = ready.send(Err(e));
        }
    }
}

unsafe extern "system" fn etw_event_callback(event: *mut EVENT_RECORD) {
    if event.is_null() {
        return;
    }
    let Some(mapped) = map_kernel_process_event(&*event) else {
        return;
    };
    let ptr = CALLBACK_QUEUE.load(Ordering::Acquire);
    if ptr.is_null() {
        return;
    }
    if let Ok(mut q) = unsafe { (*ptr).lock() } {
        q.push_back(mapped);
    }
}

/// Map a kernel-process ETW record to a discovery event (testable without live ETW).
pub(crate) fn map_kernel_process_event(record: &EVENT_RECORD) -> Option<DiscoveryEvent> {
    if record.EventHeader.ProviderId != KERNEL_PROCESS_PROVIDER {
        return None;
    }
    let id = record.EventHeader.EventDescriptor.Id;
    let kind = match id {
        EVENT_PROCESS_START => DiscoveryEventKind::Spawn,
        EVENT_PROCESS_STOP => DiscoveryEventKind::Exit,
        _ => return None,
    };
    parse_process_payload(record, kind)
}

#[derive(Clone, Copy)]
enum DiscoveryEventKind {
    Spawn,
    Exit,
}

fn parse_process_payload(record: &EVENT_RECORD, kind: DiscoveryEventKind) -> Option<DiscoveryEvent> {
    if record.UserData.is_null() || record.UserDataLength < 16 {
        return None;
    }
    let data = unsafe {
        std::slice::from_raw_parts(record.UserData as *const u8, record.UserDataLength as usize)
    };
    let pid = u32::from_ne_bytes(data[0..4].try_into().ok()?);
    let create_time = i64::from_ne_bytes(data[8..16].try_into().ok()?);
    if pid == 0 {
        return None;
    }
    Some(match kind {
        DiscoveryEventKind::Spawn => DiscoveryEvent::Spawn { pid, create_time },
        DiscoveryEventKind::Exit => DiscoveryEvent::Exit { pid, create_time },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::zeroed;
    use windows::Win32::System::Diagnostics::Etw::EVENT_HEADER;

    fn fake_record(event_id: u16, pid: u32, create_time: i64) -> EVENT_RECORD {
        let mut payload = [0u8; 16];
        payload[0..4].copy_from_slice(&pid.to_ne_bytes());
        payload[8..16].copy_from_slice(&(create_time as u64).to_ne_bytes());

        let mut record: EVENT_RECORD = unsafe { zeroed() };
        record.EventHeader = EVENT_HEADER {
            ProviderId: KERNEL_PROCESS_PROVIDER,
            EventDescriptor: windows::Win32::System::Diagnostics::Etw::EVENT_DESCRIPTOR {
                Id: event_id,
                ..unsafe { zeroed() }
            },
            ..unsafe { zeroed() }
        };
        record.UserDataLength = payload.len() as u16;
        record.UserData = payload.as_ptr() as *mut _;
        record
    }

    #[test]
    fn map_process_start_to_spawn() {
        let record = fake_record(EVENT_PROCESS_START, 42, 9_999);
        assert_eq!(
            map_kernel_process_event(&record),
            Some(DiscoveryEvent::Spawn {
                pid: 42,
                create_time: 9_999
            })
        );
    }

    #[test]
    fn map_process_stop_to_exit() {
        let record = fake_record(EVENT_PROCESS_STOP, 7, 1_234);
        assert_eq!(
            map_kernel_process_event(&record),
            Some(DiscoveryEvent::Exit {
                pid: 7,
                create_time: 1_234
            })
        );
    }

    #[test]
    fn map_ignores_other_event_ids() {
        let record = fake_record(99, 1, 1);
        assert!(map_kernel_process_event(&record).is_none());
    }

    #[test]
    fn inject_and_poll_spawn_exit() {
        let mut src = EtwProcessSource::new_inject_only();
        src.inject_events([
            DiscoveryEvent::Spawn {
                pid: 1,
                create_time: 10,
            },
            DiscoveryEvent::Exit {
                pid: 2,
                create_time: 20,
            },
        ]);
        assert_eq!(
            src.poll_events(),
            vec![
                DiscoveryEvent::Spawn {
                    pid: 1,
                    create_time: 10
                },
                DiscoveryEvent::Exit {
                    pid: 2,
                    create_time: 20
                },
            ]
        );
        assert!(src.poll_events().is_empty());
    }

    #[test]
    fn degrade_diagnostic_includes_stage_and_code() {
        let err = EtwOpenError {
            stage: "EnableTraceEx2",
            code: 5,
        };
        let msg = etw_degrade_diagnostic(&err);
        assert!(msg.contains("EnableTraceEx2"));
        assert!(msg.contains("5"));
        assert!(msg.contains("falling back"));
    }

    #[test]
    fn degrade_diagnostic_includes_open_trace_stage() {
        let err = EtwOpenError {
            stage: "OpenTraceW",
            code: 87,
        };
        let msg = etw_degrade_diagnostic(&err);
        assert!(msg.contains("OpenTraceW"));
        assert!(msg.contains("87"));
    }

    #[test]
    fn try_new_reports_open_result() {
        match EtwProcessSource::try_new() {
            Ok(_) => eprintln!("ETW session opened on this machine (live path)"),
            Err(e) => {
                eprintln!("ETW unavailable as expected: {e}");
                assert!(
                    matches!(
                        e.stage,
                        "StartTraceW" | "EnableTraceEx2" | "OpenTraceW" | "spawn_consumer"
                    ),
                    "unexpected degrade stage: {}",
                    e.stage
                );
            }
        }
    }
}
