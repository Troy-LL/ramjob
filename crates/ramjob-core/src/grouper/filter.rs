//! Display / eligibility policy (SPEC §5.2).

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use crate::scanner::ProcessRecord;

use super::pathutil::{normalize_path_str, path_under_prefix};
use super::GroupContext;

const CRITICAL_DENYLIST: &[&str] = &[
    "csrss",
    "wininit",
    "lsass",
    "winlogon",
    "services",
    "smss",
    "dwm",
    "explorer",
    "msmpeng",
    "searchhost",
    "ctfmon",
];

fn image_stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .unwrap_or(&lower)
        .to_string()
}

fn is_critical_denied(image_name: &str) -> bool {
    let stem = image_stem(image_name);
    CRITICAL_DENYLIST.iter().any(|h| *h == stem)
}

/// True when the path looks like a known game-library install root (Steam/Epic/GOG).
fn is_game_root(path: &Path) -> bool {
    let norm = normalize_path_str(path);
    // M0: Steam only; Epic/GOG markers land here later.
    norm.contains(r"steamapps\common")
}

/// True when a process may enter install-root / fallback grouping.
pub(super) fn eligible_for_grouping(
    proc: &ProcessRecord,
    ctx: &GroupContext,
    excluded: &HashSet<u32>,
) -> bool {
    if excluded.contains(&proc.pid) {
        return false;
    }
    if proc.session_id == 0 {
        return false;
    }
    if is_critical_denied(&proc.image_name) {
        return false;
    }
    let Some(path) = proc.image_path.as_ref() else {
        // No path: still allow last-resort image-name grouping unless denied above.
        return true;
    };
    if path_under_prefix(path, &ctx.windir) {
        return false;
    }
    let windows_apps = ctx.program_files.join("WindowsApps");
    if path_under_prefix(path, &windows_apps) {
        let rel = path.strip_prefix(&windows_apps).ok();
        if let Some(rel) = rel {
            if let Some(Component::Normal(first)) = rel.components().next() {
                let s = first.to_string_lossy().to_ascii_lowercase();
                if s.starts_with("microsoft.") {
                    return false;
                }
            }
        }
    }
    if is_game_root(path) {
        return false;
    }
    true
}

pub(super) fn self_tree_pids(
    self_pid: u32,
    by_pid: &HashMap<u32, &ProcessRecord>,
) -> HashSet<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for p in by_pid.values() {
        children.entry(p.ppid).or_default().push(p.pid);
    }
    let mut out = HashSet::new();
    let mut stack = vec![self_pid];
    while let Some(pid) = stack.pop() {
        if !out.insert(pid) {
            continue;
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    out
}
