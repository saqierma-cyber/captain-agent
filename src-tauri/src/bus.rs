//! ③ Event bus — tokio broadcast channel. The UDS client publishes
//! incoming events here; the store writer + UI pusher subscribe.

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
