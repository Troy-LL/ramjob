//! Minimal Win32 registry helpers shared by autostart and preflight.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{RegCloseKey, HKEY};

/// RAII wrapper that closes an opened registry key on drop.
pub struct RegKey(pub HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Null-terminated UTF-16 path or value name for registry APIs.
pub fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Map a Win32 status to `Ok(())` or the raw error code.
pub fn reg_status(status: WIN32_ERROR) -> Result<(), u32> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(status.0)
    }
}

/// Map a Win32 status to `Ok(())` or a human-readable `String` error.
pub fn reg_ok_str(status: WIN32_ERROR, context: &str) -> Result<(), String> {
    reg_status(status).map_err(|code| format!("{context} failed (win32 {code})"))
}

/// Two-phase `RegQueryValueExW`: size probe then data read into `buf`.
pub fn query_value_buf(
    key: HKEY,
    name: PCWSTR,
    buf: &mut [u8],
    value_type: &mut windows::Win32::System::Registry::REG_VALUE_TYPE,
    data_size: &mut u32,
) -> Result<(), u32> {
    use windows::Win32::System::Registry::RegQueryValueExW;

    reg_status(unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            Some(value_type),
            Some(buf.as_mut_ptr()),
            Some(data_size),
        )
    })
}

/// Size-only `RegQueryValueExW` probe (no data buffer).
pub fn query_value_size(
    key: HKEY,
    name: PCWSTR,
    value_type: &mut windows::Win32::System::Registry::REG_VALUE_TYPE,
    data_size: &mut u32,
) -> WIN32_ERROR {
    use windows::Win32::System::Registry::RegQueryValueExW;

    unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            Some(value_type),
            None,
            Some(data_size),
        )
    }
}
