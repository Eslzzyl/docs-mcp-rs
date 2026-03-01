//! Web crawler for documentation sites.

use crate::core::{Error, Result, ScraperOptions};
use crate::scraper::{Fetcher, HtmlParser, HtmlToMarkdown};
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
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
            config.include_patterns = include
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect();
        }
        if let Some(exclude) = options.exclude_patterns {
            config.exclude_patterns = exclude
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect();
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
        let fetcher = Arc::new(Fetcher::new(
            crate::scraper::HttpClient::new(&config.user_agent, config.timeout_secs)?
        ));
        
        Ok(Self {
            config,
            fetcher,
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
        
        queue.push_back((start_url.to_string(), 0usize));
        visited.insert(start_url.to_string());
        
        let parser = HtmlParser::new();
        let converter = HtmlToMarkdown::new();
        
        while let Some((url, depth)) = queue.pop_front() {
            // Check limits
            if pages_count.load(Ordering::Relaxed) >= self.config.max_pages {
                break;
            }
            
            if depth > self.config.max_depth {
                continue;
            }
            
            // Check URL patterns
            if !self.should_crawl(&url, &base_domain) {
                continue;
            }
            
            // Add delay between requests
            if self.config.delay_ms > 0 {
                sleep(Duration::from_millis(self.config.delay_ms)).await;
            }
            
            // Fetch and process the page
            match self.process_page(&url, depth, &parser, &converter).await {
                Ok((result, links)) => {
                    results.push(result);
                    pages_count.fetch_add(1, Ordering::Relaxed);
                    
                    // Add new links to queue
                    for link in links {
                        if !visited.contains(&link.url) && link.is_internal {
                            visited.insert(link.url.clone());
                            queue.push_back((link.url, depth + 1));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to process {}: {}", url, e);
                }
            }
        }
        
        // Sort by depth, then by URL
        results.sort_by(|a, b| {
            a.depth.cmp(&b.depth).then_with(|| a.url.cmp(&b.url))
        });
        
        Ok(results)
    }

    /// Process a single page.
    async fn process_page(
        &self,
        url: &str,
        depth: usize,
        parser: &HtmlParser,
        converter: &HtmlToMarkdown,
    ) -> Result<(CrawlResult, Vec<crate::scraper::Link>)> {
        let fetch_result = self.fetcher.fetch(url).await?;
        
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
                return false;
            }
        }
        
        // Check exclude patterns
        for pattern in &self.config.exclude_patterns {
            if pattern.is_match(url) {
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
                return false;
            }
        }
        
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