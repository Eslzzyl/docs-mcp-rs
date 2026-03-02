//! MCP tools implementation.

use crate::core::{Error, Result};
use crate::embed::Embedder;
use crate::pipeline::{PipelineManager, ScraperOptions};
use crate::store::{
    Connection, LibraryStore, PageStore, SearchOptions, VectorSearch, VersionStore,
};
use rmcp::model::{CallToolResult, Content};
use serde::{Deserialize, Serialize};

/// Tool parameters for scrape_docs.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScrapeDocsParams {
    /// The name of the library to index.
    pub library: String,
    /// The URL to start scraping from.
    pub url: String,
    /// Optional version string.
    #[serde(default)]
    pub version: Option<String>,
    /// Maximum pages to scrape.
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
    /// Maximum crawl depth.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_max_pages() -> usize {
    1000
}
fn default_max_depth() -> usize {
    10
}

/// Tool parameters for search_docs.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchDocsParams {
    /// The library name to search in.
    pub library: String,
    /// Optional version to search in.
    #[serde(default)]
    pub version: Option<String>,
    /// The search query.
    pub query: String,
    /// Maximum number of results.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

/// Tool parameters for list_libraries.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct ListLibrariesParams {
    #[serde(flatten)]
    _empty: std::collections::HashMap<String, serde_json::Value>,
}

/// Tool parameters for remove_library.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoveLibraryParams {
    /// The library name to remove.
    pub library: String,
}

// Tool functions that will be used by the server

/// Scrape a documentation website.
pub async fn scrape_docs(
    connection: &Connection,
    _embedder: &dyn Embedder,
    pipeline: &PipelineManager,
    params: ScrapeDocsParams,
) -> Result<CallToolResult> {
    // Enqueue the job through PipelineManager to enable progress tracking
    let options = ScraperOptions {
        max_pages: Some(params.max_pages),
        max_depth: Some(params.max_depth),
        ..Default::default()
    };

    let job_id = pipeline
        .enqueue(
            params.library.clone(),
            params.version.unwrap_or_default(),
            params.url.clone(),
            options,
        )
        .await?;

    // Wait for job completion
    pipeline.wait_for_job(&job_id).await?;

    // Get job result
    let job = pipeline
        .get_job(&job_id)
        .await
        .ok_or_else(|| Error::Mcp("Job not found after completion".to_string()))?;

    // Get the final page count from the database
    let lib_store = LibraryStore::new(connection);
    let ver_store = VersionStore::new(connection);
    let page_store = PageStore::new(connection);

    let library = lib_store
        .find_by_name(&params.library)?
        .ok_or_else(|| Error::NotFound(format!("Library '{}' not found", params.library)))?;

    let version = ver_store
        .find_by_library_and_name(library.id, &job.version)?
        .ok_or_else(|| Error::NotFound(format!("Version '{}' not found", job.version)))?;

    let pages = page_store.find_by_version(version.id)?;
    let pages_count = pages.len();

    // Count total chunks
    let total_chunks: usize = pages
        .iter()
        .map(|p| {
            connection
                .with_connection(|conn| {
                    conn.query_row(
                        "SELECT COUNT(*) FROM documents WHERE page_id = ?1",
                        rusqlite::params![p.id],
                        |row| row.get::<_, i64>(0),
                    )
                })
                .unwrap_or(0) as usize
        })
        .sum();

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Successfully scraped {} pages with {} chunks for library '{}' (version: {})",
        pages_count, total_chunks, params.library, version.name
    ))]))
}

/// Search indexed documentation.
pub async fn search_docs(
    connection: &Connection,
    embedder: &dyn Embedder,
    params: SearchDocsParams,
) -> Result<CallToolResult> {
    // Generate embedding for query
    let query_embedding = embedder.embed(&params.query).await?;

    // Perform hybrid search
    let search = VectorSearch::with_options(
        connection,
        SearchOptions {
            limit: params.limit,
            ..Default::default()
        },
    );

    let results = search
        .search(
            &params.library,
            params.version.as_deref(),
            &query_embedding,
            &params.query,
        )
        .await?;

    // Format results
    let output = if results.is_empty() {
        "No results found.".to_string()
    } else {
        results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "## Result {} (score: {:.3})\n**URL**: {}\n**Title**: {}\n\n{}\n",
                    i + 1,
                    r.score,
                    r.page.url,
                    r.page.title.as_deref().unwrap_or("N/A"),
                    r.document.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n---\n")
    };

    Ok(CallToolResult::success(vec![Content::text(output)]))
}

/// List all indexed libraries.
pub async fn list_libraries(connection: &Connection) -> Result<CallToolResult> {
    let lib_store = LibraryStore::new(connection);
    let ver_store = VersionStore::new(connection);

    let libraries = lib_store.list()?;

    let output = if libraries.is_empty() {
        "No libraries indexed yet.".to_string()
    } else {
        libraries
            .iter()
            .map(|lib| {
                let versions = ver_store.find_by_library(lib.id).unwrap_or_default();
                let version_names: Vec<String> = versions.iter().map(|v| v.name.clone()).collect();
                format!(
                    "- **{}** (ID: {})\n  Versions: {}",
                    lib.name,
                    lib.id,
                    if version_names.is_empty() {
                        "none".to_string()
                    } else {
                        version_names.join(", ")
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(CallToolResult::success(vec![Content::text(output)]))
}

/// Remove an indexed library.
pub async fn remove_library(
    connection: &Connection,
    params: RemoveLibraryParams,
) -> Result<CallToolResult> {
    let lib_store = LibraryStore::new(connection);

    // Find library
    let library = lib_store
        .find_by_name(&params.library)?
        .ok_or_else(|| Error::NotFound(format!("Library '{}' not found", params.library)))?;

    // Delete library (cascades to versions, pages, documents)
    lib_store.delete(library.id)?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Successfully removed library '{}'",
        params.library
    ))]))
}
