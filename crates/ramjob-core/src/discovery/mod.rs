//! Process discovery: spawn/exit deltas between polls (M5 §6.1 sweep backend).

mod etw;
mod sweep;

use std::collections::HashSet;

pub use etw::{etw_degrade_diagnostic, EtwOpenError, EtwProcessSource};
pub use sweep::SweepDiscovery;

/// `(pid, create_time)` process identity (same key as trim ΔGF intersection).
pub type ProcessIdentity = (u32, i64);

/// Spawn or exit notification from a [`DiscoverySource`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Spawn { pid: u32, create_time: i64 },
    Exit { pid: u32, create_time: i64 },
}

/// Event-driven process discovery (ETW / WMI / sweep backends).
pub trait DiscoverySource {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent>;
}

/// Diff two identity snapshots into spawn/exit events (exits before spawns, sorted).
pub fn diff_identities(
    previous: &HashSet<ProcessIdentity>,
    current: &HashSet<ProcessIdentity>,
) -> Vec<DiscoveryEvent> {
    let mut events = Vec::new();

    let mut exits: Vec<_> = previous.difference(current).copied().collect();
    exits.sort_unstable();
    for (pid, create_time) in exits {
        events.push(DiscoveryEvent::Exit { pid, create_time });
    }

    let mut spawns: Vec<_> = current.difference(previous).copied().collect();
    spawns.sort_unstable();
    for (pid, create_time) in spawns {
        events.push(DiscoveryEvent::Spawn { pid, create_time });
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[(u32, i64)]) -> HashSet<ProcessIdentity> {
        ids.iter().copied().collect()
    }

    #[test]
    fn diff_empty_snapshots_yield_no_events() {
        assert!(diff_identities(&set(&[]), &set(&[])).is_empty());
    }

    #[test]
    fn diff_first_snapshot_all_spawn() {
        let events = diff_identities(&set(&[]), &set(&[(1, 10), (2, 20)]));
        assert_eq!(
            events,
            vec![
                DiscoveryEvent::Spawn { pid: 1, create_time: 10 },
                DiscoveryEvent::Spawn { pid: 2, create_time: 20 },
            ]
        );
    }

    #[test]
    fn diff_spawn_and_exit_between_polls() {
        let before = set(&[(1, 10), (2, 20)]);
        let after = set(&[(1, 10), (3, 30)]);
        let events = diff_identities(&before, &after);
        assert_eq!(
            events,
            vec![
                DiscoveryEvent::Exit { pid: 2, create_time: 20 },
                DiscoveryEvent::Spawn { pid: 3, create_time: 30 },
            ]
        );
    }

    #[test]
    fn diff_pid_reuse_is_exit_plus_spawn_not_silent() {
        let before = set(&[(5, 100)]);
        let after = set(&[(5, 200)]);
        let events = diff_identities(&before, &after);
        assert_eq!(
            events,
            vec![
                DiscoveryEvent::Exit { pid: 5, create_time: 100 },
                DiscoveryEvent::Spawn { pid: 5, create_time: 200 },
            ]
        );
    }

    #[test]
    fn diff_identical_snapshots_yield_no_events() {
        let snap = set(&[(1, 1), (2, 2)]);
        assert!(diff_identities(&snap, &snap).is_empty());
    }
}
