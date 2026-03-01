//! Pipeline types.

use std::future::Future;
use std::pin::Pin;

// Re-export from events module
pub use crate::events::{Job, JobProgress, JobStatus};
// Re-export from core module
pub use crate::core::ScraperOptions;

/// Internal job state for tracking running jobs.
pub(crate) struct InternalJob {
    /// The public job representation.
    pub job: Job,
    /// Source URL for the job.
    pub source_url: String,
    /// Cancellation token.
    pub cancel_token: tokio_util::sync::CancellationToken,
}

/// Callbacks for job lifecycle events.
pub trait JobCallbacks: Send + Sync {
    /// Called when job status changes.
    fn on_status_change(&self, job: &Job) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    
    /// Called when job progress updates.
    fn on_progress(&self, job: &Job, progress: &JobProgress) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    
    /// Called when job encounters an error.
    fn on_error(&self, job: &Job, error: &str) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}