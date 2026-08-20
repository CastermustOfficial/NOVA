//! Bus di eventi: chiunque dentro il demone pubblica, chiunque fuori ascolta.
//!
//! E' un broadcast: ogni sottoscrittore ha la sua coda. Se un client e' lento
//! perde gli eventi piu' vecchi invece di rallentare tutto il demone — per la
//! telemetria e' il compromesso giusto.

use nova_proto::Event;
use serde_json::Value;
use tokio::sync::broadcast;

const CAPIENZA: usize = 1024;

#[derive(Clone)]
pub struct Bus {
    tx: broadcast::Sender<Event>,
}

impl Bus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CAPIENZA);
        Self { tx }
    }

    /// Pubblica. Non fallisce se non ascolta nessuno: e' il caso normale.
    pub fn publish(&self, event: Event) {
        let topic = event.topic.clone();
        if self.tx.send(event).is_err() {
            tracing::trace!(topic = %topic, "evento senza ascoltatori");
        }
    }

    pub fn emit(&self, topic: impl Into<String>, data: Value) {
        self.publish(Event::new(topic, data));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn listeners(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}
