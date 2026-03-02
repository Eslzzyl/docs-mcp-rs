//! Web crawler for documentation sites.

use crate::core::{Error, Result, ScraperOptions};
use crate::scraper::{BrowserFetchConfig, BrowserPool, Fetcher, HtmlToMarkdown, LinkExtractor};
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Semaphore, mpsc};
use tokio::time::sleep;
use tracing::{debug, info, trace, warn};
use url::Url;

/// Progress information during crawling.
#[derive(Debug, Clone)]
pub struct CrawlProgress {
    /// Number of pages scraped so far.
    pub pages_scraped: usize,
    /// Total pages discovered (including in queue).
    pub total_discovered: usize,
    /// Current queue length.
    pub queue_length: usize,
    /// Current URL being processed.
    pub current_url: Option<String>,
    /// Current depth.
    pub depth: usize,
}

/// Callback for progress updates.
pub type ProgressCallback = Arc<dyn Fn(CrawlProgress) + Send + Sync>;

/// Result of crawling a page.

/// Result of crawling a page.
#[derive(Debug, Clone)]
pub struct CrawlResult {
    /// The page URL.
    pub url: String,
    /// Page title.
    pub title: Option<String>,
    /// Markdown content.
    pub content: String,
    /// Content type.
    pub content_type: Option<String>,
    /// ETag for caching.
    pub etag: Option<String>,
    /// Last-Modified header.
    pub last_modified: Option<String>,
    /// Crawl depth.
    pub depth: usize,
}

/// Scrape mode for determining how to fetch pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapeMode {
    /// Simple HTTP fetch.
    Fetch,
    /// Browser-based rendering.
    Browser,
}

impl Default for ScrapeMode {
    fn default() -> Self {
        ScrapeMode::Browser
    }
}

impl std::str::FromStr for ScrapeMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fetch" => Ok(ScrapeMode::Fetch),
            "browser" => Ok(ScrapeMode::Browser),
            _ => Err(format!(
                "Unknown scrape mode: {}, expected 'fetch' or 'browser'",
                s
            )),
        }
    }
}

/// Crawler configuration.
#[derive(Debug, Clone)]
pub struct CrawlConfig {
    /// Maximum number of pages to crawl.
    pub max_pages: usize,
    /// Maximum crawl depth.
    pub max_depth: usize,
    /// Maximum concurrent requests.
    pub max_concurrency: usize,
    /// Delay between requests (in milliseconds).
    pub delay_ms: u64,
    /// URL patterns to include.
    pub include_patterns: Vec<Regex>,
    /// URL patterns to exclude.
    pub exclude_patterns: Vec<Regex>,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// User agent string.
    pub user_agent: String,
    /// Scrape mode.
    pub scrape_mode: ScrapeMode,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_pages: 1000,
            max_depth: 3,
            max_concurrency: 5,
            delay_ms: 100,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            timeout_secs: 30,
            user_agent: format!("docs-mcp-rs/{}", env!("CARGO_PKG_VERSION")),
            scrape_mode: ScrapeMode::Browser,
        }
    }
}

impl From<ScraperOptions> for CrawlConfig {
    fn from(options: ScraperOptions) -> Self {
        let mut config = Self::default();

        if let Some(max_pages) = options.max_pages {
            config.max_pages = max_pages;
        }
        if let Some(max_depth) = options.max_depth {
            config.max_depth = max_depth;
        }
        if let Some(include) = options.include_patterns {
            config.include_patterns = include.iter().filter_map(|p| Regex::new(p).ok()).collect();
        }
        if let Some(exclude) = options.exclude_patterns {
            config.exclude_patterns = exclude.iter().filter_map(|p| Regex::new(p).ok()).collect();
        }
        if let Some(mode) = options.scrape_mode {
            if let Ok(scrape_mode) = mode.parse::<ScrapeMode>() {
                config.scrape_mode = scrape_mode;
            }
        }

        config
    }
}

/// Web crawler.
pub struct Crawler {
    config: CrawlConfig,
    fetcher: Arc<Fetcher>,
    browser_pool: Arc<BrowserPool>,
}

impl Crawler {
    /// Create a new crawler with the given configuration.
    pub fn new(config: CrawlConfig) -> Result<Self> {
        let fetcher = Arc::new(Fetcher::new(crate::scraper::HttpClient::new(
            &config.user_agent,
            config.timeout_secs,
        )?));

        let browser_config = BrowserFetchConfig {
            chrome_path: std::env::var("CHROME_PATH").ok(),
            headless: true,
            timeout_secs: config.timeout_secs,
            wait_after_load_ms: config.delay_ms.saturating_mul(2),
            user_agent: Some(config.user_agent.clone()),
            window_width: 1920,
            window_height: 1080,
            extract_shadow_dom: true,
            process_iframes: true,
            block_images: true,
            block_css: false,
            headers: std::collections::HashMap::new(),
        };
        let browser_pool = Arc::new(BrowserPool::new(browser_config));

        Ok(Self {
            config,
            fetcher,
            browser_pool,
        })
    }

    /// Create a crawler with default configuration.
    pub fn with_defaults() -> Result<Self> {
        Self::new(CrawlConfig::default())
    }

    /// Crawl a documentation site starting from the given URL.
    pub async fn crawl(&self, start_url: &str) -> Result<Vec<CrawlResult>> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let pages_count = Arc::new(AtomicUsize::new(0));

        // Parse and normalize start URL
        let base_url = Url::parse(start_url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid start URL: {}", e)))?;
        let base_domain = base_url.host_str().unwrap_or("").to_string();

        debug!(
            "Starting crawl from: {} (domain: {})",
            start_url, base_domain
        );
        debug!(
            "Crawl config: max_pages={}, max_depth={}, delay_ms={}",
            self.config.max_pages, self.config.max_depth, self.config.delay_ms
        );

        queue.push_back((start_url.to_string(), 0usize));
        visited.insert(start_url.to_string());

        while let Some((url, depth)) = queue.pop_front() {
            let current_count = pages_count.load(Ordering::Relaxed);
            debug!(
                "Processing URL: {} (depth: {}, queue: {}, visited: {}, pages: {})",
                url,
                depth,
                queue.len(),
                visited.len(),
                current_count
            );

            // Check limits
            if current_count >= self.config.max_pages {
                debug!(
                    "Reached max_pages limit ({}), stopping crawl",
                    self.config.max_pages
                );
                break;
            }

            if depth > self.config.max_depth {
                trace!(
                    "Skipping {}: depth {} exceeds max_depth {}",
                    url, depth, self.config.max_depth
                );
                continue;
            }

            // Check URL patterns
            if !self.should_crawl(&url, &base_domain) {
                trace!("Skipping {}: does not match crawl patterns", url);
                continue;
            }

            // Add delay between requests
            if self.config.delay_ms > 0 {
                trace!("Waiting {}ms before fetching {}", self.config.delay_ms, url);
                sleep(Duration::from_millis(self.config.delay_ms)).await;
            }

            // Fetch and process the page
            debug!("Fetching and processing: {}", url);
            match self.process_page(&url, depth).await {
                Ok((result, links)) => {
                    debug!(
                        "Successfully processed {}: title='{:?}', content_length={}, links={}",
                        url,
                        result.title,
                        result.content.len(),
                        links.len()
                    );
                    results.push(result);
                    let new_count = pages_count.fetch_add(1, Ordering::Relaxed) + 1;

                    // Add new links to queue
                    let mut new_links = 0;
                    for link in links {
                        trace!(
                            "Found link: {} (internal: {}, text: '{}')",
                            link.url, link.is_internal, link.text
                        );
                        if !visited.contains(&link.url) && link.is_internal {
                            visited.insert(link.url.clone());
                            queue.push_back((link.url, depth + 1));
                            new_links += 1;
                        }
                    }
                    debug!("Added {} new links to queue from {}", new_links, url);
                    trace!(
                        "Progress: {}/{} pages, queue: {}",
                        new_count,
                        self.config.max_pages,
                        queue.len()
                    );
                }
                Err(e) => {
                    warn!("Failed to process {}: {}", url, e);
                }
            }
        }

        debug!("Crawl complete: {} pages collected", results.len());

        // Sort by depth, then by URL
        results.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.url.cmp(&b.url)));

        Ok(results)
    }

    /// Crawl a documentation site starting from the given URL and stream results.
    /// Returns a receiver channel that yields crawl results as they are processed.
    /// Optionally accepts a progress callback for real-time progress updates.
    /// Optionally accepts a cancellation token to gracefully stop crawling.
    ///
    /// This method uses true concurrent processing - multiple pages are fetched
    /// and processed in parallel (up to max_concurrency).
    pub async fn crawl_stream(
        &self,
        start_url: &str,
        progress_callback: Option<ProgressCallback>,
        cancel_token: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<mpsc::Receiver<CrawlResult>> {
        let (tx, rx) = mpsc::channel(10); // Buffer size of 10
        let max_pages = self.config.max_pages;
        let max_depth = self.config.max_depth;
        let delay_ms = self.config.delay_ms;
        let max_concurrency = self.config.max_concurrency;
        let scrape_mode = self.config.scrape_mode;
        let base_url = Url::parse(start_url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid start URL: {}", e)))?;
        let base_domain = base_url.host_str().unwrap_or("").to_string();

        info!(
            "Starting stream crawl from: {} (domain: {}, max_concurrency: {}, delay_ms: {}, mode: {:?})",
            start_url, base_domain, max_concurrency, delay_ms, scrape_mode
        );

        // Clone necessary data for the spawned task
        let fetcher = self.fetcher.clone();
        let browser_pool = self.browser_pool.clone();
        let include_patterns = self.config.include_patterns.clone();
        let exclude_patterns = self.config.exclude_patterns.clone();
        let start_url = start_url.to_string();
        let cancel_token = cancel_token.clone();

        // Create semaphore for rate limiting concurrent requests
        let semaphore = Arc::new(Semaphore::new(max_concurrency));

        // Spawn the crawl task
        tokio::spawn(async move {
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            let pages_count = Arc::new(AtomicUsize::new(0));

            queue.push_back((start_url.clone(), 0usize));
            visited.insert(start_url);

            // Send initial progress
            if let Some(ref callback) = progress_callback {
                callback(CrawlProgress {
                    pages_scraped: 0,
                    total_discovered: 1,
                    queue_length: 1,
                    current_url: None,
                    depth: 0,
                });
            }

            // For browser mode, pre-initialize the browser to avoid race conditions
            if scrape_mode == ScrapeMode::Browser {
                if let Err(e) = browser_pool.get_or_init() {
                    warn!("Failed to initialize browser: {}", e);
                    return;
                }
            }

            while let Some((url, depth)) = queue.pop_front() {
                // Check for cancellation at the start of each iteration
                if let Some(ref token) = cancel_token {
                    if token.is_cancelled() {
                        info!("[Crawl] Cancellation requested, stopping crawl");
                        break;
                    }
                }

                let current_count = pages_count.load(Ordering::Relaxed);

                // Log current URL and queue status
                info!(
                    "[Crawl] Processing URL: {} (depth: {}, queue: {}, visited: {}, pages: {})",
                    url,
                    depth,
                    queue.len(),
                    visited.len(),
                    current_count
                );

                // Check limits
                if current_count >= max_pages {
                    info!(
                        "Stream crawl reached max_pages limit ({}), stopping",
                        max_pages
                    );
                    break;
                }

                if depth > max_depth {
                    trace!(
                        "Skipping {}: depth {} exceeds max_depth {}",
                        url, depth, max_depth
                    );
                    continue;
                }

                // Check URL patterns
                if !Self::should_crawl_static(
                    &url,
                    &base_domain,
                    &include_patterns,
                    &exclude_patterns,
                ) {
                    trace!("Skipping {}: does not match crawl patterns", url);
                    continue;
                }

                // Add delay before acquiring semaphore permit (moved outside of permit scope)
                if delay_ms > 0 {
                    trace!("[Crawl] Waiting {}ms before fetching {}", delay_ms, url);
                    sleep(Duration::from_millis(delay_ms)).await;
                }

                // Acquire permit for concurrent rate limiting
                let permit = semaphore.clone().acquire_owned().await.ok();
                if permit.is_none() {
                    warn!("Failed to acquire semaphore permit for {}", url);
                    continue;
                }

                // Log before fetching
                info!("[Crawl] Fetching: {} (mode: {:?})", url, scrape_mode);

                // Fetch and process the page based on scrape mode
                // For browser mode, create a new TabFetcher for each request (no mutex!)
                let process_result = match scrape_mode {
                    ScrapeMode::Fetch => Self::process_page_static(&fetcher, &url, depth).await,
                    ScrapeMode::Browser => {
                        // No mutex! Each request gets its own TabFetcher
                        match browser_pool.create_fetcher() {
                            Ok(tab_fetcher) => {
                                Self::process_page_with_tab_fetcher(
                                    tab_fetcher,
                                    &url,
                                    depth,
                                    cancel_token.as_ref(),
                                )
                                .await
                            }
                            Err(e) => {
                                warn!("Failed to create tab fetcher: {}", e);
                                continue;
                            }
                        }
                    }
                };

                match process_result {
                    Ok((result, links)) => {
                        let new_count = pages_count.fetch_add(1, Ordering::Relaxed) + 1;

                        // Log successful fetch
                        info!(
                            "[Crawl] Successfully fetched: {} (title: {:?}, content_length: {}, links: {}, total_pages: {})",
                            result.url,
                            result.title.as_ref().map(|s| s.as_str()).unwrap_or("N/A"),
                            result.content.len(),
                            links.len(),
                            new_count
                        );

                        // Send result to channel
                        if tx.send(result).await.is_err() {
                            info!("Receiver dropped, stopping stream crawl");
                            break;
                        }

                        // Add new links to queue
                        let mut new_links = 0;
                        for link in links {
                            if !visited.contains(&link.url) && link.is_internal {
                                visited.insert(link.url.clone());
                                queue.push_back((link.url, depth + 1));
                                new_links += 1;
                            }
                        }
                        if new_links > 0 {
                            info!(
                                "[Crawl] Added {} new links to queue from {}",
                                new_links, url
                            );
                        }

                        // Send progress update
                        if let Some(ref callback) = progress_callback {
                            callback(CrawlProgress {
                                pages_scraped: new_count,
                                total_discovered: new_count + queue.len(),
                                queue_length: queue.len(),
                                current_url: Some(url),
                                depth,
                            });
                        }

                        // Log queue status periodically
                        if new_count % 10 == 0 {
                            info!(
                                "[Crawl] Progress: {}/{} pages, queue: {}, visited: {}",
                                new_count,
                                max_pages,
                                queue.len(),
                                visited.len()
                            );
                        }
                    }
                    Err(e) => {
                        warn!("[Crawl] Failed to process {}: {}", url, e);
                        // Continue with next URL even if this one failed
                    }
                }

                // Permit is dropped here, releasing the semaphore
                drop(permit);
            }

            let final_count = pages_count.load(Ordering::Relaxed);
            info!("Stream crawl complete: {} pages crawled", final_count);
            // Channel will be closed when tx is dropped
        });

        Ok(rx)
    }

    /// Static version of should_crawl for use in spawned task
    fn should_crawl_static(
        url: &str,
        base_domain: &str,
        include_patterns: &[Regex],
        exclude_patterns: &[Regex],
    ) -> bool {
        // Check if it's an internal link
        if let Ok(parsed) = Url::parse(url) {
            if parsed.host_str() != Some(base_domain) {
                return false;
            }
        }

        // Check exclude patterns
        for pattern in exclude_patterns {
            if pattern.is_match(url) {
                return false;
            }
        }

        // Check include patterns (if any)
        if !include_patterns.is_empty() {
            let mut matches = false;
            for pattern in include_patterns {
                if pattern.is_match(url) {
                    matches = true;
                    break;
                }
            }
            if !matches {
                return false;
            }
        }

        true
    }

    /// Static version of process_page for use in spawned task
    async fn process_page_static(
        fetcher: &Fetcher,
        url: &str,
        depth: usize,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        let fetch_result = fetcher.fetch(url).await?;

        // Stage 1: Extract main content and convert to Markdown using dom_smoothie + fast_html2md
        let article = HtmlToMarkdown::convert(&fetch_result.content, url)?;

        // Stage 2: Extract links from ORIGINAL HTML (not cleaned)
        let links = LinkExtractor::extract(&fetch_result.content, url);

        let result = CrawlResult {
            url: url.to_string(),
            title: Some(article.title),
            content: article.content,
            content_type: fetch_result.content_type,
            etag: fetch_result.etag,
            last_modified: fetch_result.last_modified,
            depth,
        };

        Ok((result, links))
    }

    /// Process page using TabFetcher for true concurrent browser operations.
    async fn process_page_with_tab_fetcher(
        tab_fetcher: crate::scraper::TabFetcher,
        url: &str,
        depth: usize,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        let fetch_result = tab_fetcher.fetch_with_cancel(url, cancel_token).await?;

        // Stage 1: Extract main content and convert to Markdown
        let article = HtmlToMarkdown::convert(&fetch_result.content, url)?;

        // Stage 2: Extract links from ORIGINAL HTML
        let links = LinkExtractor::extract(&fetch_result.content, url);

        let result = CrawlResult {
            url: url.to_string(),
            title: Some(article.title),
            content: article.content,
            content_type: fetch_result.content_type,
            etag: fetch_result.etag,
            last_modified: fetch_result.last_modified,
            depth,
        };

        Ok((result, links))
    }

    /// Process a single page.
    async fn process_page(
        &self,
        url: &str,
        depth: usize,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        debug!(
            "Processing page: {} (mode: {:?})",
            url, self.config.scrape_mode
        );

        let fetch_result = match self.config.scrape_mode {
            ScrapeMode::Fetch => {
                debug!("Using HTTP fetch for: {}", url);
                self.fetcher.fetch(url).await?
            }
            ScrapeMode::Browser => {
                debug!("Using browser fetch for: {}", url);
                let tab_fetcher = self.browser_pool.create_fetcher()?;
                tab_fetcher.fetch(url).await?
            }
        };

        debug!(
            "Fetched {}: status={}, content_length={}, content_type={:?}",
            url,
            fetch_result.status,
            fetch_result.content.len(),
            fetch_result.content_type
        );

        // Stage 1: Extract main content and convert to Markdown
        trace!("Converting HTML to markdown for: {}", url);
        let article = HtmlToMarkdown::convert(&fetch_result.content, url)?;
        debug!(
            "Converted {}: markdown_length={}",
            url,
            article.content.len()
        );

        // Stage 2: Extract links from ORIGINAL HTML
        trace!("Extracting links from: {}", url);
        let links = LinkExtractor::extract(&fetch_result.content, url);
        trace!("Extracted {} links from {}", links.len(), url);

        let result = CrawlResult {
            url: url.to_string(),
            title: Some(article.title),
            content: article.content,
            content_type: fetch_result.content_type,
            etag: fetch_result.etag,
            last_modified: fetch_result.last_modified,
            depth,
        };

        Ok((result, links))
    }

    /// Crawl a single page.
    pub async fn crawl_page(&self, url: &str) -> Result<CrawlResult> {
        let fetch_result = self.fetcher.fetch(url).await?;

        // Extract main content and convert to Markdown
        let article = HtmlToMarkdown::convert(&fetch_result.content, url)?;

        Ok(CrawlResult {
            url: url.to_string(),
            title: Some(article.title),
            content: article.content,
            content_type: fetch_result.content_type,
            etag: fetch_result.etag,
            last_modified: fetch_result.last_modified,
            depth: 0,
        })
    }

    /// Check if a URL should be crawled.
    fn should_crawl(&self, url: &str, base_domain: &str) -> bool {
        // Check if it's an internal link
        if let Ok(parsed) = Url::parse(url) {
            if parsed.host_str() != Some(base_domain) {
                trace!(
                    "Excluding {}: different domain (expected {})",
                    url, base_domain
                );
                return false;
            }
        }

        // Check exclude patterns
        for pattern in &self.config.exclude_patterns {
            if pattern.is_match(url) {
                trace!("Excluding {}: matches exclude pattern {}", url, pattern);
                return false;
            }
        }

        // Check include patterns (if any)
        if !self.config.include_patterns.is_empty() {
            let mut matches = false;
            for pattern in &self.config.include_patterns {
                if pattern.is_match(url) {
                    matches = true;
                    break;
                }
            }
            if !matches {
                trace!("Excluding {}: does not match any include pattern", url);
                return false;
            }
        }

        trace!("Including {} for crawling", url);
        true
    }

    /// Get the crawler configuration.
    pub fn config(&self) -> &CrawlConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawl_config_default() {
        let config = CrawlConfig::default();
        assert_eq!(config.max_pages, 1000);
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.max_concurrency, 5);
    }

    #[test]
    fn test_crawler_creation() {
        let crawler = Crawler::with_defaults();
        assert!(crawler.is_ok());
    }
}
