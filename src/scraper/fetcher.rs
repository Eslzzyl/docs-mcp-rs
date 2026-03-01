//! Content fetcher.

use crate::core::{Error, Result};
use crate::scraper::HttpClient;
use reqwest::Response;
use reqwest::header::{CONTENT_TYPE, ETAG, LAST_MODIFIED};
use tracing::{debug, trace, warn};
use url::Url;

/// Result of fetching a URL.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The fetched URL.
    pub url: String,
    /// The final URL after redirects.
    pub final_url: String,
    /// The content body.
    pub content: String,
    /// Content type (MIME type).
    pub content_type: Option<String>,
    /// ETag for caching.
    pub etag: Option<String>,
    /// Last-Modified header.
    pub last_modified: Option<String>,
    /// HTTP status code.
    pub status: u16,
}

/// Content fetcher.
pub struct Fetcher {
    client: HttpClient,
}

impl Fetcher {
    /// Create a new fetcher.
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    /// Create a fetcher with default HTTP client.
    pub fn with_defaults() -> Self {
        Self::new(HttpClient::default())
    }

    /// Fetch content from a URL.
    pub async fn fetch(&self, url: &str) -> Result<FetchResult> {
        self.fetch_with_cache_headers(url, None, None).await
    }

    /// Fetch content with cache headers for conditional requests.
    pub async fn fetch_with_cache_headers(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchResult> {
        debug!("Fetching URL: {} (etag: {:?}, last_modified: {:?})", url, etag, last_modified);

        // Validate URL
        let parsed_url = Url::parse(url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid URL {}: {}", url, e)))?;

        // Only allow http and https
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            warn!("Unsupported URL scheme for {}: {}", url, parsed_url.scheme());
            return Err(Error::InvalidUrl(format!(
                "Unsupported URL scheme: {}",
                parsed_url.scheme()
            )));
        }

        // Build request with optional cache headers
        let mut request = self.client.inner().get(url);

        if let Some(etag) = etag {
            trace!("Adding If-None-Match header: {}", etag);
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = last_modified {
            trace!("Adding If-Modified-Since header: {}", last_modified);
            request = request.header("If-Modified-Since", last_modified);
        }

        // Send request
        trace!("Sending HTTP GET request to: {}", url);
        let response = request
            .send()
            .await
            .map_err(|e| Error::Http(format!("Failed to fetch {}: {}", url, e)))?;

        let status = response.status().as_u16();
        let final_url = response.url().to_string();

        debug!("Received response for {}: status={}, final_url={}", url, status, final_url);

        // Handle 304 Not Modified
        if status == 304 {
            trace!("Content not modified (304) for: {}", url);
            return Err(Error::NotFound(format!("Not modified: {}", url)));
        }

        // Handle error status codes
        if !response.status().is_success() {
            warn!("HTTP error for {}: status={}", url, status);
            return Err(Error::Http(format!("HTTP {} for {}", status, url)));
        }

        // Extract headers
        let etag = extract_header(&response, ETAG);
        let last_modified = extract_header(&response, LAST_MODIFIED);
        let content_type = extract_header(&response, CONTENT_TYPE);

        trace!("Response headers for {}: etag={:?}, last_modified={:?}, content_type={:?}",
               url, etag, last_modified, content_type);

        // Read body
        trace!("Reading response body for: {}", url);
        let content = response
            .text()
            .await
            .map_err(|e| Error::Http(format!("Failed to read response body: {}", e)))?;

        debug!("Successfully fetched {}: content_length={}", url, content.len());

        Ok(FetchResult {
            url: url.to_string(),
            final_url,
            content,
            content_type,
            etag,
            last_modified,
            status,
        })
    }

    /// Check if a URL is accessible (HEAD request).
    pub async fn check_url(&self, url: &str) -> Result<bool> {
        let response = self.client.head(url).await?;
        Ok(response.status().is_success())
    }

    /// Get the HTTP client.
    pub fn client(&self) -> &HttpClient {
        &self.client
    }
}

/// Extract a header value from a response.
fn extract_header(response: &Response, header_name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        Fetcher::with_defaults();
        assert!(true); // Just check it doesn't panic
    }
}
