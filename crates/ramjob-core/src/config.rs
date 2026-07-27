//! Versioned RamJob config.toml parsing (M2).

use std::path::Path;

use serde::Deserialize;

pub const DEFAULT_RUNAWAY_MULTIPLIER: f64 = 3.0;
pub const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct RamjobConfig {
    pub version: u32,
    pub runaway_multiplier: f64,
    pub groups: Vec<GroupConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupConfig {
    pub key: String,
    pub cap_bytes: u64,
    pub always_enforce: bool,
}

#[derive(Debug, Deserialize)]
struct ConfigToml {
    version: u32,
    #[serde(default = "default_runaway_multiplier")]
    runaway_multiplier: f64,
    #[serde(default)]
    group: Vec<GroupConfigToml>,
}

#[derive(Debug, Deserialize)]
struct GroupConfigToml {
    key: String,
    #[serde(default)]
    cap_bytes: u64,
    #[serde(default)]
    always_enforce: bool,
}

fn default_runaway_multiplier() -> f64 {
    DEFAULT_RUNAWAY_MULTIPLIER
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
        groups: raw
            .group
            .into_iter()
            .map(|g| GroupConfig {
                key: g.key,
                cap_bytes: g.cap_bytes,
                always_enforce: g.always_enforce,
            })
            .collect(),
    })
}

pub fn load_config_file(path: &Path) -> Result<RamjobConfig, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config {}: {e}", path.display()))?;
    parse_config(&contents)
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
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.groups[0].cap_bytes, 4294967296);
    }

    #[test]
    fn rejects_unknown_version() {
        assert!(parse_config("version = 99\n").is_err());
    }

    #[test]
    fn missing_multiplier_defaults_to_3() {
        let c = parse_config("version = 2\n").unwrap();
        assert_eq!(c.runaway_multiplier, DEFAULT_RUNAWAY_MULTIPLIER);
    }
}
