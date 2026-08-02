//! Shared event queue shell for ETW/WMI backends.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::{DiscoveryEvent, DiscoverySource};

/// Thread-safe discovery event queue with drain/inject helpers.
#[derive(Clone)]
pub(crate) struct QueuedDiscovery {
    queue: Arc<Mutex<VecDeque<DiscoveryEvent>>>,
}

impl QueuedDiscovery {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn inner(&self) -> &Arc<Mutex<VecDeque<DiscoveryEvent>>> {
        &self.queue
    }

    pub fn inject_events(&self, events: impl IntoIterator<Item = DiscoveryEvent>) {
        if let Ok(mut q) = self.queue.lock() {
            q.extend(events);
        }
    }

    pub fn drain(&self) -> Vec<DiscoveryEvent> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.queue.lock() {
            while let Some(e) = q.pop_front() {
                out.push(e);
            }
        }
        out
    }
}

impl DiscoverySource for QueuedDiscovery {
    fn poll_events(&mut self) -> Vec<DiscoveryEvent> {
        self.drain()
    }
}
