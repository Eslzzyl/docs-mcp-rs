//! MCP tools implementation.

use crate::core::{Error, Result};
use crate::embed::Embedder;
use crate::scraper::{CrawlConfig, Crawler};
use crate::splitter::MarkdownSplitter;
use crate::store::{
    Connection, DocumentStore, LibraryStore, PageStore, SearchOptions, VectorSearch, VersionStore,
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListLibrariesParams;

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
    embedder: &dyn Embedder,
    params: ScrapeDocsParams,
) -> Result<CallToolResult> {
    // Create library
    let lib_store = LibraryStore::new(connection);
    let library = lib_store.create(&crate::core::NewLibrary {
        name: params.library.clone(),
    })?;

    // Create version
    let ver_store = VersionStore::new(connection);
    let version = ver_store.create(&crate::core::NewVersion {
        library_id: library.id,
        name: params.version.unwrap_or_default(),
        source_url: Some(params.url.clone()),
        scraper_options: None,
    })?;

    // Crawl the website
    let crawl_config = CrawlConfig {
        max_pages: params.max_pages,
        max_depth: params.max_depth,
        ..Default::default()
    };

    let crawler = Crawler::new(crawl_config).map_err(|e| Error::Scraping(e.to_string()))?;
    let pages = crawler
        .crawl(&params.url)
        .await
        .map_err(|e| Error::Scraping(e.to_string()))?;

    let pages_count = pages.len();

    // Process each page
    let page_store = PageStore::new(connection);
    let doc_store = DocumentStore::new(connection);
    let splitter = MarkdownSplitter::new();

    let mut total_chunks = 0;

    for page_content in pages {
        // Create page record
        let page = page_store.upsert(&crate::core::NewPage {
            version_id: version.id,
            url: page_content.url.clone(),
            title: page_content.title.clone(),
            etag: None,
            last_modified: None,
            content_type: Some("text/markdown".to_string()),
            depth: page_content.depth as i32,
        })?;

        // Split into chunks
        let chunks = splitter.split(&page_content.content);

        if chunks.is_empty() {
            continue;
        }

        // Generate embeddings in batch
        let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = embedder.embed_batch(&texts).await?;

        // Create documents with embeddings
        let docs: Vec<crate::core::NewDocument> = chunks
            .into_iter()
            .zip(embeddings.into_iter())
            .enumerate()
            .map(|(i, (chunk, embedding))| crate::core::NewDocument {
                page_id: page.id,
                content: chunk.content,
                metadata: chunk.metadata,
                sort_order: i as i32,
                embedding: Some(embedding),
            })
            .collect();

        doc_store.create_batch(&docs)?;
        total_chunks += docs.len();
    }

    // Update version status
    ver_store.update_status(version.id, crate::core::VersionStatus::Completed)?;

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
