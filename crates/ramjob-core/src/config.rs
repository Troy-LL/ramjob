//! Versioned RamJob config.toml parsing (M2).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_RUNAWAY_MULTIPLIER: f64 = 3.0;
pub const CONFIG_VERSION: u32 = 2;
/// Groups unseen for longer than this (SPEC §8.3) are dropped on prune.
pub const PRUNE_STALE_SECONDS: u64 = 90 * 86400;

#[derive(Debug, Clone, PartialEq)]
pub struct RamjobConfig {
    pub version: u32,
    pub runaway_multiplier: f64,
    pub overall_limit_bytes: u64,
    pub groups: Vec<GroupConfig>,
    pub pause_all: bool,
    pub autostart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupConfig {
    pub key: String,
    pub cap_bytes: u64,
    pub always_enforce: bool,
    pub pinned: bool,
    /// Unix seconds when the group was last observed; 0 = unknown/legacy (never pruned).
    pub last_seen_unix: u64,
}

impl Default for RamjobConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            runaway_multiplier: DEFAULT_RUNAWAY_MULTIPLIER,
            overall_limit_bytes: 0,
            groups: vec![],
            pause_all: false,
            autostart: false,
        }
    }
}

impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            key: String::new(),
            cap_bytes: 0,
            always_enforce: false,
            pinned: false,
            last_seen_unix: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfigToml {
    version: u32,
    #[serde(default = "default_runaway_multiplier")]
    runaway_multiplier: f64,
    #[serde(default)]
    overall_limit_bytes: u64,
    #[serde(default)]
    group: Vec<GroupConfigToml>,
    #[serde(default)]
    pause_all: bool,
    #[serde(default)]
    autostart: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct GroupConfigToml {
    key: String,
    #[serde(default)]
    cap_bytes: u64,
    #[serde(default)]
    always_enforce: bool,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    last_seen_unix: u64,
}

fn default_runaway_multiplier() -> f64 {
    DEFAULT_RUNAWAY_MULTIPLIER
}

/// Empty default config body (SPEC §8.3).
pub fn default_config_template() -> String {
    format!(
        "version = {CONFIG_VERSION}\nrunaway_multiplier = {DEFAULT_RUNAWAY_MULTIPLIER}\n"
    )
}

fn config_backup_path(path: &Path) -> PathBuf {
    path.parent()
        .map(|d| d.join("config.bak"))
        .unwrap_or_else(|| PathBuf::from("config.bak"))
}

/// Backup existing file beside `path` as `config.bak`, then write a fresh empty default config.
pub fn backup_and_regenerate_config(path: &Path, previous_contents: &str) -> Result<RamjobConfig, String> {
    let bak = config_backup_path(path);
    std::fs::write(&bak, previous_contents).map_err(|e| {
        format!(
            "failed to backup config to {}: {e}",
            bak.display()
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir {}: {e}", parent.display()))?;
    }
    let template = default_config_template();
    std::fs::write(path, &template)
        .map_err(|e| format!("failed to write regenerated config {}: {e}", path.display()))?;
    parse_config(&template)
}

pub fn parse_config(toml_str: &str) -> Result<RamjobConfig, String> {
    let raw: ConfigToml =
        toml::from_str(toml_str).map_err(|e| format!("config parse error: {e}"))?;
    if raw.version != CONFIG_VERSION {
        return Err(format!(
            "unsupported config version {}; expected {}",
            raw.version, CONFIG_VERSION
        ));
    }
    Ok(RamjobConfig {
        version: raw.version,
        runaway_multiplier: raw.runaway_multiplier,
        overall_limit_bytes: raw.overall_limit_bytes,
        groups: raw
            .group
            .into_iter()
            .map(|g| GroupConfig {
                key: g.key,
                cap_bytes: g.cap_bytes,
                always_enforce: g.always_enforce,
                pinned: g.pinned,
                last_seen_unix: g.last_seen_unix,
            })
            .collect(),
        pause_all: raw.pause_all,
        autostart: raw.autostart,
    })
}

pub fn load_config_file(path: &Path) -> Result<RamjobConfig, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config {}: {e}", path.display()))?;
    match parse_config(&contents) {
        Ok(cfg) => Ok(cfg),
        Err(e) if e.contains("unsupported config version") => {
            backup_and_regenerate_config(path, &contents)
        }
        Err(e) => Err(e),
    }
}

/// Drop stale group entries per SPEC §8.3: unpinned groups with a known `last_seen_unix`
/// older than 90 days are removed; pinned groups and legacy entries (`last_seen_unix == 0`)
/// are kept.
pub fn prune_stale_groups(cfg: &mut RamjobConfig, now_unix: u64) {
    cfg.groups.retain(|g| {
        g.pinned
            || g.last_seen_unix == 0
            || now_unix.saturating_sub(g.last_seen_unix) <= PRUNE_STALE_SECONDS
    });
}

/// Write config atomically: write to .toml.tmp, then rename over original.
pub fn save_config_atomic(path: &Path, cfg: &RamjobConfig) -> Result<(), String> {
    let toml_cfg = ConfigToml {
        version: cfg.version,
        runaway_multiplier: cfg.runaway_multiplier,
        overall_limit_bytes: cfg.overall_limit_bytes,
        group: cfg
            .groups
            .iter()
            .map(|g| GroupConfigToml {
                key: g.key.clone(),
                cap_bytes: g.cap_bytes,
                always_enforce: g.always_enforce,
                pinned: g.pinned,
                last_seen_unix: g.last_seen_unix,
            })
            .collect(),
        pause_all: cfg.pause_all,
        autostart: cfg.autostart,
    };

    let toml_str = toml::to_string(&toml_cfg)
        .map_err(|e| format!("failed to serialize config: {e}"))?;

    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &toml_str)
        .map_err(|e| format!("failed to write temp config {}: {e}", tmp_path.display()))?;

    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("failed to rename config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_groups_and_defaults() {
        let c = parse_config(
            r#"
version = 2
runaway_multiplier = 3.0
[[group]]
key = "c:\\users\\x\\bravesoftware"
cap_bytes = 4294967296
always_enforce = false
"#,
        )
        .unwrap();
        assert_eq!(c.version, 2);
        assert_eq!(c.runaway_multiplier, 3.0);
        assert_eq!(c.overall_limit_bytes, 0);
        assert_eq!(c.pause_all, false);
        assert_eq!(c.autostart, false);
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.groups[0].cap_bytes, 4294967296);
        assert_eq!(c.groups[0].pinned, false);
        assert_eq!(c.groups[0].last_seen_unix, 0);
    }

    #[test]
    fn unknown_version_backups_and_regenerates() {
        let dir = std::env::temp_dir().join(format!("ramjob_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "version = 99\n[[group]]\nkey = \"old\"\n").unwrap();

        let cfg = load_config_file(&path).unwrap();
        assert_eq!(cfg.version, CONFIG_VERSION);
        assert!(cfg.groups.is_empty());
        assert_eq!(cfg.overall_limit_bytes, 0);
        assert_eq!(cfg.pause_all, false);
        assert_eq!(cfg.autostart, false);

        let bak = dir.join("config.bak");
        let bak_body = std::fs::read_to_string(&bak).unwrap();
        assert!(bak_body.contains("version = 99"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_multiplier_defaults_to_3() {
        let c = parse_config("version = 2\n").unwrap();
        assert_eq!(c.runaway_multiplier, DEFAULT_RUNAWAY_MULTIPLIER);
        assert!(!c.autostart);
    }

    #[test]
    fn legacy_config_without_new_fields_parses() {
        let c = parse_config(
            r#"
version = 2
runaway_multiplier = 3.0
pause_all = true
[[group]]
key = "legacy"
cap_bytes = 100
always_enforce = true
"#,
        )
        .unwrap();
        assert!(c.pause_all);
        assert!(!c.autostart);
        assert_eq!(c.groups[0].pinned, false);
        assert_eq!(c.groups[0].last_seen_unix, 0);
    }

    #[test]
    fn autostart_round_trips_through_save() {
        let dir = std::env::temp_dir().join(format!("ramjob_autostart_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let original = RamjobConfig {
            autostart: true,
            ..Default::default()
        };
        save_config_atomic(&path, &original).unwrap();
        let reloaded = load_config_file(&path).unwrap();
        assert!(reloaded.autostart);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_config_atomic_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ramjob_save_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let original = RamjobConfig {
            runaway_multiplier: 2.5,
            overall_limit_bytes: 4 * 1024 * 1024 * 1024,
            groups: vec![GroupConfig {
                key: "test_group".into(),
                cap_bytes: 1024 * 1024 * 1024,
                always_enforce: true,
                pinned: true,
                last_seen_unix: 1_700_000_000,
            }],
            pause_all: true,
            autostart: false,
            ..Default::default()
        };

        save_config_atomic(&path, &original).unwrap();

        let tmp_path = path.with_extension("toml.tmp");
        assert!(!tmp_path.exists(), ".toml.tmp should be renamed away");

        let reloaded = load_config_file(&path).unwrap();
        assert_eq!(reloaded, original);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_drops_stale_unpinned_groups() {
        let now = 1_800_000_000u64;
        let stale = now - PRUNE_STALE_SECONDS - 1;
        let fresh = now - PRUNE_STALE_SECONDS;

        let mut cfg = RamjobConfig {
            groups: vec![
                GroupConfig {
                    key: "stale".into(),
                    last_seen_unix: stale,
                    ..Default::default()
                },
                GroupConfig {
                    key: "fresh".into(),
                    last_seen_unix: fresh,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        prune_stale_groups(&mut cfg, now);
        assert_eq!(cfg.groups.len(), 1);
        assert_eq!(cfg.groups[0].key, "fresh");
    }

    #[test]
    fn prune_keeps_pinned_and_legacy_groups() {
        let now = 1_800_000_000u64;
        let stale = now - PRUNE_STALE_SECONDS - 1;

        let mut cfg = RamjobConfig {
            groups: vec![
                GroupConfig {
                    key: "pinned".into(),
                    pinned: true,
                    last_seen_unix: stale,
                    ..Default::default()
                },
                GroupConfig {
                    key: "legacy".into(),
                    last_seen_unix: 0,
                    ..Default::default()
                },
                GroupConfig {
                    key: "gone".into(),
                    last_seen_unix: stale,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        prune_stale_groups(&mut cfg, now);
        assert_eq!(cfg.groups.len(), 2);
        let keys: Vec<_> = cfg.groups.iter().map(|g| g.key.as_str()).collect();
        assert!(keys.contains(&"pinned"));
        assert!(keys.contains(&"legacy"));
        assert!(!keys.contains(&"gone"));
    }
}
