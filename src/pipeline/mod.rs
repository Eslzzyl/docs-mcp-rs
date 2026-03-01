//! Pipeline module for asynchronous job processing.

mod manager;
mod types;
mod worker;

pub use manager::PipelineManager;
pub use types::{JobCallbacks, Job, JobProgress, JobStatus, ScraperOptions};
pub use worker::PipelineWorker;
