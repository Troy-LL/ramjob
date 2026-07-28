//! Cap math: snap to discrete limits, apply safety floor.

pub const FLOOR_FLAT_BYTES: u64 = 300 * 1024 * 1024;
pub const CAP_SNAP_BYTES: &[u64] = &[
    512 * 1024 * 1024,
    1024 * 1024 * 1024,
    1536 * 1024 * 1024,
    2 * 1024 * 1024 * 1024,
    3 * 1024 * 1024 * 1024,
    4 * 1024 * 1024 * 1024,
    6 * 1024 * 1024 * 1024,
    8 * 1024 * 1024 * 1024,
    12 * 1024 * 1024 * 1024,
    16 * 1024 * 1024 * 1024,
];

/// Snap cap to nearest entry in CAP_SNAP_BYTES, or fine-grained 64 MB increments.
/// `0` means unlimited.
pub fn snap_cap_bytes(raw: u64, shift_fine: bool) -> u64 {
    if raw == 0 {
        return 0;
    }

    if shift_fine {
        // Round to nearest 64 MB (min 64 MB)
        let unit = 64 * 1024 * 1024;
        std::cmp::max(unit, ((raw + unit / 2) / unit) * unit)
    } else {
        // Find nearest entry in CAP_SNAP_BYTES, treating values below half of first snap as 0
        let threshold = CAP_SNAP_BYTES[0] / 2;
        if raw < threshold {
            return 0;
        }

        // Find closest entry
        CAP_SNAP_BYTES
            .iter()
            .copied()
            .min_by_key(|&snap| {
                let diff = if raw > snap {
                    raw - snap
                } else {
                    snap - raw
                };
                diff
            })
            .unwrap_or(0)
    }
}

/// Apply safety floor: if cap is 0, return 0; else max(cap, flat floor, quarter median).
pub fn apply_cap_floor(cap_bytes: u64, median_gf_bytes: Option<u64>) -> u64 {
    if cap_bytes == 0 {
        return 0;
    }

    let floor_from_median = median_gf_bytes
        .map(|m| (0.25 * m as f64) as u64)
        .unwrap_or(0);

    cap_bytes.max(FLOOR_FLAT_BYTES).max(floor_from_median)
}

/// Clamp cap: snap first, then apply floor.
pub fn clamp_cap_with_policy(raw: u64, shift_fine: bool, median_gf_bytes: Option<u64>) -> u64 {
    let snapped = snap_cap_bytes(raw, shift_fine);
    apply_cap_floor(snapped, median_gf_bytes)
}

/// Snap the system-wide stop-loss ceiling (SPEC §8.4's overall ceiling, a
/// different concept from the per-app cap ladder above): round to the
/// nearest 1 GB, or nearest 64 MB with `shift_fine`, with no upper bound —
/// the per-app ladder tops out at 16 GB deliberately, but a machine with more
/// RAM than that must still be able to set its ceiling above it.
pub fn snap_ceiling_bytes(raw: u64, shift_fine: bool) -> u64 {
    if raw == 0 {
        return 0;
    }
    let unit: u64 = if shift_fine {
        64 * 1024 * 1024
    } else {
        1024 * 1024 * 1024
    };
    std::cmp::max(unit, ((raw + unit / 2) / unit) * unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_nearest_without_shift() {
        let near_3g = 3 * 1024 * 1024 * 1024 - 10_000_000;
        assert_eq!(snap_cap_bytes(near_3g, false), 3 * 1024 * 1024 * 1024);
    }

    #[test]
    fn zero_stays_unlimited() {
        assert_eq!(snap_cap_bytes(0, false), 0);
        assert_eq!(apply_cap_floor(0, Some(8 * 1024 * 1024 * 1024)), 0);
    }

    #[test]
    fn floor_uses_flat_300mb_without_median() {
        let c = apply_cap_floor(100 * 1024 * 1024, None);
        assert_eq!(c, FLOOR_FLAT_BYTES);
    }

    #[test]
    fn floor_uses_quarter_median_when_higher() {
        let median = 8 * 1024 * 1024 * 1024u64;
        let floor = (0.25 * median as f64) as u64;
        assert_eq!(apply_cap_floor(100, Some(median)), floor);
    }

    #[test]
    fn ceiling_snap_is_unbounded_above_16gb() {
        let raw = 32 * 1024 * 1024 * 1024;
        assert_eq!(snap_ceiling_bytes(raw, false), raw);
        assert_eq!(snap_ceiling_bytes(0, false), 0);
        // Fine control rounds to 64 MB, still unbounded.
        let raw_fine = 20 * 1024 * 1024 * 1024 + 10 * 1024 * 1024;
        assert_eq!(
            snap_ceiling_bytes(raw_fine, true),
            20 * 1024 * 1024 * 1024
        );
    }
}
