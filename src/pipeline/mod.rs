//! Pipeline module for asynchronous job processing.

mod manager;
mod types;
mod worker;

pub use manager::PipelineManager;
pub use types::{Job, JobCallbacks, JobProgress, JobStatus, ScraperOptions};
pub use worker::PipelineWorker;
