//! Content fetcher.

use crate::core::{Error, Result};
use crate::scraper::HttpClient;
use reqwest::header::{ETAG, LAST_MODIFIED, CONTENT_TYPE};
use reqwest::Response;
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
        // Validate URL
        let parsed_url = Url::parse(url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid URL {}: {}", url, e)))?;

        // Only allow http and https
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(Error::InvalidUrl(format!(
                "Unsupported URL scheme: {}",
                parsed_url.scheme()
            )));
        }

        // Build request with optional cache headers
        let mut request = self.client.inner().get(url);
        
        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = last_modified {
            request = request.header("If-Modified-Since", last_modified);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| Error::Http(format!("Failed to fetch {}: {}", url, e)))?;

        let status = response.status().as_u16();
        let final_url = response.url().to_string();

        // Handle 304 Not Modified
        if status == 304 {
            return Err(Error::NotFound(format!("Not modified: {}", url)));
        }

        // Handle error status codes
        if !response.status().is_success() {
            return Err(Error::Http(format!(
                "HTTP {} for {}",
                status, url
            )));
        }

        // Extract headers
        let etag = extract_header(&response, ETAG);
        let last_modified = extract_header(&response, LAST_MODIFIED);
        let content_type = extract_header(&response, CONTENT_TYPE);

        // Read body
        let content = response
            .text()
            .await
            .map_err(|e| Error::Http(format!("Failed to read response body: {}", e)))?;

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
        let fetcher = Fetcher::with_defaults();
        assert!(true); // Just check it doesn't panic
    }
}
