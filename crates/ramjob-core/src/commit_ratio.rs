//! §3.2 GF→commit translation for Job Object backstop (SPEC §3.2, §4.2).

pub const MIN_SAMPLES: u32 = 3;
pub const RATIO_CLAMP_MIN: f64 = 1.0;
pub const RATIO_CLAMP_MAX: f64 = 2.0;
pub const LIMIT_SCALE: f64 = 1.15;
pub const RATCHET_COMMIT_FACTOR: f64 = 1.05;
pub const EMA_ALPHA: f64 = 0.3;

/// Clamp a sampled commit/GF ratio to the §3.2 safety rail.
pub fn clamp_ratio(ratio: f64) -> f64 {
    ratio.clamp(RATIO_CLAMP_MIN, RATIO_CLAMP_MAX)
}

/// `JobMemoryLimit = 1.15 × C × clamp(ratio, 1.0, 2.0)`.
pub fn translate_job_limit(cap_bytes: u64, ratio: f64) -> u64 {
    let scaled = LIMIT_SCALE * cap_bytes as f64 * clamp_ratio(ratio);
    scaled.round() as u64
}

/// Cap-decrease ratchet: never set limit below `current_commit × 1.05`.
pub fn ratchet_limit(target: u64, current_commit: u64) -> u64 {
    let floor = (current_commit as f64 * RATCHET_COMMIT_FACTOR).round() as u64;
    target.max(floor)
}

/// Per-group EMA of `Σ PrivateUsage / GF`, sampled during PRESSURE.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitRatio {
    ema: f64,
    samples: u32,
}

impl Default for CommitRatio {
    fn default() -> Self {
        Self {
            ema: 0.0,
            samples: 0,
        }
    }
}

impl CommitRatio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ready(&self) -> bool {
        self.samples >= MIN_SAMPLES
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Current EMA (meaningful once at least one sample has been taken).
    pub fn ratio(&self) -> f64 {
        self.ema
    }

    /// Record a PRESSURE sample. Returns `false` when `group_gf == 0` (skipped).
    pub fn sample(&mut self, group_commit: u64, group_gf: u64) -> bool {
        if group_gf == 0 {
            return false;
        }
        let instant = group_commit as f64 / group_gf as f64;
        if self.samples == 0 {
            self.ema = instant;
        } else {
            self.ema = EMA_ALPHA * instant + (1.0 - EMA_ALPHA) * self.ema;
        }
        self.samples += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gb(n: u64) -> u64 {
        n * 1024 * 1024 * 1024
    }

    #[test]
    fn ready_false_until_three_samples() {
        let mut cr = CommitRatio::new();
        assert!(!cr.ready());
        assert!(cr.sample(gb(2), gb(1)));
        assert!(!cr.ready());
        assert!(cr.sample(gb(2), gb(1)));
        assert!(!cr.ready());
        assert!(cr.sample(gb(2), gb(1)));
        assert!(cr.ready());
    }

    #[test]
    fn zero_gf_skips_sample() {
        let mut cr = CommitRatio::new();
        assert!(!cr.sample(100, 0));
        assert_eq!(cr.samples(), 0);
        assert!(!cr.ready());
    }

    #[test]
    fn ema_first_sample_is_instant_ratio() {
        let mut cr = CommitRatio::new();
        cr.sample(gb(3), gb(2));
        assert!((cr.ratio() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn ema_smooths_subsequent_samples() {
        let mut cr = CommitRatio::new();
        cr.sample(2_000_000_000, 1_000_000_000); // 2.0
        cr.sample(1_000_000_000, 1_000_000_000); // 1.0 → 0.3*1 + 0.7*2 = 1.7
        assert!((cr.ratio() - 1.7).abs() < 1e-9);
        cr.sample(1_000_000_000, 1_000_000_000); // 1.0 → 0.3*1 + 0.7*1.7 = 1.49
        assert!((cr.ratio() - 1.49).abs() < 1e-9);
        assert!(cr.ready());
    }

    #[test]
    fn clamp_ratio_bounds() {
        assert_eq!(clamp_ratio(0.5), 1.0);
        assert_eq!(clamp_ratio(1.0), 1.0);
        assert_eq!(clamp_ratio(1.5), 1.5);
        assert_eq!(clamp_ratio(2.0), 2.0);
        assert_eq!(clamp_ratio(3.0), 2.0);
    }

    #[test]
    fn translate_job_limit_formula() {
        let cap = gb(4);
        assert_eq!(translate_job_limit(cap, 1.0), (1.15 * cap as f64).round() as u64);
        assert_eq!(translate_job_limit(cap, 0.5), translate_job_limit(cap, 1.0));
        assert_eq!(
            translate_job_limit(cap, 2.0),
            (1.15 * 2.0 * cap as f64).round() as u64
        );
        assert_eq!(translate_job_limit(cap, 3.0), translate_job_limit(cap, 2.0));
    }

    #[test]
    fn ratchet_never_below_commit_times_105_percent() {
        assert_eq!(ratchet_limit(1_000, 900), 1_000);
        assert_eq!(ratchet_limit(1_000, 1_000), 1_050);
        assert_eq!(ratchet_limit(500, 1_000), 1_050);
    }
}
