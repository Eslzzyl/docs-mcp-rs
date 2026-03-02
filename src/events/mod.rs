//! Event system for application-wide event distribution.

mod event_bus;
mod types;

pub use event_bus::EventBus;
pub use types::{CrawlPhase, Event, EventPayload, EventType, Job, JobProgress, JobStatus};
