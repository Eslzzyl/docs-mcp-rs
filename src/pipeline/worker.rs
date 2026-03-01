//! Pipeline worker for executing jobs.

use crate::core::{
    ChunkMetadata, NewDocument, NewLibrary, NewPage, NewVersion, Result, ScraperOptions,
};
use crate::embed::Embedder;
use crate::scraper::{CrawlConfig, Crawler};
use crate::splitter::MarkdownSplitter;
use crate::store::{Connection, DocumentStore, LibraryStore, PageStore, VersionStore};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Pipeline worker for executing scraping jobs.
///
/// This is a simpler interface for running jobs synchronously.
/// For async job management, use `PipelineManager` instead.
pub struct PipelineWorker {
    connection: Arc<Connection>,
    embedder: Arc<RwLock<Box<dyn Embedder>>>,
}

impl PipelineWorker {
    /// Create a new pipeline worker.
    pub fn new(connection: Arc<Connection>, embedder: Arc<RwLock<Box<dyn Embedder>>>) -> Self {
        Self {
            connection,
            embedder,
        }
    }

    /// Run a simple crawl and index operation.
    /// This is a blocking operation that runs synchronously.
    pub async fn run_crawl(
        &self,
        library: &str,
        version: &str,
        source_url: &str,
        options: Option<ScraperOptions>,
    ) -> Result<usize> {
        debug!(
            "Starting crawl for {}@{} from {}",
            library, version, source_url
        );

        // Build crawler config
        let options = options.unwrap_or_default();
        let config = CrawlConfig::from(options.clone());

        let crawler = Crawler::new(config)?;
        let splitter = MarkdownSplitter::new();

        // Ensure library and version exist
        let library_store = LibraryStore::new(&self.connection);
        let version_store = VersionStore::new(&self.connection);
        let page_store = PageStore::new(&self.connection);
        let doc_store = DocumentStore::new(&self.connection);

        // Find or create library
        let lib = match library_store.find_by_name(library)? {
            Some(l) => l,
            None => library_store.create(&NewLibrary {
                name: library.to_string(),
            })?,
        };

        // Find or create version
        let ver = match version_store.find_by_library_and_name(lib.id, version)? {
            Some(v) => v,
            None => version_store.create(&NewVersion {
                library_id: lib.id,
                name: version.to_string(),
                source_url: Some(source_url.to_string()),
                scraper_options: None,
            })?,
        };

        // Update status to running
        version_store.update_status(ver.id, crate::core::types::VersionStatus::Running)?;

        // Crawl the site using streaming
        let mut rx = crawler.crawl_stream(source_url).await?;
        let mut pages_processed = 0;
        let mut total_pages = 0usize;

        // Get embedder
        let embedder_guard = self.embedder.read().await;

        // Process pages as they arrive from the stream
        while let Some(crawl_result) = rx.recv().await {
            total_pages += 1;

            // Create page record
            let page = page_store.upsert(&NewPage {
                version_id: ver.id,
                url: crawl_result.url.clone(),
                title: crawl_result.title.clone(),
                etag: crawl_result.etag.clone(),
                last_modified: crawl_result.last_modified.clone(),
                content_type: crawl_result.content_type.clone(),
                depth: crawl_result.depth as i32,
            })?;

            // Split content into chunks
            if !crawl_result.content.is_empty() {
                let chunks = splitter.split(&crawl_result.content);

                if !chunks.is_empty() {
                    // Generate embeddings
                    let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
                    let embeddings = match embedder_guard.embed_batch(&texts).await {
                        Ok(embs) => embs,
                        Err(e) => {
                            warn!("Failed to generate embeddings for {}: {}", crawl_result.url, e);
                            continue;
                        }
                    };

                    // Create documents
                    let documents: Vec<NewDocument> = chunks
                        .into_iter()
                        .zip(embeddings.into_iter())
                        .enumerate()
                        .map(|(i, (chunk, emb))| NewDocument {
                            page_id: page.id,
                            content: chunk.content,
                            metadata: ChunkMetadata::default(),
                            sort_order: i as i32,
                            embedding: Some(emb),
                        })
                        .collect();

                    if let Err(e) = doc_store.create_batch(&documents) {
                        warn!("Failed to store documents for {}: {}", crawl_result.url, e);
                        continue;
                    }
                }
            }

            pages_processed += 1;
            debug!("Processed page {}/{}: {}", pages_processed, total_pages, crawl_result.url);
        }

        // Update version status to completed
        version_store.update_status(ver.id, crate::core::types::VersionStatus::Completed)?;

        info!(
            "Crawled and indexed {} pages for {}@{}",
            pages_processed, library, version
        );
        Ok(total_pages)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_worker_creation() {
        // Just test that the type exists
    }
}
