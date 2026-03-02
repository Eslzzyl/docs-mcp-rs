//! Event types for the event bus system.

use crate::core::types::VersionStatus;
use serde::{Deserialize, Serialize};

/// Event type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// Job status changed.
    JobStatusChange,
    /// Job progress updated.
    JobProgress,
    /// Library changed (added/removed).
    LibraryChange,
    /// Job list changed (cleared).
    JobListChange,
}

/// Job status for pipeline jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is queued waiting to run.
    Queued,
    /// Job is currently running.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed with an error.
    Failed,
    /// Job is being cancelled.
    Cancelling,
    /// Job was cancelled.
    Cancelled,
}

impl From<JobStatus> for VersionStatus {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::Queued => VersionStatus::Queued,
            JobStatus::Running => VersionStatus::Running,
            JobStatus::Completed => VersionStatus::Completed,
            JobStatus::Failed => VersionStatus::Failed,
            JobStatus::Cancelled => VersionStatus::Cancelled,
            JobStatus::Cancelling => VersionStatus::Running, // Keep as running until actually cancelled
        }
    }
}

/// Crawl phase enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlPhase {
    /// Discovering pages (exploration phase).
    Discovering,
    /// Scraping content (actual crawling).
    Scraping,
}

impl Default for CrawlPhase {
    fn default() -> Self {
        Self::Discovering
    }
}

/// Progress information for a running job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    /// Current phase of the crawl.
    #[serde(default)]
    pub phase: CrawlPhase,
    /// Number of pages scraped so far.
    pub pages_scraped: usize,
    /// Total pages discovered (including queue).
    pub total_discovered: usize,
    /// Pages currently in queue waiting to be processed.
    pub queue_length: usize,
    /// Maximum pages limit.
    pub max_pages: usize,
    /// Total pages to scrape (effective total, min of discovered and max_pages).
    #[serde(default = "default_total_pages")]
    pub total_pages: usize,
    /// Number of pages explored/during discovery phase.
    pub pages_explored: usize,
    /// Current URL being processed.
    pub current_url: Option<String>,
    /// Current depth.
    pub depth: usize,
    /// Maximum depth.
    pub max_depth: usize,
    /// Whether the total is still being discovered.
    #[serde(default)]
    pub is_discovering: bool,
}

fn default_total_pages() -> usize {
    1
}

/// Pipeline job representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique identifier for the job.
    pub id: String,
    /// Library name.
    pub library: String,
    /// Version (empty string for unversioned).
    pub version: String,
    /// Current status.
    pub status: JobStatus,
    /// Progress information.
    pub progress: Option<JobProgress>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Source URL.
    pub source_url: Option<String>,
    /// Creation timestamp (milliseconds since epoch).
    pub created_at: i64,
    /// Start timestamp (milliseconds since epoch).
    pub started_at: Option<i64>,
    /// Finish timestamp (milliseconds since epoch).
    pub finished_at: Option<i64>,
}

impl Job {
    /// Create a new job with the current timestamp.
    pub fn new(id: String, library: String, version: String, source_url: Option<String>) -> Self {
        Self {
            id,
            library,
            version,
            status: JobStatus::Queued,
            progress: None,
            error: None,
            source_url,
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            finished_at: None,
        }
    }
}

/// Event payload types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventPayload {
    /// Job status changed.
    JobStatusChange(Job),
    /// Job progress updated.
    JobProgress { job: Job, progress: JobProgress },
    /// Library changed.
    LibraryChange,
    /// Job list changed.
    JobListChange,
}

/// Event with type and payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event type.
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// Event payload.
    pub payload: EventPayload,
}

impl Event {
    /// Create a job status change event.
    pub fn job_status_change(job: Job) -> Self {
        Self {
            event_type: EventType::JobStatusChange,
            payload: EventPayload::JobStatusChange(job),
        }
    }

    /// Create a job progress event.
    pub fn job_progress(job: Job, progress: JobProgress) -> Self {
        Self {
            event_type: EventType::JobProgress,
            payload: EventPayload::JobProgress { job, progress },
        }
    }

    /// Create a library change event.
    pub fn library_change() -> Self {
        Self {
            event_type: EventType::LibraryChange,
            payload: EventPayload::LibraryChange,
        }
    }

    /// Create a job list change event.
    pub fn job_list_change() -> Self {
        Self {
            event_type: EventType::JobListChange,
            payload: EventPayload::JobListChange,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_to_version_status() {
        assert_eq!(
            VersionStatus::from(JobStatus::Queued),
            VersionStatus::Queued
        );
        assert_eq!(
            VersionStatus::from(JobStatus::Running),
            VersionStatus::Running
        );
        assert_eq!(
            VersionStatus::from(JobStatus::Completed),
            VersionStatus::Completed
        );
        assert_eq!(
            VersionStatus::from(JobStatus::Failed),
            VersionStatus::Failed
        );
        assert_eq!(
            VersionStatus::from(JobStatus::Cancelled),
            VersionStatus::Cancelled
        );
        assert_eq!(
            VersionStatus::from(JobStatus::Cancelling),
            VersionStatus::Running
        );
    }

    #[test]
    fn test_event_creation() {
        let job = Job::new(
            "test-id".to_string(),
            "test-lib".to_string(),
            "1.0.0".to_string(),
            Some("https://example.com".to_string()),
        );

        let event = Event::job_status_change(job.clone());
        assert_eq!(event.event_type, EventType::JobStatusChange);

        let progress = JobProgress {
            phase: CrawlPhase::Scraping,
            pages_scraped: 10,
            total_pages: 100,
            total_discovered: 150,
            queue_length: 50,
            max_pages: 100,
            pages_explored: 100,
            current_url: Some("https://example.com/page".to_string()),
            depth: 2,
            max_depth: 5,
            is_discovering: false,
        };

        let event = Event::job_progress(job, progress);
        assert_eq!(event.event_type, EventType::JobProgress);
    }

    #[test]
    fn test_job_serialization() {
        let job = Job::new(
            "test-id".to_string(),
            "test-lib".to_string(),
            "1.0.0".to_string(),
            Some("https://example.com".to_string()),
        );

        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("test-id"));
        assert!(json.contains("test-lib"));
    }
}
