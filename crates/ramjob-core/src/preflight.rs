//! Startup environment preflight (SPEC §5.4) — once per process.

use std::sync::OnceLock;

use crate::diagnostics::DiagnosticsRing;

const ONE_GIB: u64 = 1024 * 1024 * 1024;
const ONE_MIB: u64 = 1024 * 1024;
const HIGH_RAM_THRESHOLD: u64 = 32 * ONE_GIB;
const MIN_PAGEFILE_BYTES: u64 = ONE_GIB;

/// Convert registry `PagingFiles` max megabytes to bytes.
fn mb_to_bytes(mb: u64) -> u64 {
    mb.saturating_mul(ONE_MIB)
}

static CACHED: OnceLock<PreflightReport> = OnceLock::new();

/// Pagefile presence / size classification for §5.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagefileStatus {
    Disabled,
    Small { max_bytes: u64 },
    Ok { max_bytes: u64 },
    Unknown { detail: String },
}

/// Structured §5.4 startup probe results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub total_ram_bytes: u64,
    pub pagefile: PagefileStatus,
    pub elevated: bool,
    /// Human-readable lines for diagnostics clipboard and first-run copy.
    pub notes: Vec<String>,
}

/// Injectable system probe (production Windows APIs or test doubles).
pub trait SysProbe {
    fn total_ram_bytes(&self) -> Result<u64, String>;
    fn pagefile_max_bytes(&self) -> Result<Option<u64>, String>;
    fn is_elevated(&self) -> Result<bool, String>;
}

/// Collect a preflight report from the injected probe.
pub fn collect_with<P: SysProbe + ?Sized>(probe: &P) -> PreflightReport {
    let total_ram_bytes = probe.total_ram_bytes().unwrap_or(0);

    let pagefile = match probe.pagefile_max_bytes() {
        Ok(None) => PagefileStatus::Disabled,
        Ok(Some(0)) => PagefileStatus::Unknown {
            detail: "pagefile configured but size unknown".into(),
        },
        Ok(Some(max_bytes)) if max_bytes < MIN_PAGEFILE_BYTES => PagefileStatus::Small { max_bytes },
        Ok(Some(max_bytes)) => PagefileStatus::Ok { max_bytes },
        Err(e) => PagefileStatus::Unknown {
            detail: e,
        },
    };

    let elevated = probe.is_elevated().unwrap_or(false);

    let notes = build_notes(total_ram_bytes, &pagefile, elevated);
    PreflightReport {
        total_ram_bytes,
        pagefile,
        elevated,
        notes,
    }
}

/// Run §5.4 preflight once per process; later calls return the cached report.
pub fn run_once() -> &'static PreflightReport {
    CACHED.get_or_init(|| collect_with(&WindowsProbe))
}

impl PreflightReport {
    /// Push preflight lines into the §8.1 diagnostics ring (caller-owned ring).
    pub fn push_to_diagnostics(&self, ring: &mut DiagnosticsRing) {
        ring.push("--- startup preflight (§5.4) ---");
        for line in &self.notes {
            ring.push(line.clone());
        }
    }

    /// Short §5.4 copy for the §7.3 first-run panel (not the diagnostics dump).
    pub fn panel_notes(&self) -> Vec<String> {
        format_panel_notes(self.total_ram_bytes, &self.pagefile, self.elevated)
    }
}

fn format_panel_notes(
    total_ram_bytes: u64,
    pagefile: &PagefileStatus,
    elevated: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(line) = pagefile_panel_line(pagefile) {
        out.push(line);
    }
    if let Some(line) = dormancy_panel_line(total_ram_bytes) {
        out.push(line);
    }
    if let Some(line) = privilege_panel_line(elevated) {
        out.push(line);
    }
    out
}

fn pagefile_panel_line(pagefile: &PagefileStatus) -> Option<String> {
    match pagefile {
        PagefileStatus::Disabled => Some(
            "Pagefile is disabled — soft trim has nowhere to spill; prefer backstop-only \
             or a system-managed pagefile."
                .into(),
        ),
        PagefileStatus::Small { .. } => Some(
            "Pagefile is under 1 GB — soft trim spill space is limited; yield may be poor."
                .into(),
        ),
        _ => None,
    }
}

fn pagefile_diagnostic_line(pagefile: &PagefileStatus) -> String {
    match pagefile {
        PagefileStatus::Disabled => {
            "Pagefile: disabled — soft trim has nowhere to spill; yield will be poor; \
             prefer backstop-only or a system-managed pagefile"
                .into()
        }
        PagefileStatus::Small { max_bytes } => format!(
            "Pagefile: {:.1} GB (< 1 GB) — soft trim spill space is limited; yield may be poor",
            gib(*max_bytes)
        ),
        PagefileStatus::Ok { max_bytes } => format!("Pagefile: {:.1} GB", gib(*max_bytes)),
        PagefileStatus::Unknown { detail } => format!("Pagefile: {detail}"),
    }
}

fn dormancy_panel_line(total_ram_bytes: u64) -> Option<String> {
    if total_ram_bytes >= HIGH_RAM_THRESHOLD {
        Some(
            "With 32 GB+ RAM, low-memory pressure rarely arms — RamJob will mostly stay \
             dormant. That's expected, not a bug."
                .into(),
        )
    } else {
        None
    }
}

fn dormancy_diagnostic_line(total_ram_bytes: u64) -> Option<String> {
    if total_ram_bytes >= HIGH_RAM_THRESHOLD {
        Some(
            "Total RAM >= 32 GB — low-memory pressure will rarely arm; RamJob will mostly \
             stay dormant; this is expected, not a bug"
                .into(),
        )
    } else {
        None
    }
}

fn privilege_panel_line(elevated: bool) -> Option<String> {
    if elevated {
        None
    } else {
        Some(
            "Not running as administrator — some apps may appear uncappable.".into(),
        )
    }
}

fn privilege_diagnostic_line(elevated: bool) -> Option<String> {
    if elevated {
        None
    } else {
        Some(
            "Not elevated — cannot limit higher-integrity or other-user processes; \
             those groups appear uncappable"
                .into(),
        )
    }
}

fn build_notes(total_ram_bytes: u64, pagefile: &PagefileStatus, elevated: bool) -> Vec<String> {
    let mut notes = Vec::new();

    notes.push(format!(
        "Total RAM: {:.1} GB",
        gib(total_ram_bytes)
    ));
    notes.push(pagefile_diagnostic_line(pagefile));
    if let Some(line) = dormancy_diagnostic_line(total_ram_bytes) {
        notes.push(line);
    }
    if let Some(line) = privilege_diagnostic_line(elevated) {
        notes.push(line);
    }

    notes
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / ONE_GIB as f64
}

/// Parse `PagingFiles` REG_MULTI_SZ wide chars into summed max MB, or `None` when empty.
fn parse_paging_files_max_mb(chars: &[u16]) -> Option<u64> {
    let text = String::from_utf16_lossy(chars);
    let mut total_mb = 0u64;
    let mut any = false;
    for entry in text.split('\0').filter(|s| !s.is_empty()) {
        any = true;
        let parts: Vec<&str> = entry.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let max_mb = parts[2].parse::<u64>().unwrap_or(0);
        total_mb = total_mb.saturating_add(max_mb);
    }
    if any { Some(total_mb) } else { None }
}

#[cfg(windows)]
mod windows_impl {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_MULTI_SZ, REG_VALUE_TYPE,
    };
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use crate::win_reg::{query_value_buf, query_value_size, reg_ok_str, wide, RegKey};

    use super::{mb_to_bytes, SysProbe};

    const MEMORY_MANAGEMENT_KEY: &str =
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management";
    const PAGING_FILES_VALUE: &str = "PagingFiles";

    struct TokenHandle(windows::Win32::Foundation::HANDLE);

    impl Drop for TokenHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub struct WindowsProbe;

    impl SysProbe for WindowsProbe {
        fn total_ram_bytes(&self) -> Result<u64, String> {
            unsafe {
                let mut status = MEMORYSTATUSEX {
                    dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                    ..Default::default()
                };
                GlobalMemoryStatusEx(&mut status)
                    .map_err(|e| format!("GlobalMemoryStatusEx: {e}"))?;
                Ok(status.ullTotalPhys)
            }
        }

        fn pagefile_max_bytes(&self) -> Result<Option<u64>, String> {
            match read_paging_files_max_mb()? {
                None => memory_status_pagefile_max(),
                Some(0) => memory_status_pagefile_max(),
                Some(total_mb) => Ok(Some(mb_to_bytes(total_mb))),
            }
        }

        fn is_elevated(&self) -> Result<bool, String> {
            unsafe {
                let mut token = windows::Win32::Foundation::HANDLE::default();
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                    .map_err(|e| format!("OpenProcessToken: {e}"))?;
                let token = TokenHandle(token);

                let mut elevation = TOKEN_ELEVATION::default();
                let mut size = 0u32;
                GetTokenInformation(
                    token.0,
                    TokenElevation,
                    Some(&mut elevation as *mut _ as *mut _),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut size,
                )
                .map_err(|e| format!("GetTokenInformation(TokenElevation): {e}"))?;

                Ok(elevation.TokenIsElevated != 0)
            }
        }
    }

    fn memory_status_pagefile_max() -> Result<Option<u64>, String> {
        unsafe {
            let mut status = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            GlobalMemoryStatusEx(&mut status)
                .map_err(|e| format!("GlobalMemoryStatusEx: {e}"))?;
            let pagefile = status.ullTotalPageFile.saturating_sub(status.ullTotalPhys);
            if pagefile == 0 {
                Ok(None)
            } else {
                Ok(Some(pagefile))
            }
        }
    }

    fn read_paging_files_max_mb() -> Result<Option<u64>, String> {
        let path = wide(MEMORY_MANAGEMENT_KEY);
        let mut raw = HKEY::default();
        reg_ok_str(
            unsafe {
                RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR(path.as_ptr()),
                    0,
                    KEY_READ,
                    &mut raw,
                )
            },
            "RegOpenKeyExW(Memory Management)",
        )?;
        let key = RegKey(raw);

        let name = wide(PAGING_FILES_VALUE);
        let mut value_type = REG_VALUE_TYPE::default();
        let mut data_size = 0u32;
        let status = query_value_size(key.0, PCWSTR(name.as_ptr()), &mut value_type, &mut data_size);
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        reg_ok_str(status, "RegQueryValueExW(PagingFiles size)")?;
        if value_type != REG_MULTI_SZ {
            return Err(format!(
                "PagingFiles: expected REG_MULTI_SZ, got {}",
                value_type.0
            ));
        }
        if data_size <= 2 {
            return Ok(None);
        }

        let mut buf = vec![0u8; data_size as usize];
        query_value_buf(
            key.0,
            PCWSTR(name.as_ptr()),
            &mut buf,
            &mut value_type,
            &mut data_size,
        )
        .map_err(|code| format!("RegQueryValueExW(PagingFiles data) failed (win32 {code})"))?;

        let wide_len = buf.len() / 2;
        let chars =
            unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, wide_len) };
        Ok(super::parse_paging_files_max_mb(chars))
    }
}

#[cfg(windows)]
use windows_impl::WindowsProbe;

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, Default)]
struct WindowsProbe;

#[cfg(not(windows))]
impl SysProbe for WindowsProbe {
    fn total_ram_bytes(&self) -> Result<u64, String> {
        Err("preflight requires Windows".into())
    }

    fn pagefile_max_bytes(&self) -> Result<Option<u64>, String> {
        Err("preflight requires Windows".into())
    }

    fn is_elevated(&self) -> Result<bool, String> {
        Err("preflight requires Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProbe {
        ram: u64,
        pagefile: Option<u64>,
        elevated: bool,
    }

    impl SysProbe for MockProbe {
        fn total_ram_bytes(&self) -> Result<u64, String> {
            Ok(self.ram)
        }

        fn pagefile_max_bytes(&self) -> Result<Option<u64>, String> {
            Ok(self.pagefile)
        }

        fn is_elevated(&self) -> Result<bool, String> {
            Ok(self.elevated)
        }
    }

    #[test]
    fn disabled_pagefile_warns() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: None,
            elevated: true,
        });
        assert_eq!(report.pagefile, PagefileStatus::Disabled);
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("Pagefile: disabled"))
        );
    }

    #[test]
    fn registry_mb_512_classifies_as_small() {
        let report = collect_with(&RegistryMbProbe { max_mb: 512 });
        assert!(matches!(report.pagefile, PagefileStatus::Small { max_bytes } if max_bytes == 512 * 1024 * 1024));
    }

    #[test]
    fn registry_mb_2048_classifies_as_ok() {
        let report = collect_with(&RegistryMbProbe { max_mb: 2048 });
        assert!(matches!(report.pagefile, PagefileStatus::Ok { max_bytes } if max_bytes == 2048 * 1024 * 1024));
    }

    struct RegistryMbProbe {
        max_mb: u64,
    }

    impl SysProbe for RegistryMbProbe {
        fn total_ram_bytes(&self) -> Result<u64, String> {
            Ok(16 * ONE_GIB)
        }

        fn pagefile_max_bytes(&self) -> Result<Option<u64>, String> {
            Ok(Some(mb_to_bytes(self.max_mb)))
        }

        fn is_elevated(&self) -> Result<bool, String> {
            Ok(true)
        }
    }

    #[test]
    fn mb_to_bytes_uses_mebibytes_not_gibibytes() {
        assert_eq!(mb_to_bytes(512), 512 * 1024 * 1024);
        assert_ne!(mb_to_bytes(512), 512 * ONE_GIB);
    }

    #[test]
    fn small_pagefile_warns() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(512 * 1024 * 1024),
            elevated: true,
        });
        assert!(matches!(report.pagefile, PagefileStatus::Small { .. }));
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("< 1 GB"))
        );
    }

    #[test]
    fn ok_pagefile_reports_size() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(4 * ONE_GIB),
            elevated: true,
        });
        assert!(matches!(report.pagefile, PagefileStatus::Ok { .. }));
        assert!(report.notes.iter().any(|n| n.contains("Pagefile: 4.0 GB")));
    }

    #[test]
    fn high_ram_adds_dormancy_note() {
        let report = collect_with(&MockProbe {
            ram: 64 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: true,
        });
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("mostly stay dormant"))
        );
    }

    #[test]
    fn low_ram_skips_dormancy_note() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: true,
        });
        assert!(
            !report
                .notes
                .iter()
                .any(|n| n.contains("mostly stay dormant"))
        );
    }

    #[test]
    fn not_elevated_adds_privilege_note() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: false,
        });
        assert!(
            report
                .notes
                .iter()
                .any(|n| n.contains("Not elevated"))
        );
    }

    #[test]
    fn elevated_skips_privilege_note() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: true,
        });
        assert!(!report.notes.iter().any(|n| n.contains("Not elevated")));
    }

    #[test]
    fn panel_notes_include_dormancy_and_privilege() {
        let report = collect_with(&MockProbe {
            ram: 64 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: false,
        });
        let notes = report.panel_notes();
        assert!(notes.iter().any(|n| n.contains("mostly stay dormant")));
        assert!(notes.iter().any(|n| n.contains("administrator")));
        assert!(!notes.iter().any(|n| n.starts_with("Total RAM:")));
    }

    #[test]
    fn panel_notes_warn_on_disabled_pagefile() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: None,
            elevated: true,
        });
        let notes = report.panel_notes();
        assert!(notes.iter().any(|n| n.contains("Pagefile is disabled")));
    }

    #[test]
    fn panel_notes_empty_when_all_clear() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: true,
        });
        assert!(report.panel_notes().is_empty());
    }

    #[test]
    fn push_to_diagnostics_includes_header_and_notes() {
        let report = collect_with(&MockProbe {
            ram: 16 * ONE_GIB,
            pagefile: Some(8 * ONE_GIB),
            elevated: false,
        });
        let mut ring = DiagnosticsRing::new();
        report.push_to_diagnostics(&mut ring);
        let lines = ring.lines();
        assert_eq!(lines[0], "--- startup preflight (§5.4) ---");
        assert_eq!(lines.len(), 1 + report.notes.len());
    }

    #[cfg(windows)]
    #[test]
    fn run_once_smoke_on_host() {
        let report = run_once();
        assert!(!report.notes.is_empty(), "preflight notes should be non-empty");
        assert!(report.total_ram_bytes > 0, "total RAM should be detected");
        // Cached: same pointer on second call.
        assert!(std::ptr::eq(report, run_once()));
    }

    #[cfg(windows)]
    mod windows_unit {
        use super::parse_paging_files_max_mb;

        fn wide(s: &str) -> Vec<u16> {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            OsStr::new(s).encode_wide().chain(Some(0)).collect()
        }

        #[test]
        fn parse_paging_files_sums_max_mb() {
            let chars = wide("C:\\pagefile.sys 2048 4096\0D:\\pagefile.sys 1024 2048\0\0");
            assert_eq!(parse_paging_files_max_mb(&chars), Some(6144));
        }

        #[test]
        fn parse_paging_files_empty_is_none() {
            let chars = wide("\0\0");
            assert_eq!(parse_paging_files_max_mb(&chars), None);
        }
    }
}
