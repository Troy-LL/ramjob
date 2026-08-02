//! App-wide state shared with Tauri commands (Task 6).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ramjob_core::autostart;
use ramjob_core::config::{
    default_config_template, load_config_file, parse_config, prune_stale_groups,
    save_config_atomic, RamjobConfig,
};
use ramjob_core::panel::{PanelGroup, PanelState};
use ramjob_core::preflight;
use ramjob_core::pressure::{PressureSource, SimulatedPressure, WinPressure};
use ramjob_core::runtime::Runtime;
use ramjob_core::sys_history::SysHistory;

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Align HKCU Run with `config.autostart`.
pub fn sync_autostart_run(enabled: bool) -> Result<(), String> {
    if enabled {
        autostart::enable().map_err(|e| e.to_string())
    } else {
        autostart::disable().map_err(|e| e.to_string())
    }
}

fn prune_and_save_if_needed(path: &Path, cfg: &mut RamjobConfig, now: u64) -> Result<(), String> {
    let before = cfg.groups.len();
    prune_stale_groups(cfg, now);
    if cfg.groups.len() != before {
        save_config_atomic(path, cfg)?;
    }
    Ok(())
}

/// Prune stale groups on load, persist if needed, and sync HKCU Run to `autostart`.
pub fn maintain_config_on_startup(path: &Path, cfg: RamjobConfig) -> Result<RamjobConfig, String> {
    let mut cfg = cfg;
    prune_and_save_if_needed(path, &mut cfg, now_unix_secs())?;
    sync_autostart_run(cfg.autostart)?;
    Ok(cfg)
}

/// Persist `config.autostart` and sync HKCU Run.
pub fn set_autostart(path: &Path, cfg: &mut RamjobConfig, enabled: bool) -> Result<(), String> {
    if cfg.autostart == enabled {
        return Ok(());
    }
    cfg.autostart = enabled;
    save_config_atomic(path, cfg)?;
    sync_autostart_run(enabled)
}

/// Refresh `last_seen_unix` for config groups observed this tick; save if any changed.
pub fn touch_observed_groups(
    path: &Path,
    cfg: &mut RamjobConfig,
    observed_keys: &[String],
    now_unix: u64,
) -> Result<(), String> {
    let mut changed = false;
    for key in observed_keys {
        if let Some(g) = cfg.groups.iter_mut().find(|g| g.key == *key) {
            if g.last_seen_unix != now_unix {
                g.last_seen_unix = now_unix;
                changed = true;
            }
        }
    }
    if changed {
        save_config_atomic(path, cfg)?;
    }
    Ok(())
}

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
        let config = maintain_config_on_startup(config_path, config)?;
        let mut runtime = Runtime::new();
        preflight::run_once().push_to_diagnostics(&mut runtime.diagnostics);
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
    use ramjob_core::config::{GroupConfig, PRUNE_STALE_SECONDS};

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
        assert!(!inner.panel.config.autostart);
        assert_eq!(inner.panel.config_path, path);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn startup_prune_drops_stale_groups_and_persists() {
        let path = temp_config_path("prune_startup");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let now = now_unix_secs();
        let stale = now - PRUNE_STALE_SECONDS - 1;
        let mut cfg = RamjobConfig::default();
        cfg.groups.push(GroupConfig {
            key: "stale".into(),
            last_seen_unix: stale,
            ..Default::default()
        });
        save_config_atomic(&path, &cfg).unwrap();

        let state = AppState::at_path(&path).unwrap();
        let inner = state.0.lock().unwrap();
        assert!(inner.panel.config.groups.is_empty());

        let reloaded = load_config_file(&path).unwrap();
        assert!(reloaded.groups.is_empty());

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn touch_observed_groups_updates_last_seen_and_persists() {
        let path = temp_config_path("touch_seen");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut cfg = RamjobConfig::default();
        cfg.groups.push(GroupConfig {
            key: "hog".into(),
            last_seen_unix: 1,
            ..Default::default()
        });
        save_config_atomic(&path, &cfg).unwrap();

        let now = 1_800_000_000u64;
        touch_observed_groups(&path, &mut cfg, &["hog".into()], now).unwrap();
        assert_eq!(cfg.groups[0].last_seen_unix, now);

        let reloaded = load_config_file(&path).unwrap();
        assert_eq!(reloaded.groups[0].last_seen_unix, now);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn set_autostart_persists_config_flag() {
        let path = temp_config_path("set_autostart");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut cfg = RamjobConfig::default();
        save_config_atomic(&path, &cfg).unwrap();

        set_autostart(&path, &mut cfg, true).unwrap();
        assert!(cfg.autostart);
        let reloaded = load_config_file(&path).unwrap();
        assert!(reloaded.autostart);

        set_autostart(&path, &mut cfg, false).unwrap();
        assert!(!cfg.autostart);

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
