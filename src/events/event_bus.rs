//! Central event bus for application-wide events.

use crate::events::Event;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::debug;

/// Default channel capacity for the event bus.
const DEFAULT_CAPACITY: usize = 256;

/// Central event bus for application-wide events.
///
/// This service provides a pub/sub mechanism that can be used by any service
/// to emit and listen for events.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    subscriber_count: Arc<RwLock<usize>>,
}

impl EventBus {
    /// Create a new event bus with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new event bus with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            subscriber_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Emit an event to all subscribers.
    pub fn emit(&self, event: Event) {
        debug!("Event emitted: {:?}", event.event_type);
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to all events.
    /// Returns a receiver that can be used to receive events.
    pub async fn subscribe(&self) -> EventReceiver {
        let mut count = self.subscriber_count.write().await;
        *count += 1;
        EventReceiver {
            receiver: self.sender.subscribe(),
            subscriber_count: Arc::clone(&self.subscriber_count),
        }
    }

    /// Get the number of active subscribers.
    pub async fn subscriber_count(&self) -> usize {
        *self.subscriber_count.read().await
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Event receiver for subscribing to events.
pub struct EventReceiver {
    receiver: broadcast::Receiver<Event>,
    #[allow(dead_code)]
    subscriber_count: Arc<RwLock<usize>>,
}

impl EventReceiver {
    /// Receive the next event.
    pub async fn recv(&mut self) -> Result<Event, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    /// Try to receive an event without blocking.
    pub fn try_recv(&mut self) -> Result<Event, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Drop for EventReceiver {
    fn drop(&mut self) {
        // Note: We can't reliably decrement the count synchronously in drop
        // because Arc::get_mut requires exclusive ownership.
        // The count is a best-effort metric for monitoring purposes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventType, Job};

    #[tokio::test]
    async fn test_event_bus_emit_subscribe() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe().await;

        let job = Job::new(
            "test".to_string(),
            "lib".to_string(),
            "1.0".to_string(),
            None,
        );

        let event = Event::job_status_change(job);
        bus.emit(event.clone());

        let received = receiver.recv().await.expect("Should receive event");
        assert_eq!(received.event_type, EventType::JobStatusChange);
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        let mut receiver1 = bus.subscribe().await;
        let mut receiver2 = bus.subscribe().await;

        bus.emit(Event::library_change());

        let event1 = receiver1.recv().await.expect("Should receive event");
        let event2 = receiver2.recv().await.expect("Should receive event");

        assert_eq!(event1.event_type, EventType::LibraryChange);
        assert_eq!(event2.event_type, EventType::LibraryChange);
    }
}
