//! WMI backend: `Win32_Process` creation/deletion events → spawn/exit.
//!
//! Subscribes via SWbem `__InstanceCreationEvent` / `__InstanceDeletionEvent` on
//! `Win32_Process`. Live events are polled in a background thread into an internal
//! queue; [`DiscoverySource::poll_events`] drains that queue. If COM/WMI setup
//! fails, [`WmiProcessSource::try_new`] returns [`WmiOpenError`] for sweep fallback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::core::{BSTR, HRESULT, IUnknown, Interface, VARIANT};
use windows::Win32::Foundation::{S_FALSE, S_OK};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Wmi::{
    ISWbemEventSource, ISWbemLocator, ISWbemObject, ISWbemServices, SWbemLocator,
};

use super::queued::QueuedDiscovery;
use super::{DiscoveryEvent, DiscoverySource};

const WMI_CREATE_QUERY: &str =
    "SELECT * FROM __InstanceCreationEvent WITHIN 1 WHERE TargetInstance ISA 'Win32_Process'";
const WMI_DELETE_QUERY: &str =
    "SELECT * FROM __InstanceDeletionEvent WITHIN 1 WHERE TargetInstance ISA 'Win32_Process'";
const WMI_QUERY_LANG: &str = "WQL";

/// Failure to initialize COM or subscribe to WMI process events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WmiOpenError {
    pub stage: &'static str,
    pub code: u32,
}

impl std::fmt::Display for WmiOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WMI open failed at {} (win32={})", self.stage, self.code)
    }
}

impl std::error::Error for WmiOpenError {}

/// One-shot diagnostic when WMI is unavailable and discovery falls back to sweep.
pub fn wmi_degrade_diagnostic(err: &WmiOpenError) -> String {
    format!(
        "discovery WMI unavailable at {} (win32={}): falling back",
        err.stage, err.code
    )
}

struct WmiSubscriptions {
    create_source: ISWbemEventSource,
    delete_source: ISWbemEventSource,
}

/// WMI-backed process discovery via `Win32_Process` instance events.
pub struct WmiProcessSource {
    queued: QueuedDiscovery,
    shutdown: Arc<AtomicBool>,
    consumer: Option<JoinHandle<()>>,
}

impl WmiProcessSource {
    /// Open WMI process event subscriptions. Returns `Err` when COM/WMI is unavailable
    /// so callers degrade to sweep.
    pub fn try_new() -> Result<Self, WmiOpenError> {
        let queued = QueuedDiscovery::new();
        let shutdown = Arc::new(AtomicBool::new(false));
        let consumer = start_consumer_thread(queued.inner(), &shutdown)?;
        Ok(Self {
            queued,
            shutdown,
            consumer: Some(consumer),
        })
    }

    /// Push events into the poll queue (unit tests / harness).
    pub fn inject_events(&mut self, events: impl IntoIterator<Item = DiscoveryEvent>) {
        self.queued.inject_events(events);
    }

    #[cfg(test)]
    pub(crate) fn new_inject_only() -> Self {
        Self {
            queued: QueuedDiscovery::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
            consumer: None,
        }
    }
}

impl DiscoverySource for WmiProcessSource {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent> {
        self.queued.drain()
    }
}

impl Drop for WmiProcessSource {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.consumer.take() {
            let _ = handle.join();
        }
    }
}

fn init_com() -> Result<bool, WmiOpenError> {
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr == S_OK {
        Ok(true)
    } else if hr == S_FALSE {
        Ok(false)
    } else {
        Err(WmiOpenError {
            stage: "CoInitializeEx",
            code: hr.0 as u32,
        })
    }
}

fn open_wmi_subscriptions() -> Result<WmiSubscriptions, WmiOpenError> {
    unsafe {
        let locator: ISWbemLocator =
            CoCreateInstance(&SWbemLocator, None, CLSCTX_INPROC_SERVER).map_err(|e| WmiOpenError {
                stage: "CoCreateInstance",
                code: e.code().0 as u32,
            })?;

        let services: ISWbemServices = locator
            .ConnectServer(
                &BSTR::from("."),
                &BSTR::from("root\\cimv2"),
                &BSTR::from(""),
                &BSTR::from(""),
                &BSTR::from(""),
                &BSTR::from(""),
                0,
                None,
            )
            .map_err(|e| WmiOpenError {
                stage: "ConnectServer",
                code: e.code().0 as u32,
            })?;

        let create_source = services
            .ExecNotificationQuery(
                &BSTR::from(WMI_QUERY_LANG),
                &BSTR::from(WMI_CREATE_QUERY),
                0,
                None,
            )
            .map_err(|e| WmiOpenError {
                stage: "ExecNotificationQuery(create)",
                code: e.code().0 as u32,
            })?;

        let delete_source = services
            .ExecNotificationQuery(
                &BSTR::from(WMI_QUERY_LANG),
                &BSTR::from(WMI_DELETE_QUERY),
                0,
                None,
            )
            .map_err(|e| WmiOpenError {
                stage: "ExecNotificationQuery(delete)",
                code: e.code().0 as u32,
            })?;

        Ok(WmiSubscriptions {
            create_source,
            delete_source,
        })
    }
}

fn start_consumer_thread(
    queue: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<DiscoveryEvent>>>,
    shutdown: &Arc<AtomicBool>,
) -> Result<JoinHandle<()>, WmiOpenError> {
    let queue = std::sync::Arc::clone(queue);
    let shutdown_for_worker = Arc::clone(&shutdown);
    let (ready_tx, ready_rx) = mpsc::channel();

    let handle = thread::Builder::new()
        .name("ramjob-wmi-consumer".into())
        .spawn(move || run_consumer_thread(queue, shutdown_for_worker, ready_tx))
        .map_err(|e| WmiOpenError {
            stage: "spawn_consumer",
            code: e.raw_os_error().unwrap_or(1) as u32,
        })?;

    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(e)) => {
            shutdown.store(true, Ordering::Release);
            let _ = handle.join();
            Err(e)
        }
        Err(_) => {
            shutdown.store(true, Ordering::Release);
            let _ = handle.join();
            Err(WmiOpenError {
                stage: "open_subscriptions",
                code: HRESULT::from_win32(1460).0 as u32, // WAIT_TIMEOUT
            })
        }
    }
}

fn run_consumer_thread(
    queue: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<DiscoveryEvent>>>,
    shutdown: Arc<AtomicBool>,
    ready: mpsc::Sender<Result<(), WmiOpenError>>,
) {
    let com_initialized = match init_com() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };

    let subscriptions = match open_wmi_subscriptions() {
        Ok(subs) => subs,
        Err(e) => {
            if com_initialized {
                unsafe { CoUninitialize() };
            }
            let _ = ready.send(Err(e));
            return;
        }
    };

    if ready.send(Ok(())).is_err() {
        if com_initialized {
            unsafe { CoUninitialize() };
        }
        return;
    }

    run_consumer(subscriptions, queue, shutdown);

    if com_initialized {
        unsafe { CoUninitialize() };
    }
}

fn run_consumer(
    subscriptions: WmiSubscriptions,
    queue: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<DiscoveryEvent>>>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Acquire) {
        unsafe {
            if let Ok(event) = subscriptions.create_source.NextEvent(100) {
                if let Some(mapped) = map_wmi_process_event(&event, DiscoveryEventKind::Spawn) {
                    push_event(&queue, mapped);
                }
            }
            if shutdown.load(Ordering::Acquire) {
                break;
            }
            if let Ok(event) = subscriptions.delete_source.NextEvent(100) {
                if let Some(mapped) = map_wmi_process_event(&event, DiscoveryEventKind::Exit) {
                    push_event(&queue, mapped);
                }
            }
        }
    }
}

fn push_event(
    queue: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<DiscoveryEvent>>>,
    event: DiscoveryEvent,
) {
    if let Ok(mut q) = queue.lock() {
        q.push_back(event);
    }
}

#[derive(Clone, Copy)]
pub(crate) enum DiscoveryEventKind {
    Spawn,
    Exit,
}

/// Map a WMI process instance event to a discovery event (testable without live WMI).
pub(crate) fn map_wmi_process_event(
    event: &ISWbemObject,
    kind: DiscoveryEventKind,
) -> Option<DiscoveryEvent> {
    unsafe {
        let props = event.Properties_().ok()?;
        let target_prop = props.Item(&BSTR::from("TargetInstance"), 0).ok()?;
        let target_var = target_prop.Value().ok()?;
        let target = variant_to_swem_object(&target_var)?;
        map_wmi_target_instance(&target, kind)
    }
}

pub(crate) fn map_wmi_target_instance(
    target: &ISWbemObject,
    kind: DiscoveryEventKind,
) -> Option<DiscoveryEvent> {
    unsafe {
        let props = target.Properties_().ok()?;
        let pid = property_u32(&props, "ProcessId")?;
        let create_time = property_create_time(&props)?;
        map_wmi_process_fields(pid, create_time, kind)
    }
}

pub(crate) fn map_wmi_process_fields(
    pid: u32,
    create_time: i64,
    kind: DiscoveryEventKind,
) -> Option<DiscoveryEvent> {
    if pid == 0 {
        return None;
    }
    Some(match kind {
        DiscoveryEventKind::Spawn => DiscoveryEvent::Spawn { pid, create_time },
        DiscoveryEventKind::Exit => DiscoveryEvent::Exit { pid, create_time },
    })
}

fn property_u32(
    props: &windows::Win32::System::Wmi::ISWbemPropertySet,
    name: &str,
) -> Option<u32> {
    unsafe {
        let prop = props.Item(&BSTR::from(name), 0).ok()?;
        let var = prop.Value().ok()?;
        u32::try_from(&var).ok()
    }
}

fn property_create_time(props: &windows::Win32::System::Wmi::ISWbemPropertySet) -> Option<i64> {
    unsafe {
        let prop = props.Item(&BSTR::from("CreationDate"), 0).ok()?;
        let var = prop.Value().ok()?;
        if let Ok(text) = BSTR::try_from(&var) {
            return wmi_datetime_to_create_time(&text.to_string());
        }
        i64::try_from(&var).ok()
    }
}

fn variant_to_swem_object(var: &VARIANT) -> Option<ISWbemObject> {
    IUnknown::try_from(var)
        .ok()
        .and_then(|unk| unk.cast().ok())
}

/// Parse CIM_DATETIME bias suffix (`+UUU` / `-UUU` minutes from UTC).
fn wmi_timezone_bias_minutes(text: &str) -> Option<i64> {
    let sign_idx = text.rfind(['+', '-'])?;
    let sign = text.as_bytes()[sign_idx];
    if sign_idx + 4 > text.len() {
        return None;
    }
    let digits: i64 = text[sign_idx + 1..sign_idx + 4].parse().ok()?;
    Some(if sign == b'-' { -digits } else { digits })
}

/// Convert WMI `CreationDate` (`YYYYMMDDHHmmss.ffffff±UUU`) to NtQSI-style 100 ns ticks since 1601 UTC.
pub(crate) fn wmi_datetime_to_create_time(text: &str) -> Option<i64> {
    if text.len() < 14 {
        return None;
    }
    let year = text[0..4].parse::<i64>().ok()?;
    let month = text[4..6].parse::<i64>().ok()?;
    let day = text[6..8].parse::<i64>().ok()?;
    let hour = text[8..10].parse::<i64>().ok()?;
    let minute = text[10..12].parse::<i64>().ok()?;
    let second = text[12..14].parse::<i64>().ok()?;
    let fraction = if text.len() > 15 && text.as_bytes().get(14) == Some(&b'.') {
        let end = text[15..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| 15 + i)
            .unwrap_or(text.len());
        let frac_digits = end - 15;
        if frac_digits == 0 {
            0
        } else {
            let frac = text[15..end].parse::<i64>().ok()?;
            frac * 10_000_000 / 10i64.pow(frac_digits as u32)
        }
    } else {
        0
    };

    let days = days_since_1601(year, month, day)?;
    let local_ticks =
        (days * 86_400 + hour * 3_600 + minute * 60 + second) * 10_000_000 + fraction;
    let bias_minutes = wmi_timezone_bias_minutes(text).unwrap_or(0);
    // WMI local time + bias → UTC (e.g. -480 means 480 min west of UTC).
    Some(local_ticks - bias_minutes * 60 * 10_000_000)
}

fn days_since_1601(year: i64, month: i64, day: i64) -> Option<i64> {
    if month < 1 || month > 12 || day < 1 || day > 31 {
        return None;
    }
    let mut y = year;
    let mut m = month;
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 584_754;
    Some(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wmi_datetime_maps_to_filetime_ticks() {
        let ticks = wmi_datetime_to_create_time("20240102030405.000000+000").unwrap();
        assert!(ticks > 0);
    }

    #[test]
    fn wmi_datetime_applies_negative_timezone_bias() {
        let utc = wmi_datetime_to_create_time("20240102030405.000000+000").unwrap();
        let west = wmi_datetime_to_create_time("20240102030405.000000-480").unwrap();
        assert_eq!(west - utc, 480 * 60 * 10_000_000);
    }

    #[test]
    fn wmi_datetime_applies_positive_timezone_bias() {
        let utc = wmi_datetime_to_create_time("20240102030405.000000+000").unwrap();
        let east = wmi_datetime_to_create_time("20240102030405.000000+330").unwrap();
        assert_eq!(utc - east, 330 * 60 * 10_000_000);
    }

    #[test]
    fn map_process_fields_spawn_and_exit() {
        assert_eq!(
            map_wmi_process_fields(99, 12_345, DiscoveryEventKind::Spawn),
            Some(DiscoveryEvent::Spawn {
                pid: 99,
                create_time: 12_345
            })
        );
        assert_eq!(
            map_wmi_process_fields(7, 1_234, DiscoveryEventKind::Exit),
            Some(DiscoveryEvent::Exit {
                pid: 7,
                create_time: 1_234
            })
        );
        assert!(map_wmi_process_fields(0, 1, DiscoveryEventKind::Spawn).is_none());
    }

    #[test]
    fn inject_and_poll_spawn_exit() {
        let mut src = WmiProcessSource::new_inject_only();
        src.inject_events([
            DiscoveryEvent::Spawn {
                pid: 3,
                create_time: 30,
            },
            DiscoveryEvent::Exit {
                pid: 4,
                create_time: 40,
            },
        ]);
        assert_eq!(
            src.poll_events(),
            vec![
                DiscoveryEvent::Spawn {
                    pid: 3,
                    create_time: 30
                },
                DiscoveryEvent::Exit {
                    pid: 4,
                    create_time: 40
                },
            ]
        );
        assert!(src.poll_events().is_empty());
    }

    #[test]
    fn degrade_diagnostic_includes_stage_and_code() {
        let err = WmiOpenError {
            stage: "ConnectServer",
            code: 0x80041001,
        };
        let msg = wmi_degrade_diagnostic(&err);
        assert!(msg.contains("ConnectServer"));
        assert!(msg.contains("falling back"));
    }

    #[test]
    fn try_new_reports_open_result() {
        match WmiProcessSource::try_new() {
            Ok(_) => eprintln!("WMI subscriptions opened on this machine (live path)"),
            Err(e) => {
                eprintln!("WMI unavailable as expected: {e}");
                assert!(
                    matches!(
                        e.stage,
                        "CoInitializeEx"
                            | "CoCreateInstance"
                            | "ConnectServer"
                            | "ExecNotificationQuery(create)"
                            | "ExecNotificationQuery(delete)"
                            | "spawn_consumer"
                            | "open_subscriptions"
                    ),
                    "unexpected degrade stage: {}",
                    e.stage
                );
            }
        }
    }
}
