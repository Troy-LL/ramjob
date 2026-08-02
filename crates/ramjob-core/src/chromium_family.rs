//! Chromium-family group detection (SPEC §4.2 post-ship OPEN).

const EXE_STEMS: &[&str] = &[
    "chrome", "chromium", "msedge", "brave", "opera", "vivaldi",
];

const PATH_SEGMENTS: &[&str] = &[
    "chrome",
    "chromium",
    "msedge",
    "edge",
    "brave",
    "bravesoftware",
    "opera",
    "vivaldi",
    "google",
];

/// Whether `group_key` names a Chromium-family install (path or `image:` stem).
pub fn is_chromium_family(group_key: &str) -> bool {
    let key = group_key.to_ascii_lowercase();
    if let Some(stem) = key.strip_prefix("image:") {
        return EXE_STEMS.iter().any(|&name| stem == name);
    }
    key.split(['\\', '/']).any(segment_matches)
}

fn segment_matches(segment: &str) -> bool {
    let seg = segment.to_ascii_lowercase();
    PATH_SEGMENTS.iter().any(|&name| seg == name)
        || seg.starts_with("opera")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_stems_match() {
        for stem in ["chrome", "Chrome", "msedge", "brave", "opera", "vivaldi", "chromium"] {
            assert!(
                is_chromium_family(&format!("image:{stem}")),
                "expected image:{stem}"
            );
        }
        assert!(!is_chromium_family("image:firefox"));
        assert!(!is_chromium_family("image:slack"));
    }

    #[test]
    fn install_path_keys_match() {
        assert!(is_chromium_family(
            r"c:\program files\google\chrome\application"
        ));
        assert!(is_chromium_family(r"c:\program files\bravesoftware"));
        assert!(is_chromium_family(
            r"c:\program files (x86)\microsoft\edge\application"
        ));
        assert!(is_chromium_family(r"c:\users\me\appdata\local\vivaldi"));
        assert!(is_chromium_family(r"c:\program files\opera gx"));
        assert!(!is_chromium_family(r"c:\program files\mozilla firefox"));
        assert!(!is_chromium_family(r"c:\program files\discord"));
    }
}
