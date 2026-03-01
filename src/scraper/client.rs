//! HTTP client with retry support.

use crate::core::{Error, Result};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::time::Duration;

/// HTTP client wrapper with retry support.
#[derive(Clone)]
pub struct HttpClient {
    client: ClientWithMiddleware,
    user_agent: String,
}

impl HttpClient {
    /// Create a new HTTP client with default configuration.
    pub fn new(user_agent: &str, timeout_secs: u64) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(user_agent)
                .map_err(|e| Error::Http(format!("Invalid user agent: {}", e)))?,
        );

        let reqwest_client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| Error::Http(format!("Failed to create HTTP client: {}", e)))?;

        // Retry policy: exponential backoff with max 3 retries
        let retry_policy = ExponentialBackoff::builder()
            .base(2)
            .build_with_max_retries(3);

        let client = ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            client,
            user_agent: user_agent.to_string(),
        })
    }

    /// Get the underlying client.
    pub fn inner(&self) -> &ClientWithMiddleware {
        &self.client
    }

    /// Get the user agent string.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Perform a GET request.
    pub async fn get(&self, url: &str) -> Result<reqwest::Response> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Request failed for {}: {}", url, e)))
    }

    /// Perform a GET request with custom headers.
    pub async fn get_with_headers(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<reqwest::Response> {
        self.client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Request failed for {}: {}", url, e)))
    }

    /// Perform a HEAD request (for checking if a URL exists).
    pub async fn head(&self, url: &str) -> Result<reqwest::Response> {
        self.client
            .head(url)
            .send()
            .await
            .map_err(|e| Error::Http(format!("HEAD request failed for {}: {}", url, e)))
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(
            &format!("docs-mcp-rs/{}", env!("CARGO_PKG_VERSION")),
            30,
        ).expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HttpClient::new("test-agent/1.0", 60);
        assert!(client.is_ok());
    }
}
