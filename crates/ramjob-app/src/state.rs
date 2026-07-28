//! App-wide state shared with Tauri commands (Task 6).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ramjob_core::config::{default_config_template, load_config_file, parse_config, RamjobConfig};
use ramjob_core::panel::{PanelGroup, PanelState};
use ramjob_core::pressure::{PressureSource, SimulatedPressure, WinPressure};
use ramjob_core::runtime::Runtime;
use ramjob_core::sys_history::SysHistory;

/// `%APPDATA%\RamJob\config.toml` — same layout as `ramjob run` (M2 CLI).
pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("RamJob").join("config.toml")
}

/// Load `path`, or create its directory + a fresh default config if missing.
fn ensure_config(path: &Path) -> Result<RamjobConfig, String> {
    if path.exists() {
        return load_config_file(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir {}: {e}", parent.display()))?;
    }
    let template = default_config_template();
    std::fs::write(path, &template)
        .map_err(|e| format!("write config {}: {e}", path.display()))?;
    parse_config(&template)
}

pub struct AppStateInner {
    pub panel: PanelState,
    pub runtime: Runtime,
    pub pressure: Box<dyn PressureSource + Send>,
    /// Latest tick's inputs to `build_snapshot`, refreshed by the 1s poll loop
    /// while the panel is visible; commands read these instead of re-sampling.
    pub last_used_bytes: u64,
    pub last_total_bytes: u64,
    pub last_groups: Vec<PanelGroup>,
}

pub struct AppState(pub Mutex<AppStateInner>);

impl AppState {
    pub fn new() -> Result<Self, String> {
        Self::at_path(&default_config_path())
    }

    pub fn at_path(config_path: &Path) -> Result<Self, String> {
        let config = ensure_config(config_path)?;
        let runtime = Runtime::from_config(config.clone());
        let panel = PanelState {
            config_path: config_path.to_path_buf(),
            config,
            history: SysHistory::new(),
        };
        // ponytail: fall back to a Disarmed-leaning simulated source rather than
        // failing app startup when WinPressure notifications can't be created
        // (matches `ramjob run`'s existing fallback behavior).
        let pressure: Box<dyn PressureSource + Send> = match WinPressure::new() {
            Ok(w) => Box::new(w),
            Err(_) => Box::new(SimulatedPressure {
                low_memory: false,
                high_memory: true,
                hard_faults_per_sec: 0.0,
            }),
        };
        Ok(Self(Mutex::new(AppStateInner {
            panel,
            runtime,
            pressure,
            last_used_bytes: 0,
            last_total_bytes: 0,
            last_groups: Vec::new(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramjob_appstate_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    #[test]
    fn creates_missing_config_dir_and_default_template() {
        let path = temp_config_path("create");
        assert!(!path.exists());

        let state = AppState::at_path(&path).unwrap();
        assert!(path.exists(), "config.toml should be created on first run");

        let inner = state.0.lock().unwrap();
        assert_eq!(inner.panel.config.version, ramjob_core::config::CONFIG_VERSION);
        assert!(inner.panel.config.groups.is_empty());
        assert_eq!(inner.panel.config_path, path);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn loads_existing_config_without_overwriting() {
        let path = temp_config_path("load");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "version = 2\nrunaway_multiplier = 5.0\n").unwrap();

        let state = AppState::at_path(&path).unwrap();
        let inner = state.0.lock().unwrap();
        assert_eq!(inner.panel.config.runaway_multiplier, 5.0);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
