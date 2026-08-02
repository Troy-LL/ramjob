//! HKCU Run autostart helper (SPEC §8 — `RamJob` value under CurrentVersion\Run).

use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE,
};

use crate::win_reg::{query_value_buf, query_value_size, reg_status, wide, RegKey};

/// Run value name written under HKCU `...\CurrentVersion\Run`.
pub const VALUE_NAME: &str = "RamJob";

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Injectable registry backend for unit tests and production HKCU Run access.
pub trait RunRegistry {
    fn get_value(&self, name: &str) -> Result<Option<String>, AutostartError>;
    fn set_value(&self, name: &str, value: &str) -> Result<(), AutostartError>;
    fn delete_value(&self, name: &str) -> Result<(), AutostartError>;
}

/// HKCU `Software\Microsoft\Windows\CurrentVersion\Run`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HkcuRunRegistry;

impl RunRegistry for HkcuRunRegistry {
    fn get_value(&self, name: &str) -> Result<Option<String>, AutostartError> {
        with_run_key(KEY_QUERY_VALUE, |key| query_sz_value(key, name))
    }

    fn set_value(&self, name: &str, value: &str) -> Result<(), AutostartError> {
        with_run_key(KEY_SET_VALUE, |key| write_sz_value(key, name, value))
    }

    fn delete_value(&self, name: &str) -> Result<(), AutostartError> {
        with_run_key(KEY_SET_VALUE, |key| {
            let name_w = wide(name);
            let status = unsafe { RegDeleteValueW(key.0, PCWSTR(name_w.as_ptr())) };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            reg_ok(status, "RegDeleteValueW")
        })
    }
}

#[derive(Debug)]
pub enum AutostartError {
    Io(std::io::Error),
    Registry { code: u32, context: &'static str },
    UnexpectedValueType { expected: &'static str, got: u32 },
}

impl std::fmt::Display for AutostartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Registry { code, context } => {
                write!(f, "{context} failed (win32 {code})")
            }
            Self::UnexpectedValueType { expected, got } => {
                write!(f, "expected registry type {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for AutostartError {}

impl From<std::io::Error> for AutostartError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Format an executable path the way Windows Run values expect (quoted).
pub fn quoted_exe_path(path: &Path) -> String {
    let lossy = path.to_string_lossy();
    if lossy.starts_with('"') && lossy.ends_with('"') {
        lossy.into_owned()
    } else {
        format!("\"{lossy}\"")
    }
}

/// Returns whether the `RamJob` Run value is present.
pub fn is_enabled() -> Result<bool, AutostartError> {
    is_enabled_with(&HkcuRunRegistry)
}

/// Returns whether `name` is present in the injected registry backend.
pub fn is_enabled_with<R: RunRegistry + ?Sized>(registry: &R) -> Result<bool, AutostartError> {
    Ok(registry.get_value(VALUE_NAME)?.is_some())
}

/// Writes the quoted current executable path to the `RamJob` Run value.
pub fn enable() -> Result<(), AutostartError> {
    let exe = std::env::current_exe()?;
    enable_with(&HkcuRunRegistry, &exe)
}

/// Writes a quoted `exe` path to the `RamJob` Run value.
pub fn enable_with<R: RunRegistry + ?Sized>(
    registry: &R,
    exe: &Path,
) -> Result<(), AutostartError> {
    registry.set_value(VALUE_NAME, &quoted_exe_path(exe))
}

/// Removes the `RamJob` Run value if present.
pub fn disable() -> Result<(), AutostartError> {
    disable_with(&HkcuRunRegistry)
}

/// Removes the `RamJob` Run value via the injected registry backend.
pub fn disable_with<R: RunRegistry + ?Sized>(registry: &R) -> Result<(), AutostartError> {
    registry.delete_value(VALUE_NAME)
}

fn with_run_key<T>(
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
    f: impl FnOnce(RegKey) -> Result<T, AutostartError>,
) -> Result<T, AutostartError> {
    let path = wide(RUN_KEY);
    let mut raw = HKEY::default();
    reg_ok(
        unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                0,
                access,
                &mut raw,
            )
        },
        "RegOpenKeyExW",
    )?;
    f(RegKey(raw))
}

fn query_sz_value(key: RegKey, name: &str) -> Result<Option<String>, AutostartError> {
    let name_w = wide(name);
    let mut value_type = REG_VALUE_TYPE::default();
    let mut data_size = 0u32;

    let status = query_value_size(key.0, PCWSTR(name_w.as_ptr()), &mut value_type, &mut data_size);
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    reg_ok(status, "RegQueryValueExW(size)")?;
    if value_type != REG_SZ {
        return Err(AutostartError::UnexpectedValueType {
            expected: "REG_SZ",
            got: value_type.0,
        });
    }

    let mut buf = vec![0u8; data_size as usize];
    reg_ok_code(
        query_value_buf(
            key.0,
            PCWSTR(name_w.as_ptr()),
            &mut buf,
            &mut value_type,
            &mut data_size,
        ),
        "RegQueryValueExW(data)",
    )?;

    let wide_len = buf.len() / 2;
    let chars =
        unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, wide_len) };
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    Ok(Some(String::from_utf16_lossy(&chars[..end])))
}

fn write_sz_value(key: RegKey, name: &str, value: &str) -> Result<(), AutostartError> {
    let name_w = wide(name);
    let value_w = wide(value);
    let bytes = unsafe {
        std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2)
    };
    reg_ok(
        unsafe {
            RegSetValueExW(
                key.0,
                PCWSTR(name_w.as_ptr()),
                0,
                REG_SZ,
                Some(bytes),
            )
        },
        "RegSetValueExW",
    )
}

fn reg_ok(
    status: windows::Win32::Foundation::WIN32_ERROR,
    context: &'static str,
) -> Result<(), AutostartError> {
    reg_status(status).map_err(|code| AutostartError::Registry { code, context })
}

fn reg_ok_code(result: Result<(), u32>, context: &'static str) -> Result<(), AutostartError> {
    result.map_err(|code| AutostartError::Registry { code, context })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockRegistry {
        values: Arc<Mutex<HashMap<String, String>>>,
    }

    impl RunRegistry for MockRegistry {
        fn get_value(&self, name: &str) -> Result<Option<String>, AutostartError> {
            Ok(self.values.lock().unwrap().get(name).cloned())
        }

        fn set_value(&self, name: &str, value: &str) -> Result<(), AutostartError> {
            self.values
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }

        fn delete_value(&self, name: &str) -> Result<(), AutostartError> {
            self.values.lock().unwrap().remove(name);
            Ok(())
        }
    }

    #[test]
    fn quoted_exe_path_wraps_unquoted_paths() {
        let path = PathBuf::from(r"C:\Program Files\RamJob\ramjob.exe");
        assert_eq!(
            quoted_exe_path(&path),
            r#""C:\Program Files\RamJob\ramjob.exe""#
        );
    }

    #[test]
    fn quoted_exe_path_preserves_already_quoted() {
        let path = PathBuf::from(r#""C:\already\quoted.exe""#);
        assert_eq!(
            quoted_exe_path(&path),
            r#""C:\already\quoted.exe""#
        );
    }

    #[test]
    fn enable_writes_quoted_path_to_ramjob_value() {
        let registry = MockRegistry::default();
        let exe = PathBuf::from(r"C:\build\ramjob.exe");

        enable_with(&registry, &exe).unwrap();

        assert_eq!(
            registry.get_value(VALUE_NAME).unwrap(),
            Some(r#""C:\build\ramjob.exe""#.to_owned())
        );
    }

    #[test]
    fn disable_removes_ramjob_value() {
        let registry = MockRegistry::default();
        enable_with(&registry, &PathBuf::from(r"C:\build\ramjob.exe")).unwrap();

        disable_with(&registry).unwrap();

        assert_eq!(registry.get_value(VALUE_NAME).unwrap(), None);
    }

    #[test]
    fn is_enabled_reflects_value_presence() {
        let registry = MockRegistry::default();
        assert!(!is_enabled_with(&registry).unwrap());

        enable_with(&registry, &PathBuf::from(r"C:\build\ramjob.exe")).unwrap();
        assert!(is_enabled_with(&registry).unwrap());

        disable_with(&registry).unwrap();
        assert!(!is_enabled_with(&registry).unwrap());
    }

    #[test]
    fn disable_is_idempotent_when_value_missing() {
        let registry = MockRegistry::default();
        disable_with(&registry).unwrap();
        assert!(!is_enabled_with(&registry).unwrap());
    }
}
