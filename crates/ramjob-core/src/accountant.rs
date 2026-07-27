//! Group Footprint accounting (SPEC §3.1).
//!
//! GF = Σ private working set of members + unique shared (counted once).
//! Unique shared via QueryWorkingSetEx is stubbed to 0 for M0.

use crate::grouper::AppGroup;

/// Floor for displaying a group in the default UI (SPEC §5.2 / §6).
pub const VISIBLE_GF_FLOOR_BYTES: u64 = 50 * 1024 * 1024;

/// Unique shared working set attributed to the group, counted once.
///
/// TODO(M1+): QueryWorkingSetEx per member, dedupe by page frame number, cache
/// for ~5 minutes (or refresh on panel open). M0 returns 0 so hot paths and
/// unit tests never touch QueryWorkingSetEx.
pub fn unique_shared_ws(_group: &AppGroup) -> u64 {
    0
}

/// Group Footprint in bytes: private WS sum + [`unique_shared_ws`].
pub fn group_footprint(group: &AppGroup) -> u64 {
    let private: u64 = group
        .members
        .iter()
        .map(|m| m.private_working_set_bytes)
        .sum();
    private.saturating_add(unique_shared_ws(group))
}

/// True when GF meets the default display floor (≥ 50 MB).
pub fn meets_gf_floor(gf: u64) -> bool {
    gf >= VISIBLE_GF_FLOOR_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grouper::GroupMember;

    fn group(members: Vec<(u32, u64)>) -> AppGroup {
        AppGroup {
            group_key: "test-app".into(),
            members: members
                .into_iter()
                .map(|(pid, private_working_set_bytes)| GroupMember {
                    pid,
                    create_time: 1,
                    private_working_set_bytes,
                })
                .collect(),
        }
    }

    #[test]
    fn private_ws_sum_across_synthetic_members() {
        let g = group(vec![(1, 10), (2, 20), (3, 30)]);
        assert_eq!(group_footprint(&g), 60);
    }

    #[test]
    fn empty_group_footprint_is_zero() {
        let g = group(vec![]);
        assert_eq!(group_footprint(&g), 0);
    }

    #[test]
    fn unique_shared_ws_defaults_to_zero_without_panic() {
        let g = group(vec![(1, 1_000_000)]);
        assert_eq!(unique_shared_ws(&g), 0);
        assert_eq!(group_footprint(&g), 1_000_000);
    }

    #[test]
    fn meets_gf_floor_filters_below_50_mb() {
        let just_under = VISIBLE_GF_FLOOR_BYTES - 1;
        let at_floor = VISIBLE_GF_FLOOR_BYTES;
        assert!(!meets_gf_floor(just_under));
        assert!(meets_gf_floor(at_floor));
        assert!(meets_gf_floor(VISIBLE_GF_FLOOR_BYTES + 1));
        assert!(!meets_gf_floor(0));
    }

    #[test]
    fn group_footprint_includes_unique_shared_placeholder() {
        // M0: unique_shared is 0, so GF equals private sum only.
        let g = group(vec![(10, 40 * 1024 * 1024), (11, 20 * 1024 * 1024)]);
        let gf = group_footprint(&g);
        assert_eq!(gf, 60 * 1024 * 1024);
        assert!(meets_gf_floor(gf));
    }
}
