//! Panel snapshot + mutators for the tray UI (M3).

use std::path::PathBuf;

use crate::cap_math::clamp_cap_with_policy;
use crate::config::{save_config_atomic, GroupConfig, RamjobConfig};
use crate::diagnostics::DiagnosticsRing;
use crate::policy::SystemArm;
use crate::sys_history::{CeilingEdit, SysHistory, SysSample};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PanelSnapshot {
    pub system_arm: String, // "Armed" | "Disarmed"
    pub pause_all: bool,
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub overall_limit_bytes: u64,
    pub status_line: String,
    pub warning: bool, // any LOW_YIELD/THRASHING
    /// True while no per-app caps are set (SPEC §7.3 first-run).
    pub first_run: bool,
    /// User-facing §5.4 warnings for the first-run panel (dormancy, pagefile, privilege).
    pub preflight_notes: Vec<String>,
    pub samples: Vec<SysSample>,
    pub ceiling_edits: Vec<CeilingEdit>,
    pub groups: Vec<PanelGroup>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PanelGroup {
    pub key: String,
    pub name: String, // basename display
    pub gf_bytes: u64,
    pub cap_bytes: u64,
    pub always_enforce: bool,
    pub fsm_hint: String, // "Idle"|"Pressure"|"Trim"|"LowYield"|"Thrashing"|...
}

pub struct PanelState {
    pub config_path: PathBuf,
    pub config: RamjobConfig,
    pub history: SysHistory,
}

impl PanelState {
    fn upsert_group(&mut self, key: &str) -> &mut GroupConfig {
        if let Some(i) = self.config.groups.iter().position(|g| g.key == key) {
            return &mut self.config.groups[i];
        }
        self.config.groups.push(GroupConfig {
            key: key.to_string(),
            cap_bytes: 0,
            always_enforce: false,
            ..Default::default()
        });
        self.config.groups.last_mut().expect("just pushed")
    }

    /// Upsert a group's cap by key, snapping + flooring via `cap_math`, then persist.
    pub fn set_cap(
        &mut self,
        key: &str,
        raw_cap: u64,
        shift_fine: bool,
        median_gf: Option<u64>,
    ) -> Result<(), String> {
        let cap = clamp_cap_with_policy(raw_cap, shift_fine, median_gf);
        self.upsert_group(key).cap_bytes = cap;
        save_config_atomic(&self.config_path, &self.config)
    }

    /// Set the overall memory ceiling. Snaps (no app floor), records a history
    /// tick, and persists config. Never touches `PolicyState`/arming.
    pub fn set_overall_limit(
        &mut self,
        raw: u64,
        now_unix_ms: u64,
        shift_fine: bool,
    ) -> Result<(), String> {
        let cap = crate::cap_math::snap_ceiling_bytes(raw, shift_fine);
        self.config.overall_limit_bytes = cap;
        self.history.commit_ceiling(CeilingEdit {
            unix_ms: now_unix_ms,
            overall_limit_bytes: cap,
        });
        save_config_atomic(&self.config_path, &self.config)
    }

    /// Set a group's `always_enforce` flag (opt-in hard backstop), persisting config.
    /// Creates the group with a zero cap if it doesn't exist yet, same as `set_cap`.
    pub fn set_flags(&mut self, key: &str, always_enforce: bool) -> Result<(), String> {
        self.upsert_group(key).always_enforce = always_enforce;
        save_config_atomic(&self.config_path, &self.config)
    }

    pub fn set_pause_all(&mut self, pause: bool) -> Result<(), String> {
        self.config.pause_all = pause;
        save_config_atomic(&self.config_path, &self.config)
    }

    pub fn build_snapshot(
        &self,
        system: SystemArm,
        used_bytes: u64,
        total_bytes: u64,
        groups: &[PanelGroup],
    ) -> PanelSnapshot {
        let pause_all = self.config.pause_all;
        let system_arm = match system {
            SystemArm::Armed => "Armed",
            SystemArm::Disarmed => "Disarmed",
        };
        let status_line = if pause_all {
            "Paused — enforcement off".to_string()
        } else if system == SystemArm::Armed {
            "Armed — enforcing caps under memory pressure".to_string()
        } else {
            "Idle — caps set but paused until memory gets tight".to_string()
        };
        let warning = groups
            .iter()
            .any(|g| g.fsm_hint == "LowYield" || g.fsm_hint == "Thrashing");
        let first_run = !self.config.groups.iter().any(|g| g.cap_bytes > 0);

        PanelSnapshot {
            system_arm: system_arm.to_string(),
            pause_all,
            used_bytes,
            total_bytes,
            overall_limit_bytes: self.config.overall_limit_bytes,
            status_line,
            warning,
            first_run,
            preflight_notes: Vec::new(),
            samples: self.history.samples().to_vec(),
            ceiling_edits: self.history.ceiling_edits().to_vec(),
            groups: groups.to_vec(),
        }
    }

    pub fn diagnostics_text(&self, ring: &DiagnosticsRing) -> String {
        ring.lines().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap_math::FLOOR_FLAT_BYTES;

    fn temp_config_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramjob_panel_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn empty_config() -> RamjobConfig {
        RamjobConfig::default()
    }

    fn state(path: PathBuf) -> PanelState {
        PanelState {
            config_path: path,
            config: empty_config(),
            history: SysHistory::new(),
        }
    }

    #[test]
    fn set_overall_limit_commits_tick_and_config() {
        let path = temp_config_path("ceiling");
        let mut s = state(path.clone());
        s.set_overall_limit(12 << 30, 5000, false).unwrap();

        assert_eq!(s.config.overall_limit_bytes, 12 << 30);
        let edits = s.history.ceiling_edits();
        assert_eq!(edits.last().unwrap().unix_ms, 5000);
        assert_eq!(edits.last().unwrap().overall_limit_bytes, 12 << 30);

        let reloaded = crate::config::load_config_file(&path).unwrap();
        assert_eq!(reloaded.overall_limit_bytes, 12 << 30);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn set_cap_applies_floor() {
        let path = temp_config_path("floor");
        let mut s = state(path.clone());
        // shift_fine=true so the raw cap snaps to a tiny non-zero value (64 MB units)
        // instead of being treated as "unlimited" by the coarse snap threshold,
        // letting the floor actually engage (see clamp_cap_with_policy/snap_cap_bytes).
        s.set_cap("hog", 1_000_000, true, None).unwrap();

        let g = s.config.groups.iter().find(|g| g.key == "hog").unwrap();
        assert!(g.cap_bytes >= FLOOR_FLAT_BYTES);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn set_cap_updates_existing_group() {
        let path = temp_config_path("update");
        let mut s = state(path.clone());
        s.config.groups.push(GroupConfig {
            key: "hog".into(),
            cap_bytes: 1 << 30,
            always_enforce: true,
            ..Default::default()
        });
        s.set_cap("hog", 4 * 1024 * 1024 * 1024, false, None)
            .unwrap();

        assert_eq!(s.config.groups.len(), 1);
        assert_eq!(s.config.groups[0].cap_bytes, 4 * 1024 * 1024 * 1024);
        assert!(s.config.groups[0].always_enforce);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn set_flags_toggles_always_enforce_and_persists() {
        let path = temp_config_path("flags");
        let mut s = state(path.clone());
        s.set_flags("hog", true).unwrap();

        let g = s.config.groups.iter().find(|g| g.key == "hog").unwrap();
        assert!(g.always_enforce);
        assert_eq!(g.cap_bytes, 0);

        let reloaded = crate::config::load_config_file(&path).unwrap();
        assert!(reloaded.groups.iter().find(|g| g.key == "hog").unwrap().always_enforce);

        s.set_flags("hog", false).unwrap();
        assert!(!s.config.groups.iter().find(|g| g.key == "hog").unwrap().always_enforce);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn pause_toggles_flag_and_persists() {
        let path = temp_config_path("pause");
        let mut s = state(path.clone());
        s.set_pause_all(true).unwrap();
        assert!(s.config.pause_all);
        let reloaded = crate::config::load_config_file(&path).unwrap();
        assert!(reloaded.pause_all);

        s.set_pause_all(false).unwrap();
        assert!(!s.config.pause_all);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn status_line_paused() {
        let path = temp_config_path("status_paused");
        let mut s = state(path.clone());
        s.config.pause_all = true;
        let snap = s.build_snapshot(SystemArm::Disarmed, 0, 0, &[]);
        assert_eq!(snap.status_line, "Paused — enforcement off");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn status_line_armed() {
        let path = temp_config_path("status_armed");
        let s = state(path.clone());
        let snap = s.build_snapshot(SystemArm::Armed, 0, 0, &[]);
        assert_eq!(
            snap.status_line,
            "Armed — enforcing caps under memory pressure"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn status_line_idle_dormant() {
        let path = temp_config_path("status_idle");
        let s = state(path.clone());
        let snap = s.build_snapshot(SystemArm::Disarmed, 0, 0, &[]);
        assert_eq!(
            snap.status_line,
            "Idle — caps set but paused until memory gets tight"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn warning_true_when_group_low_yield_or_thrashing() {
        let path = temp_config_path("warn");
        let s = state(path.clone());
        let groups = vec![PanelGroup {
            key: "hog".into(),
            name: "hog".into(),
            gf_bytes: 100,
            cap_bytes: 100,
            always_enforce: false,
            fsm_hint: "Thrashing".into(),
        }];
        let snap = s.build_snapshot(SystemArm::Armed, 0, 0, &groups);
        assert!(snap.warning);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn build_snapshot_first_run_when_no_caps() {
        let path = temp_config_path("first_run");
        let s = state(path.clone());
        let snap = s.build_snapshot(SystemArm::Disarmed, 0, 0, &[]);
        assert!(snap.first_run);
        assert!(snap.preflight_notes.is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn build_snapshot_not_first_run_when_cap_set() {
        let path = temp_config_path("not_first_run");
        let mut s = state(path.clone());
        s.config.groups.push(GroupConfig {
            key: "hog".into(),
            cap_bytes: 1 << 30,
            ..Default::default()
        });
        let snap = s.build_snapshot(SystemArm::Disarmed, 0, 0, &[]);
        assert!(!snap.first_run);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
