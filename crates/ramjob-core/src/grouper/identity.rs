//! Ordered group-key resolution (SPEC §5.3).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::scanner::ProcessRecord;

use super::pathutil::{normalize_path_str, normalize_pathbuf, paths_equal};
use super::GroupContext;

const RUNTIME_HOSTS: &[&str] = &[
    "msedgewebview2",
    "java",
    "javaw",
    "python",
    "pythonw",
    "node",
    "dotnet",
    "wscript",
];

fn image_stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .unwrap_or(&lower)
        .to_string()
}

fn is_runtime_host(image_name: &str) -> bool {
    let stem = image_stem(image_name);
    RUNTIME_HOSTS.iter().any(|h| *h == stem)
}

pub(super) fn resolve_group_key(
    proc: &ProcessRecord,
    by_pid: &HashMap<u32, &ProcessRecord>,
    ctx: &GroupContext,
) -> Option<String> {
    if is_runtime_host(&proc.image_name) {
        return resolve_runtime_host_key(proc, by_pid, ctx);
    }
    resolve_non_runtime_key(proc, ctx)
}

fn resolve_runtime_host_key(
    proc: &ProcessRecord,
    by_pid: &HashMap<u32, &ProcessRecord>,
    ctx: &GroupContext,
) -> Option<String> {
    let mut current = proc.ppid;
    let mut seen = HashSet::new();
    seen.insert(proc.pid);
    while current != 0 && seen.insert(current) {
        let Some(ancestor) = by_pid.get(&current) else {
            break;
        };
        if is_runtime_host(&ancestor.image_name) {
            current = ancestor.ppid;
            continue;
        }
        return resolve_non_runtime_key(ancestor, ctx);
    }
    None
}

fn resolve_non_runtime_key(proc: &ProcessRecord, ctx: &GroupContext) -> Option<String> {
    if let Some(path) = proc.image_path.as_ref() {
        if let Some(key) = install_root_key(path, &ctx.known_install_roots) {
            return Some(key);
        }
        // Signer / tree tiers not implemented yet — fall through to image stem.
    }
    if proc.image_name.is_empty() {
        return None;
    }
    Some(format!("image:{}", image_stem(&proc.image_name)))
}

fn install_root_key(image_path: &Path, known_roots: &[PathBuf]) -> Option<String> {
    let mut dir = image_path.parent()?.to_path_buf();
    // Strip trailing version-shaped segments (Discord app-1.0.x, Electron "current", …).
    while dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(is_version_segment)
    {
        dir = dir.parent()?.to_path_buf();
    }

    let roots_norm: Vec<PathBuf> = known_roots.iter().map(|r| normalize_pathbuf(r)).collect();
    let mut cursor = dir;
    loop {
        let parent = match cursor.parent() {
            Some(p) if p.as_os_str().len() > 0 => p.to_path_buf(),
            _ => break,
        };
        let parent_norm = normalize_pathbuf(&parent);
        if roots_norm.iter().any(|r| paths_equal(r, &parent_norm)) {
            // Launcher-exec fallback stubbed for M0 (no filesystem probe).
            return Some(normalize_path_str(&cursor));
        }
        cursor = parent;
    }
    None
}

fn is_version_segment(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name == "current" {
        return true;
    }
    // ^(app-)?v?\d+(\.\d+)+$
    let rest = name.strip_prefix("app-").unwrap_or(&name);
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    is_dotted_version(rest)
}

fn is_dotted_version(s: &str) -> bool {
    let mut parts = s.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    if first.is_empty() || !first.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let mut saw_dot_part = false;
    for part in parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        saw_dot_part = true;
    }
    saw_dot_part
}
