//! Path normalization helpers for filter + identity.

use std::path::{Path, PathBuf};

pub(super) fn path_under_prefix(path: &Path, prefix: &Path) -> bool {
    let p = normalize_pathbuf(path);
    let pre = normalize_pathbuf(prefix);
    p.starts_with(&pre)
}

pub(super) fn paths_equal(a: &Path, b: &Path) -> bool {
    normalize_path_str(a) == normalize_path_str(b)
}

pub(super) fn normalize_pathbuf(path: &Path) -> PathBuf {
    PathBuf::from(normalize_path_str(path))
}

pub(super) fn normalize_path_str(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    let lower = s.to_ascii_lowercase();
    let mut chars = lower.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' || c == '\\' {
            while matches!(chars.peek(), Some('/' | '\\')) {
                chars.next();
            }
            if !out.is_empty() && !out.ends_with('\\') {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    while out.ends_with('\\') {
        out.pop();
    }
    out
}
