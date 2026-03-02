//! Core domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A documentation library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    /// Unique identifier.
    pub id: i64,
    /// Library name.
    pub name: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// A version of a documentation library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// Unique identifier.
    pub id: i64,
    /// Library ID this version belongs to.
    pub library_id: i64,
    /// Version name (empty string for unversioned).
    pub name: String,
    /// Current status of the version.
    pub status: VersionStatus,
    /// Number of pages processed.
    pub progress_pages: i64,
    /// Total number of pages to process.
    pub progress_max_pages: i64,
    /// Error message if failed.
    pub error_message: Option<String>,
    /// Source URL for scraping.
    pub source_url: Option<String>,
    /// Scraper options as JSON.
    pub scraper_options: Option<serde_json::Value>,
    /// When processing started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the version was created.
    pub created_at: DateTime<Utc>,
    /// When the version was last updated.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Status of a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    /// Not yet indexed.
    #[default]
    NotIndexed,
    /// Queued for processing.
    Queued,
    /// Currently being processed.
    Running,
    /// Processing completed successfully.
    Completed,
    /// Processing failed.
    Failed,
    /// Processing was cancelled.
    Cancelled,
    /// Currently updating.
    Updating,
}

impl std::fmt::Display for VersionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionStatus::NotIndexed => write!(f, "not_indexed"),
            VersionStatus::Queued => write!(f, "queued"),
            VersionStatus::Running => write!(f, "running"),
            VersionStatus::Completed => write!(f, "completed"),
            VersionStatus::Failed => write!(f, "failed"),
            VersionStatus::Cancelled => write!(f, "cancelled"),
            VersionStatus::Updating => write!(f, "updating"),
        }
    }
}

impl std::str::FromStr for VersionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "not_indexed" => Ok(VersionStatus::NotIndexed),
            "queued" => Ok(VersionStatus::Queued),
            "running" => Ok(VersionStatus::Running),
            "completed" => Ok(VersionStatus::Completed),
            "failed" => Ok(VersionStatus::Failed),
            "cancelled" => Ok(VersionStatus::Cancelled),
            "updating" => Ok(VersionStatus::Updating),
            _ => Err(format!("Unknown version status: {}", s)),
        }
    }
}

/// A page from a documentation version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Unique identifier.
    pub id: i64,
    /// Version ID this page belongs to.
    pub version_id: i64,
    /// Page URL.
    pub url: String,
    /// Page title.
    pub title: Option<String>,
    /// HTTP ETag for caching.
    pub etag: Option<String>,
    /// HTTP Last-Modified header.
    pub last_modified: Option<String>,
    /// Content MIME type.
    pub content_type: Option<String>,
    /// Crawl depth from the starting URL.
    pub depth: i32,
    /// When the page was created.
    pub created_at: DateTime<Utc>,
    /// When the page was last updated.
    pub updated_at: Option<DateTime<Utc>>,
}

/// A document chunk from a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier.
    pub id: i64,
    /// Page ID this document belongs to.
    pub page_id: i64,
    /// Document content.
    pub content: String,
    /// Chunk metadata.
    pub metadata: ChunkMetadata,
    /// Sort order within the page.
    pub sort_order: i32,
    /// Vector embedding (optional, stored separately).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// When the document was created.
    pub created_at: DateTime<Utc>,
}

/// Metadata for a document chunk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Heading level depth.
    pub level: Option<i32>,
    /// Path of headings to this chunk.
    pub path: Option<Vec<String>>,
    /// Content types (e.g., "text", "code").
    pub types: Option<Vec<String>>,
}

/// Scraper options for a version.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScraperOptions {
    /// Maximum number of pages to scrape.
    pub max_pages: Option<usize>,
    /// Maximum crawl depth.
    pub max_depth: Option<usize>,
    /// URL patterns to include.
    pub include_patterns: Option<Vec<String>>,
    /// URL patterns to exclude.
    pub exclude_patterns: Option<Vec<String>>,
    /// Follow robots.txt rules.
    pub respect_robots_txt: Option<bool>,
    /// Scrape mode: "fetch" (HTTP only) or "browser" (headless Chrome).
    pub scrape_mode: Option<String>,
}

/// Search result with context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The document chunk.
    pub document: Document,
    /// The page this document belongs to.
    pub page: Page,
    /// The version this page belongs to.
    pub version: Version,
    /// The library this version belongs to.
    pub library: Library,
    /// Search relevance score.
    pub score: f32,
}

/// New library to create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLibrary {
    pub name: String,
}

/// New version to create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewVersion {
    pub library_id: i64,
    pub name: String,
    pub source_url: Option<String>,
    pub scraper_options: Option<ScraperOptions>,
}

/// New page to create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPage {
    pub version_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub depth: i32,
}

/// New document to create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocument {
    pub page_id: i64,
    pub content: String,
    pub metadata: ChunkMetadata,
    pub sort_order: i32,
    pub embedding: Option<Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_status_display() {
        assert_eq!(VersionStatus::NotIndexed.to_string(), "not_indexed");
        assert_eq!(VersionStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_version_status_from_str() {
        assert_eq!(
            "not_indexed".parse::<VersionStatus>().unwrap(),
            VersionStatus::NotIndexed
        );
        assert_eq!(
            "completed".parse::<VersionStatus>().unwrap(),
            VersionStatus::Completed
        );
    }
}
