//! Process discovery: spawn/exit deltas between polls (M5 §6.1 sweep backend).

mod etw;
mod sweep;
mod wmi;

use std::collections::HashSet;

pub use etw::{etw_degrade_diagnostic, EtwOpenError, EtwProcessSource};
pub use sweep::SweepDiscovery;
pub use wmi::{wmi_degrade_diagnostic, WmiOpenError, WmiProcessSource};

/// `(pid, create_time)` process identity (same key as trim ΔGF intersection).
pub type ProcessIdentity = (u32, i64);

/// Spawn or exit notification from a [`DiscoverySource`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Spawn { pid: u32, create_time: i64 },
    Exit { pid: u32, create_time: i64 },
}

/// Active discovery backend selected by [`select_discovery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMode {
    Etw,
    Wmi,
    Sweep,
}

/// Event-driven process discovery (ETW / WMI / sweep backends).
pub trait DiscoverySource: Send {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent>;
}

/// Select the best available discovery backend: ETW → WMI → sweep.
///
/// Returns the source, which mode was selected, and an optional one-shot degrade
/// diagnostic when ETW or WMI was unavailable.
pub fn select_discovery() -> (Box<dyn DiscoverySource>, DiscoveryMode, Option<String>) {
    select_discovery_with(EtwProcessSource::try_new, WmiProcessSource::try_new)
}

pub(crate) fn select_discovery_with<E, W>(
    etw_try: E,
    wmi_try: W,
) -> (Box<dyn DiscoverySource>, DiscoveryMode, Option<String>)
where
    E: FnOnce() -> Result<EtwProcessSource, EtwOpenError>,
    W: FnOnce() -> Result<WmiProcessSource, WmiOpenError>,
{
    match etw_try() {
        Ok(source) => (Box::new(source), DiscoveryMode::Etw, None),
        Err(etw_err) => {
            let etw_diag = etw_degrade_diagnostic(&etw_err);
            match wmi_try() {
                Ok(source) => (Box::new(source), DiscoveryMode::Wmi, Some(etw_diag)),
                Err(wmi_err) => {
                    let diagnostic = format!("{}; {}", etw_diag, wmi_degrade_diagnostic(&wmi_err));
                    (Box::new(SweepDiscovery::new()), DiscoveryMode::Sweep, Some(diagnostic))
                }
            }
        }
    }
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

    fn etw_fail() -> Result<EtwProcessSource, EtwOpenError> {
        Err(EtwOpenError {
            stage: "test_etw",
            code: 1,
        })
    }

    fn etw_ok() -> Result<EtwProcessSource, EtwOpenError> {
        Ok(EtwProcessSource::new_inject_only())
    }

    fn wmi_fail() -> Result<WmiProcessSource, WmiOpenError> {
        Err(WmiOpenError {
            stage: "test_wmi",
            code: 2,
        })
    }

    fn wmi_ok() -> Result<WmiProcessSource, WmiOpenError> {
        Ok(WmiProcessSource::new_inject_only())
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

    #[test]
    fn selector_prefers_etw_when_available() {
        let (_, mode, diagnostic) = select_discovery_with(etw_ok, wmi_ok);
        assert_eq!(mode, DiscoveryMode::Etw);
        assert!(diagnostic.is_none());
    }

    #[test]
    fn selector_falls_back_to_wmi_when_etw_fails() {
        let (_, mode, diagnostic) = select_discovery_with(etw_fail, wmi_ok);
        assert_eq!(mode, DiscoveryMode::Wmi);
        assert!(diagnostic.as_ref().is_some_and(|d| d.contains("test_etw")));
        assert!(diagnostic.as_ref().is_some_and(|d| d.contains("falling back")));
    }

    #[test]
    fn selector_falls_back_to_sweep_when_etw_and_wmi_fail() {
        let (_, mode, diagnostic) = select_discovery_with(etw_fail, wmi_fail);
        assert_eq!(mode, DiscoveryMode::Sweep);
        let diag = diagnostic.expect("degrade diagnostic");
        assert!(diag.contains("test_etw"));
        assert!(diag.contains("test_wmi"));
        assert!(diag.contains("falling back"));
    }
}
