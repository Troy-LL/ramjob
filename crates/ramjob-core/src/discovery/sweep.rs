//! Sweep backend: diff NtQSI enumerate snapshots into spawn/exit events.

use std::collections::HashSet;

use crate::scanner::ProcessRecord;

use super::{diff_identities, DiscoveryEvent, DiscoverySource, ProcessIdentity};

/// Last-resort discovery: diff `(pid, create_time)` snapshots (no own enumerate).
pub struct SweepDiscovery {
    seen: HashSet<ProcessIdentity>,
}

impl SweepDiscovery {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Advance from an explicit identity set (unit tests / inject).
    pub fn poll_from_identities(
        &mut self,
        current: HashSet<ProcessIdentity>,
    ) -> Vec<DiscoveryEvent> {
        let events = diff_identities(&self.seen, &current);
        self.seen = current;
        events
    }
}

impl Default for SweepDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoverySource for SweepDiscovery {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent> {
        Vec::new()
    }

    fn poll_events_from_enumerate(&mut self, procs: &[ProcessRecord]) -> Vec<DiscoveryEvent> {
        let current: HashSet<ProcessIdentity> = procs
            .iter()
            .filter(|p| p.pid != 0)
            .map(|p| (p.pid, p.create_time))
            .collect();
        self.poll_from_identities(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[(u32, i64)]) -> HashSet<ProcessIdentity> {
        ids.iter().copied().collect()
    }

    #[test]
    fn sweep_first_poll_from_identities_all_spawn() {
        let mut sweep = SweepDiscovery::new();
        let events = sweep.poll_from_identities(set(&[(10, 1), (20, 2)]));
        assert_eq!(
            events,
            vec![
                DiscoveryEvent::Spawn { pid: 10, create_time: 1 },
                DiscoveryEvent::Spawn { pid: 20, create_time: 2 },
            ]
        );
    }

    #[test]
    fn sweep_second_poll_diffs_spawn_exit() {
        let mut sweep = SweepDiscovery::new();
        sweep.poll_from_identities(set(&[(1, 100), (2, 200)]));
        let events = sweep.poll_from_identities(set(&[(1, 100), (3, 300)]));
        assert_eq!(
            events,
            vec![
                DiscoveryEvent::Exit { pid: 2, create_time: 200 },
                DiscoveryEvent::Spawn { pid: 3, create_time: 300 },
            ]
        );
    }

    #[test]
    fn sweep_from_enumerate_does_not_panic() {
        let mut sweep = SweepDiscovery::new();
        let _ = sweep.poll_events_from_enumerate(&[]);
    }
}
