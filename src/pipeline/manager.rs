//! Pipeline manager for orchestrating job queue.

use crate::core::{
    ChunkMetadata, NewDocument, NewLibrary, NewPage, NewVersion, Result, ScraperOptions,
};
use crate::embed::Embedder;
use crate::events::{CrawlPhase, Event, EventBus, Job, JobProgress, JobStatus};
use crate::scraper::{CrawlConfig, Crawler};
use crate::splitter::MarkdownSplitter;
use crate::store::{Connection, DocumentStore, LibraryStore, PageStore, VersionStore};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Internal job state.
struct InternalJob {
    /// Public job representation.
    job: Job,
    /// Source URL for the job.
    source_url: String,
    /// Scraper options.
    options: ScraperOptions,
    /// Cancellation token.
    cancel_token: tokio_util::sync::CancellationToken,
}

/// Pipeline manager for orchestrating scraping jobs.
pub struct PipelineManager {
    /// Database connection.
    connection: Arc<Connection>,
    /// Embedder for vector embeddings.
    embedder: Arc<RwLock<Box<dyn Embedder>>>,
    /// Event bus for notifications.
    event_bus: EventBus,
    /// Job storage.
    jobs: Arc<Mutex<HashMap<String, InternalJob>>>,
    /// Job queue (FIFO).
    queue: Arc<Mutex<VecDeque<String>>>,
    /// Currently running job count.
    running_count: Arc<Mutex<usize>>,
    /// Maximum concurrent jobs.
    concurrency: usize,
    /// Whether the manager is running.
    is_running: AtomicBool,
}

impl PipelineManager {
    /// Create a new pipeline manager.
    pub fn new(
        connection: Arc<Connection>,
        embedder: Arc<RwLock<Box<dyn Embedder>>>,
        event_bus: EventBus,
        concurrency: usize,
    ) -> Self {
        Self {
            connection,
            embedder,
            event_bus,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            running_count: Arc::new(Mutex::new(0)),
            concurrency,
            is_running: AtomicBool::new(false),
        }
    }

    /// Start the pipeline manager.
    pub async fn start(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("PipelineManager is already running");
            return;
        }

        self.is_running.store(true, Ordering::SeqCst);
        info!(
            "PipelineManager started with concurrency {}",
            self.concurrency
        );
    }

    /// Stop the pipeline manager.
    pub async fn stop(&self) {
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("PipelineManager is not running");
            return;
        }

        self.is_running.store(false, Ordering::SeqCst);
        info!("PipelineManager stopping");

        // Cancel all running jobs
        let jobs = self.jobs.lock().await;
        for (_, internal_job) in jobs.iter() {
            if internal_job.job.status == JobStatus::Running {
                internal_job.cancel_token.cancel();
            }
        }
    }

    /// Enqueue a new scraping job.
    pub async fn enqueue(
        &self,
        library: String,
        version: String,
        source_url: String,
        options: ScraperOptions,
    ) -> Result<String> {
        let job_id = Uuid::new_v4().to_string();
        let cancel_token = tokio_util::sync::CancellationToken::new();

        let job = Job {
            id: job_id.clone(),
            library: library.clone(),
            version: version.clone(),
            status: JobStatus::Queued,
            progress: None,
            error: None,
            source_url: Some(source_url.clone()),
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            finished_at: None,
        };

        // Update database status
        self.ensure_version(&job).await?;

        // Store job
        let internal_job = InternalJob {
            job: job.clone(),
            source_url,
            options,
            cancel_token,
        };

        self.jobs.lock().await.insert(job_id.clone(), internal_job);
        self.queue.lock().await.push_back(job_id.clone());

        // Emit event
        self.event_bus.emit(Event::job_status_change(job.clone()));

        info!(
            "Job enqueued: {} for {}@{}",
            job_id, job.library, job.version
        );

        // Process queue
        if self.is_running.load(Ordering::SeqCst) {
            self.process_queue().await;
        }

        Ok(job_id)
    }

    /// Get a job by ID.
    pub async fn get_job(&self, job_id: &str) -> Option<Job> {
        self.jobs.lock().await.get(job_id).map(|j| j.job.clone())
    }

    /// Get all jobs.
    pub async fn get_jobs(&self) -> Vec<Job> {
        self.jobs
            .lock()
            .await
            .values()
            .map(|j| j.job.clone())
            .collect()
    }

    /// Get jobs by status.
    pub async fn get_jobs_by_status(&self, status: JobStatus) -> Vec<Job> {
        self.jobs
            .lock()
            .await
            .values()
            .filter(|j| j.job.status == status)
            .map(|j| j.job.clone())
            .collect()
    }

    /// Cancel a job.
    pub async fn cancel_job(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.jobs.lock().await;

        if let Some(internal_job) = jobs.get_mut(job_id) {
            match internal_job.job.status {
                JobStatus::Queued => {
                    // Remove from queue
                    let mut queue = self.queue.lock().await;
                    queue.retain(|id| id != job_id);

                    // Update status
                    internal_job.job.status = JobStatus::Cancelled;
                    internal_job.job.finished_at = Some(chrono::Utc::now().timestamp_millis());

                    let job = internal_job.job.clone();
                    drop(jobs);

                    self.event_bus.emit(Event::job_status_change(job));
                    info!("Job cancelled (was queued): {}", job_id);
                }
                JobStatus::Running => {
                    // Signal cancellation
                    internal_job.job.status = JobStatus::Cancelling;
                    internal_job.cancel_token.cancel();

                    info!("Signalling cancellation for running job: {}", job_id);
                }
                _ => {
                    warn!(
                        "Job {} cannot be cancelled in state {:?}",
                        job_id, internal_job.job.status
                    );
                }
            }
        }

        Ok(())
    }

    /// Wait for job completion.
    pub async fn wait_for_job(&self, job_id: &str) -> Result<()> {
        // Simple polling implementation
        loop {
            if let Some(job) = self.get_job(job_id).await {
                match job.status {
                    JobStatus::Completed => return Ok(()),
                    JobStatus::Failed => {
                        return Err(crate::core::Error::Mcp(
                            job.error.unwrap_or_else(|| "Job failed".to_string()),
                        ));
                    }
                    JobStatus::Cancelled => {
                        return Err(crate::core::Error::Mcp("Job cancelled".to_string()));
                    }
                    _ => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            } else {
                return Err(crate::core::Error::Mcp(format!(
                    "Job not found: {}",
                    job_id
                )));
            }
        }
    }

    /// Clear completed jobs.
    pub async fn clear_completed(&self) -> usize {
        let completed_statuses = [
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ];
        let mut jobs = self.jobs.lock().await;
        let mut count = 0;

        let to_remove: Vec<String> = jobs
            .iter()
            .filter(|(_, j)| completed_statuses.contains(&j.job.status))
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            jobs.remove(&id);
            count += 1;
        }

        if count > 0 {
            info!("Cleared {} completed job(s)", count);
            self.event_bus.emit(Event::job_list_change());
        }

        count
    }

    /// Process the job queue.
    async fn process_queue(&self) {
        if !self.is_running.load(Ordering::SeqCst) {
            return;
        }

        let running_count = *self.running_count.lock().await;

        if running_count >= self.concurrency {
            return;
        }

        // Try to start a job from the queue
        let job_id = {
            let mut queue = self.queue.lock().await;
            queue.pop_front()
        };

        if let Some(job_id) = job_id {
            self.start_job(&job_id).await;
        }
    }

    /// Start executing a job.
    async fn start_job(&self, job_id: &str) {
        // Increment running count
        *self.running_count.lock().await += 1;

        // Get job info and update status
        let connection = Arc::clone(&self.connection);
        let embedder = Arc::clone(&self.embedder);
        let event_bus = self.event_bus.clone();
        let jobs = Arc::clone(&self.jobs);
        let running_count = Arc::clone(&self.running_count);

        // Update job status to Running
        {
            let mut jobs_guard = jobs.lock().await;
            if let Some(internal_job) = jobs_guard.get_mut(job_id) {
                internal_job.job.status = JobStatus::Running;
                internal_job.job.started_at = Some(chrono::Utc::now().timestamp_millis());

                let job = internal_job.job.clone();
                drop(jobs_guard);

                event_bus.emit(Event::job_status_change(job));
            } else {
                *self.running_count.lock().await -= 1;
                return;
            }
        }

        let job_id_owned = job_id.to_string();

        // Spawn task to run the job
        tokio::spawn(async move {
            // Get options and cancel token
            let (source_url, options, cancel_token) = {
                let jobs_guard = jobs.lock().await;
                if let Some(internal_job) = jobs_guard.get(&job_id_owned) {
                    (
                        internal_job.source_url.clone(),
                        internal_job.options.clone(),
                        internal_job.cancel_token.clone(),
                    )
                } else {
                    *running_count.lock().await -= 1;
                    return;
                }
            };

            // Execute the job
            let result = execute_job_internal(
                &job_id_owned,
                &connection,
                &embedder,
                &source_url,
                &options,
                &cancel_token,
                &event_bus,
                &jobs,
            )
            .await;

            // Update final status
            {
                let mut jobs_guard = jobs.lock().await;
                if let Some(internal_job) = jobs_guard.get_mut(&job_id_owned) {
                    match result {
                        Ok(()) => {
                            internal_job.job.status = JobStatus::Completed;
                            info!("Job completed: {}", job_id_owned);
                        }
                        Err(e) => {
                            if cancel_token.is_cancelled() {
                                internal_job.job.status = JobStatus::Cancelled;
                                info!("Job cancelled: {}", job_id_owned);
                            } else {
                                internal_job.job.status = JobStatus::Failed;
                                internal_job.job.error = Some(e.to_string());
                                error!("Job failed: {}: {}", job_id_owned, e);
                            }
                        }
                    }
                    internal_job.job.finished_at = Some(chrono::Utc::now().timestamp_millis());

                    let job = internal_job.job.clone();
                    event_bus.emit(Event::job_status_change(job));
                }
            }

            // Decrement running count
            *running_count.lock().await -= 1;
        });
    }

    /// Ensure library and version exist in database.
    async fn ensure_version(&self, job: &Job) -> Result<()> {
        let library_store = LibraryStore::new(&self.connection);
        let version_store = VersionStore::new(&self.connection);

        // Find or create library
        let library = match library_store.find_by_name(&job.library)? {
            Some(lib) => lib,
            None => library_store.create(&NewLibrary {
                name: job.library.clone(),
            })?,
        };

        // Find or create version
        let version = match version_store.find_by_library_and_name(library.id, &job.version)? {
            Some(ver) => ver,
            None => version_store.create(&NewVersion {
                library_id: library.id,
                name: job.version.clone(),
                source_url: job.source_url.clone(),
                scraper_options: None,
            })?,
        };

        // Update status to queued
        version_store.update_status(version.id, crate::core::types::VersionStatus::Queued)?;

        Ok(())
    }
}

/// Internal function to execute a job.
async fn execute_job_internal(
    job_id: &str,
    connection: &Connection,
    embedder: &Arc<RwLock<Box<dyn Embedder>>>,
    source_url: &str,
    options: &ScraperOptions,
    cancel_token: &tokio_util::sync::CancellationToken,
    event_bus: &EventBus,
    jobs: &Arc<Mutex<HashMap<String, InternalJob>>>,
) -> Result<()> {
    debug!("[{}] Worker starting job for {}", job_id, source_url);

    // Build crawler config from options
    let config = CrawlConfig::from(options.clone());

    let crawler = Crawler::new(config)?;
    let splitter = MarkdownSplitter::new();

    // Get library and version info
    let (library, version) = {
        let jobs_guard = jobs.lock().await;
        jobs_guard
            .get(job_id)
            .map(|j| (j.job.library.clone(), j.job.version.clone()))
            .unwrap_or((String::new(), String::new()))
    };

    // Ensure library and version exist
    let library_store = LibraryStore::new(connection);
    let version_store = VersionStore::new(connection);
    let page_store = PageStore::new(connection);
    let doc_store = DocumentStore::new(connection);

    // Find or create library
    let lib = match library_store.find_by_name(&library)? {
        Some(l) => l,
        None => library_store.create(&NewLibrary {
            name: library.clone(),
        })?,
    };

    // Find or create version
    let ver = match version_store.find_by_library_and_name(lib.id, &version)? {
        Some(v) => v,
        None => version_store.create(&NewVersion {
            library_id: lib.id,
            name: version.clone(),
            source_url: Some(source_url.to_string()),
            scraper_options: None,
        })?,
    };

    // Crawl the site using streaming
    let max_pages = options.max_pages.unwrap_or(1000);
    let max_depth = options.max_depth.unwrap_or(3);
    
    // Start crawling
    let mut rx = crawler.crawl_stream(source_url, None).await?;

    debug!("[{}] Starting stream crawl", job_id);

    // Get embedder for processing
    let embedder_guard = embedder.read().await;
    
    // Progress tracking
    let mut pages_scraped = 0;
    let mut last_progress_update = std::time::Instant::now();
    
    // Send initial progress
    {
        let progress = JobProgress {
            phase: CrawlPhase::Discovering,
            pages_scraped: 0,
            total_discovered: 1,
            queue_length: 1,
            max_pages,
            total_pages: 1,
            pages_explored: 1,
            current_url: Some(source_url.to_string()),
            depth: 0,
            max_depth,
            is_discovering: true,
        };
        let mut jobs_guard = jobs.lock().await;
        if let Some(internal_job) = jobs_guard.get_mut(job_id) {
            internal_job.job.progress = Some(progress.clone());
            let job = internal_job.job.clone();
            event_bus.emit(Event::job_progress(job, progress));
        }
    }

    // Process pages as they arrive from the stream
    while let Some(crawl_result) = rx.recv().await {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(crate::core::Error::Mcp("Job cancelled".to_string()));
        }

        pages_scraped += 1;
        
        // Estimate discovered pages (pages scraped + likely more in queue)
        // We don't have direct access to queue length, so we estimate based on progress
        // Initially we estimate higher to show discovering phase
        let estimated_queue = if pages_scraped < 5 {
            // During early phase, assume there are more pages to discover
            std::cmp::max(3, pages_scraped * 2)
        } else {
            // Later phase, estimate based on observed pattern
            std::cmp::max(1, pages_scraped / 3)
        };
        let total_discovered = pages_scraped + estimated_queue;

        // Send progress update (throttled to every 50ms, but always send for first 5 pages)
        let now = std::time::Instant::now();
        let should_update = pages_scraped <= 5 || now.duration_since(last_progress_update).as_millis() >= 50;
        
        if should_update {
            // Determine phase: show "Discovering" until we've processed 3 pages AND have a reasonable estimate
            let phase = if pages_scraped < 3 {
                CrawlPhase::Discovering
            } else {
                CrawlPhase::Scraping
            };
            
            let effective_total = std::cmp::min(total_discovered, max_pages);
            
            let progress = JobProgress {
                phase: phase.clone(),
                pages_scraped,
                total_discovered,
                queue_length: estimated_queue,
                max_pages,
                total_pages: std::cmp::max(effective_total, 1),
                pages_explored: total_discovered,
                current_url: Some(crawl_result.url.clone()),
                depth: crawl_result.depth,
                max_depth,
                is_discovering: phase == CrawlPhase::Discovering,
            };
            
            info!("[{}] Progress update: phase={:?}, scraped={}/{}, queue={}", 
                  job_id, phase, pages_scraped, effective_total, estimated_queue);
            
            // Update job progress and emit event
            let mut jobs_guard = jobs.lock().await;
            if let Some(internal_job) = jobs_guard.get_mut(job_id) {
                internal_job.job.progress = Some(progress.clone());
                let job = internal_job.job.clone();
                event_bus.emit(Event::job_progress(job, progress));
            }
            drop(jobs_guard);
            
            last_progress_update = now;
        }

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
                // Generate embeddings for all chunks
                let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
                let embeddings = match embedder_guard.embed_batch(&texts).await {
                    Ok(embs) => embs,
                    Err(e) => {
                        warn!("[{}] Failed to generate embeddings for {}: {}", job_id, crawl_result.url, e);
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

                // Store documents
                if let Err(e) = doc_store.create_batch(&documents) {
                    warn!("[{}] Failed to store documents for {}: {}", job_id, crawl_result.url, e);
                    continue;
                }

                debug!(
                    "[{}] Stored {} chunks from {}",
                    job_id,
                    documents.len(),
                    crawl_result.url
                );
            }
        }
    }

    // Update version status to completed
    version_store.update_status(ver.id, crate::core::types::VersionStatus::Completed)?;

    debug!("[{}] Worker finished job successfully", job_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_manager_creation() {
        // Would need mock connection for real tests
    }
}
