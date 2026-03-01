//! Web crawler for documentation sites.

use crate::core::{Error, Result, ScraperOptions};
use crate::scraper::{Fetcher, HtmlParser, HtmlToMarkdown};
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Semaphore, mpsc};
use tokio::time::sleep;
use tracing::{debug, info, trace, warn};
use url::Url;

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

        config
    }
}

/// Web crawler.
pub struct Crawler {
    config: CrawlConfig,
    fetcher: Arc<Fetcher>,
}

impl Crawler {
    /// Create a new crawler with the given configuration.
    pub fn new(config: CrawlConfig) -> Result<Self> {
        let fetcher = Arc::new(Fetcher::new(crate::scraper::HttpClient::new(
            &config.user_agent,
            config.timeout_secs,
        )?));

        Ok(Self { config, fetcher })
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

        debug!("Starting crawl from: {} (domain: {})", start_url, base_domain);
        debug!("Crawl config: max_pages={}, max_depth={}, delay_ms={}",
               self.config.max_pages, self.config.max_depth, self.config.delay_ms);

        queue.push_back((start_url.to_string(), 0usize));
        visited.insert(start_url.to_string());

        let parser = HtmlParser::new();
        let converter = HtmlToMarkdown::new();

        while let Some((url, depth)) = queue.pop_front() {
            let current_count = pages_count.load(Ordering::Relaxed);
            debug!("Processing URL: {} (depth: {}, queue: {}, visited: {}, pages: {})",
                   url, depth, queue.len(), visited.len(), current_count);

            // Check limits
            if current_count >= self.config.max_pages {
                debug!("Reached max_pages limit ({}), stopping crawl", self.config.max_pages);
                break;
            }

            if depth > self.config.max_depth {
                trace!("Skipping {}: depth {} exceeds max_depth {}", url, depth, self.config.max_depth);
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
            match self.process_page(&url, depth, &parser, &converter).await {
                Ok((result, links)) => {
                    debug!("Successfully processed {}: title='{:?}', content_length={}, links={}",
                           url, result.title, result.content.len(), links.len());
                    results.push(result);
                    let new_count = pages_count.fetch_add(1, Ordering::Relaxed) + 1;

                    // Add new links to queue
                    let mut new_links = 0;
                    for link in links {
                        trace!("Found link: {} (internal: {}, text: '{}')",
                               link.url, link.is_internal, link.text);
                        if !visited.contains(&link.url) && link.is_internal {
                            visited.insert(link.url.clone());
                            queue.push_back((link.url, depth + 1));
                            new_links += 1;
                        }
                    }
                    debug!("Added {} new links to queue from {}", new_links, url);
                    trace!("Progress: {}/{} pages, queue: {}", new_count, self.config.max_pages, queue.len());
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
    pub async fn crawl_stream(&self, start_url: &str) -> Result<mpsc::Receiver<CrawlResult>> {
        let (tx, rx) = mpsc::channel(10); // Buffer size of 10
        let max_pages = self.config.max_pages;
        let max_depth = self.config.max_depth;
        let delay_ms = self.config.delay_ms;
        let max_concurrency = self.config.max_concurrency;
        let base_url = Url::parse(start_url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid start URL: {}", e)))?;
        let base_domain = base_url.host_str().unwrap_or("").to_string();

        info!("Starting stream crawl from: {} (domain: {}, max_concurrency: {}, delay_ms: {})",
              start_url, base_domain, max_concurrency, delay_ms);

        // Clone necessary data for the spawned task
        let fetcher = self.fetcher.clone();
        let include_patterns = self.config.include_patterns.clone();
        let exclude_patterns = self.config.exclude_patterns.clone();
        let start_url = start_url.to_string();

        // Create semaphore for rate limiting concurrent requests
        let semaphore = Arc::new(Semaphore::new(max_concurrency));

        // Spawn the crawl task
        tokio::spawn(async move {
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            let pages_count = Arc::new(AtomicUsize::new(0));

            queue.push_back((start_url.clone(), 0usize));
            visited.insert(start_url);

            let parser = HtmlParser::new();
            let converter = HtmlToMarkdown::new();

            while let Some((url, depth)) = queue.pop_front() {
                let current_count = pages_count.load(Ordering::Relaxed);

                // Log current URL and queue status
                info!("[Crawl] Processing URL: {} (depth: {}, queue: {}, visited: {}, pages: {})",
                      url, depth, queue.len(), visited.len(), current_count);

                // Check limits
                if current_count >= max_pages {
                    info!("Stream crawl reached max_pages limit ({}), stopping", max_pages);
                    break;
                }

                if depth > max_depth {
                    trace!("Skipping {}: depth {} exceeds max_depth {}", url, depth, max_depth);
                    continue;
                }

                // Check URL patterns
                if !Self::should_crawl_static(&url, &base_domain, &include_patterns, &exclude_patterns) {
                    trace!("Skipping {}: does not match crawl patterns", url);
                    continue;
                }

                // Acquire permit for concurrent rate limiting
                let permit = semaphore.clone().acquire_owned().await.ok();
                if permit.is_none() {
                    warn!("Failed to acquire semaphore permit for {}", url);
                    continue;
                }

                // Add delay between requests
                if delay_ms > 0 {
                    trace!("[Crawl] Waiting {}ms before fetching {}", delay_ms, url);
                    sleep(Duration::from_millis(delay_ms)).await;
                }

                // Log before fetching
                info!("[Crawl] Fetching: {}", url);

                // Fetch and process the page
                match Self::process_page_static(&fetcher, &url, depth, &parser, &converter).await {
                    Ok((result, links)) => {
                        let new_count = pages_count.fetch_add(1, Ordering::Relaxed) + 1;

                        // Log successful fetch
                        info!("[Crawl] Successfully fetched: {} (title: {:?}, content_length: {}, links: {}, total_pages: {})",
                              result.url, result.title.as_ref().map(|s| s.as_str()).unwrap_or("N/A"),
                              result.content.len(), links.len(), new_count);

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
                            info!("[Crawl] Added {} new links to queue from {}", new_links, url);
                        }

                        // Log queue status periodically
                        if new_count % 10 == 0 {
                            info!("[Crawl] Progress: {}/{} pages, queue: {}, visited: {}",
                                  new_count, max_pages, queue.len(), visited.len());
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
        parser: &HtmlParser,
        converter: &HtmlToMarkdown,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        let fetch_result = fetcher.fetch(url).await?;

        // Parse HTML (synchronous operations)
        let doc = parser.parse(&fetch_result.content);
        let title = parser.extract_title(&doc);
        let markdown = converter.convert(&fetch_result.content)?;
        let links = parser.extract_links(&doc, url)?;

        let result = CrawlResult {
            url: url.to_string(),
            title,
            content: markdown,
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
        parser: &HtmlParser,
        converter: &HtmlToMarkdown,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        debug!("Fetching: {}", url);
        let fetch_result = self.fetcher.fetch(url).await?;

        debug!("Fetched {}: status={}, content_length={}, content_type={:?}",
               url, fetch_result.status, fetch_result.content.len(), fetch_result.content_type);

        // Parse HTML (synchronous operations)
        trace!("Parsing HTML for: {}", url);
        let doc = parser.parse(&fetch_result.content);

        trace!("Extracting title for: {}", url);
        let title = parser.extract_title(&doc);

        trace!("Converting HTML to markdown for: {}", url);
        let markdown = converter.convert(&fetch_result.content)?;
        debug!("Converted {}: markdown_length={}", url, markdown.len());

        trace!("Extracting links from: {}", url);
        let links = parser.extract_links(&doc, url)?;
        trace!("Extracted {} links from {}", links.len(), url);

        let result = CrawlResult {
            url: url.to_string(),
            title,
            content: markdown,
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

        let parser = HtmlParser::new();
        let doc = parser.parse(&fetch_result.content);
        let title = parser.extract_title(&doc);
        let converter = HtmlToMarkdown::new();
        let markdown = converter.convert(&fetch_result.content)?;

        Ok(CrawlResult {
            url: url.to_string(),
            title,
            content: markdown,
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
                trace!("Excluding {}: different domain (expected {})", url, base_domain);
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
