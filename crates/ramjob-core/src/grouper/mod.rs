//! Install-root grouping + identity rules (SPEC §5.2–5.3).

mod filter;
mod identity;
mod pathutil;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::scanner::ProcessRecord;

use filter::{eligible_for_grouping, self_tree_pids};
use identity::resolve_group_key;

/// One process inside an [`AppGroup`], carrying private WS for footprint math.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMember {
    pub pid: u32,
    /// FILETIME ticks; pairs with `pid` for trim ΔGF intersection.
    pub create_time: i64,
    pub private_working_set_bytes: u64,
    /// Commit charge (`Σ PrivateUsage` per member); from SPI `PagefileUsage`.
    pub private_usage_bytes: u64,
}

/// One visible application group after install-root / fallback resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGroup {
    /// Stable identity key: normalized, version-stripped install root (or fallback).
    pub group_key: String,
    pub members: Vec<GroupMember>,
}

impl AppGroup {
    /// Member PIDs in group order (sorted ascending at build time).
    pub fn member_pids(&self) -> Vec<u32> {
        self.members.iter().map(|m| m.pid).collect()
    }
}

/// Roots and self-PID used when grouping. Tests inject synthetic roots.
#[derive(Debug, Clone)]
pub struct GroupContext {
    pub known_install_roots: Vec<PathBuf>,
    pub self_pid: u32,
    pub windir: PathBuf,
    pub program_files: PathBuf,
}

impl GroupContext {
    /// Build context from process environment (`ProgramFiles`, `LOCALAPPDATA`, …).
    pub fn from_env(self_pid: u32) -> Self {
        let mut roots = Vec::new();
        for key in [
            "ProgramFiles",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
            "APPDATA",
        ] {
            if let Ok(v) = std::env::var(key) {
                let p = PathBuf::from(v);
                if key == "LOCALAPPDATA" {
                    roots.push(p.join("Programs"));
                }
                roots.push(p);
            }
        }
        let windir = std::env::var("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Windows"));
        let program_files = std::env::var("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Program Files"));
        Self {
            known_install_roots: roots,
            self_pid,
            windir,
            program_files,
        }
    }
}

/// Group processes using env-derived install roots and the current process as self.
pub fn group_processes(procs: &[ProcessRecord]) -> Vec<AppGroup> {
    let ctx = GroupContext::from_env(std::process::id());
    group_processes_with_context(procs, &ctx)
}

/// Group with an injectable context (synthetic roots / self PID for tests).
pub fn group_processes_with_context(procs: &[ProcessRecord], ctx: &GroupContext) -> Vec<AppGroup> {
    let by_pid: HashMap<u32, &ProcessRecord> = procs.iter().map(|p| (p.pid, p)).collect();
    let excluded = self_tree_pids(ctx.self_pid, &by_pid);

    let mut key_to_members: HashMap<String, Vec<GroupMember>> = HashMap::new();

    for proc in procs {
        if !eligible_for_grouping(proc, ctx, &excluded) {
            continue;
        }
        let Some(key) = resolve_group_key(proc, &by_pid, ctx) else {
            continue;
        };
        key_to_members.entry(key).or_default().push(GroupMember {
            pid: proc.pid,
            create_time: proc.create_time,
            private_working_set_bytes: proc.private_working_set_bytes,
            private_usage_bytes: proc.private_usage_bytes,
        });
    }

    let mut groups: Vec<AppGroup> = key_to_members
        .into_iter()
        .map(|(group_key, mut members)| {
            members.sort_unstable_by_key(|m| m.pid);
            AppGroup {
                group_key,
                members,
            }
        })
        .collect();
    groups.sort_by(|a, b| a.group_key.cmp(&b.group_key));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(
        pid: u32,
        ppid: u32,
        session_id: u32,
        image_name: &str,
        path: Option<&str>,
    ) -> ProcessRecord {
        ProcessRecord {
            pid,
            ppid,
            session_id,
            image_name: image_name.to_string(),
            private_working_set_bytes: 0,
            private_usage_bytes: 0,
            working_set_bytes: 0,
            create_time: 1,
            image_path: path.map(PathBuf::from),
        }
    }

    fn test_ctx(self_pid: u32) -> GroupContext {
        GroupContext {
            known_install_roots: vec![
                PathBuf::from(r"C:\Program Files"),
                PathBuf::from(r"C:\Users\test\AppData\Local"),
                PathBuf::from(r"C:\Users\test\AppData\Local\Programs"),
                PathBuf::from(r"C:\Users\test\AppData\Roaming"),
            ],
            self_pid,
            windir: PathBuf::from(r"C:\Windows"),
            program_files: PathBuf::from(r"C:\Program Files"),
        }
    }

    #[test]
    fn discord_app_version_segment_stripped() {
        let procs = vec![
            rec(
                10,
                1,
                1,
                "Discord.exe",
                Some(r"C:\Users\test\AppData\Local\Discord\app-1.0.9036\Discord.exe"),
            ),
            rec(
                11,
                10,
                1,
                "Discord.exe",
                Some(r"C:\Users\test\AppData\Local\Discord\app-1.0.9036\Discord.exe"),
            ),
        ];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].group_key,
            r"c:\users\test\appdata\local\discord"
        );
        assert_eq!(groups[0].member_pids(), vec![10, 11]);
    }

    #[test]
    fn two_apps_under_different_roots_never_merge() {
        let procs = vec![
            rec(
                20,
                1,
                1,
                "brave.exe",
                Some(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe"),
            ),
            rec(
                21,
                20,
                1,
                "brave.exe",
                Some(
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                ),
            ),
            rec(
                30,
                1,
                1,
                "Discord.exe",
                Some(r"C:\Users\test\AppData\Local\Discord\app-1.0.9000\Discord.exe"),
            ),
        ];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 2);
        let keys: Vec<_> = groups.iter().map(|g| g.group_key.as_str()).collect();
        assert!(keys.contains(&r"c:\program files\bravesoftware"));
        assert!(keys.contains(&r"c:\users\test\appdata\local\discord"));
        assert!(!keys.iter().any(|k| k.contains("brave") && k.contains("discord")));
    }

    #[test]
    fn runtime_host_under_brave_joins_brave_when_ppid_set() {
        let brave_path =
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe";
        let webview_path =
            r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application\msedgewebview2.exe";
        let procs = vec![
            rec(40, 1, 1, "brave.exe", Some(brave_path)),
            rec(41, 40, 1, "msedgewebview2.exe", Some(webview_path)),
        ];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_key, r"c:\program files\bravesoftware");
        assert_eq!(groups[0].member_pids(), vec![40, 41]);
    }

    #[test]
    fn runtime_host_without_ancestor_stays_ungrouped() {
        let procs = vec![rec(
            50,
            1,
            1,
            "node.exe",
            Some(r"C:\Program Files\nodejs\node.exe"),
        )];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert!(groups.is_empty());
    }

    #[test]
    fn self_pid_and_descendants_excluded() {
        let procs = vec![
            rec(
                100,
                1,
                1,
                "ramjob.exe",
                Some(r"C:\Users\test\AppData\Local\Programs\RamJob\ramjob.exe"),
            ),
            rec(
                101,
                100,
                1,
                "msedgewebview2.exe",
                Some(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application\msedgewebview2.exe"),
            ),
            rec(
                200,
                1,
                1,
                "Discord.exe",
                Some(r"C:\Users\test\AppData\Local\Discord\app-1.0.1\Discord.exe"),
            ),
        ];
        let groups = group_processes_with_context(&procs, &test_ctx(100));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_pids(), vec![200]);
    }

    #[test]
    fn session_zero_and_windir_and_critical_filtered() {
        let procs = vec![
            rec(
                60,
                1,
                0,
                "svchost.exe",
                Some(r"C:\Windows\System32\svchost.exe"),
            ),
            rec(
                61,
                1,
                1,
                "notepad.exe",
                Some(r"C:\Windows\System32\notepad.exe"),
            ),
            rec(
                62,
                1,
                1,
                "explorer.exe",
                Some(r"C:\Windows\explorer.exe"),
            ),
            rec(
                63,
                1,
                1,
                "Discord.exe",
                Some(r"C:\Users\test\AppData\Local\Discord\app-1.0.1\Discord.exe"),
            ),
        ];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].member_pids(), vec![63]);
    }

    #[test]
    fn steam_game_excluded_from_displayed_groups() {
        let procs = vec![rec(
            70,
            1,
            1,
            "game.exe",
            Some(r"D:\Steam\steamapps\common\CoolGame\game.exe"),
        )];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert!(groups.is_empty());
    }

    #[test]
    fn current_version_segment_stripped() {
        let procs = vec![rec(
            80,
            1,
            1,
            "app.exe",
            Some(r"C:\Users\test\AppData\Local\Programs\SomeApp\current\app.exe"),
        )];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].group_key,
            r"c:\users\test\appdata\local\programs\someapp"
        );
    }

    #[test]
    fn install_root_miss_falls_through_to_image_stem() {
        let procs = vec![rec(
            90,
            1,
            1,
            "WeirdApp.exe",
            Some(r"D:\Portable\WeirdApp\WeirdApp.exe"),
        )];
        let groups = group_processes_with_context(&procs, &test_ctx(9999));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_key, "image:weirdapp");
    }
}
