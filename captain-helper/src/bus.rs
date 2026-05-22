//! Helper-internal broadcast channel. The osquery subscriber publishes
//! events here; the UDS server subscribes per-connected-client and
//! forwards over the socket.

use captain_common::Event;
use tokio::sync::broadcast;

const BUS_CAPACITY: usize = 16_384;

#[derive(Clone)]
pub struct Bus {
    tx: broadcast::Sender<Event>,
}

impl Bus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self { tx }
    }

    pub fn publish(&self, ev: Event) {
        // Send fails only if there are no receivers; drop is fine.
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}
