//! System memory history ring + ceiling edit ticks (M3 tray UI).

use std::collections::VecDeque;

pub const SYS_HISTORY_CAP: usize = 600;
const CEILING_EDITS_CAP: usize = 128;

/// Single sample of system RAM usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysSample {
    pub unix_ms: u64,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

/// Single ceiling edit event (limit change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeilingEdit {
    pub unix_ms: u64,
    pub overall_limit_bytes: u64,
}

/// Ring buffer of system memory samples + ceiling edit history.
pub struct SysHistory {
    samples: VecDeque<SysSample>,
    ceiling_edits: Vec<CeilingEdit>,
}

impl SysHistory {
    /// Create a new empty history.
    pub fn new() -> Self {
        SysHistory {
            samples: VecDeque::with_capacity(SYS_HISTORY_CAP),
            ceiling_edits: Vec::with_capacity(CEILING_EDITS_CAP),
        }
    }

    /// Push a sample to the ring. Drops the oldest sample if at capacity.
    /// Maintains contiguous internal layout for efficient slice access.
    pub fn push_sample(&mut self, s: SysSample) {
        if self.samples.len() >= SYS_HISTORY_CAP {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
        self.samples.make_contiguous();
    }

    /// Record a ceiling edit (limit change). Does not push a sample.
    /// Drops the oldest edit if edits are at capacity.
    pub fn commit_ceiling(&mut self, edit: CeilingEdit) {
        if self.ceiling_edits.len() >= CEILING_EDITS_CAP {
            self.ceiling_edits.remove(0);
        }
        self.ceiling_edits.push(edit);
    }

    /// View all samples in the ring (oldest to newest).
    pub fn samples(&self) -> &[SysSample] {
        self.samples.as_slices().0
    }

    /// View all ceiling edits (oldest to newest).
    pub fn ceiling_edits(&self) -> &[CeilingEdit] {
        &self.ceiling_edits
    }
}

impl Default for SysHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_samples() {
        let mut h = SysHistory::new();
        for i in 0..(SYS_HISTORY_CAP + 5) {
            h.push_sample(SysSample {
                unix_ms: i as u64,
                used_bytes: i as u64,
                total_bytes: 32 << 30,
            });
        }
        assert_eq!(h.samples().len(), SYS_HISTORY_CAP);
        assert_eq!(h.samples()[0].unix_ms, 5);
    }

    #[test]
    fn ceiling_commit_records_tick_without_sample() {
        let mut h = SysHistory::new();
        h.commit_ceiling(CeilingEdit {
            unix_ms: 1000,
            overall_limit_bytes: 12 << 30,
        });
        assert_eq!(h.samples().len(), 0);
        assert_eq!(h.ceiling_edits().len(), 1);
    }
}
