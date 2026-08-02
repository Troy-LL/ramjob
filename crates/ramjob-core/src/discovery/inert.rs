//! No-op discovery backend for unit tests (no live ETW/WMI).

use super::{DiscoveryEvent, DiscoverySource};

/// Inert discovery: always returns no events.
pub struct InertDiscovery;

impl DiscoverySource for InertDiscovery {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent> {
        Vec::new()
    }
}
