//! Google (Gemini) embedding provider.

use crate::core::{Error, Result};
use crate::embed::{
    rate_limiter::{estimate_batch_tokens, SharedRateLimiter},
    Embedder, EmbeddingModel, GOOGLE_MODELS,
};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{trace, warn};

/// Google embedding provider.
pub struct GoogleEmbedder {
    client: HttpClient,
    api_key: String,
    model: String,
    dimension: usize,
    base_url: String,
    /// Rate limiter for controlling API request rates
    rate_limiter: Option<SharedRateLimiter>,
    /// Maximum retries for 429 errors
    max_retries: u32,
    /// Base delay for retries in milliseconds
    retry_base_delay_ms: u64,
}

impl GoogleEmbedder {
    /// Create a new Google embedder.
    pub fn new(
        api_key: String,
        model: String,
        dimension: usize,
        rate_limiter: Option<SharedRateLimiter>,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Result<Self> {
        let client = HttpClient::new();

        Ok(Self {
            client,
            api_key,
            model,
            dimension,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            rate_limiter,
            max_retries,
            retry_base_delay_ms,
        })
    }

    /// Create with custom base URL.
    pub fn with_base_url(
        api_key: String,
        model: String,
        dimension: usize,
        base_url: String,
        rate_limiter: Option<SharedRateLimiter>,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Result<Self> {
        let client = HttpClient::new();

        Ok(Self {
            client,
            api_key,
            model,
            dimension,
            base_url,
            rate_limiter,
            max_retries,
            retry_base_delay_ms,
        })
    }

    /// Set the rate limiter after construction.
    pub fn with_rate_limiter(mut self, rate_limiter: SharedRateLimiter) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Get model info.
    pub fn model_info(&self) -> EmbeddingModel {
        let (dim, max_tokens) = GOOGLE_MODELS
            .iter()
            .find(|(id, _, _)| *id == self.model)
            .map(|(_, d, t)| (*d, *t))
            .unwrap_or((self.dimension, 2048));

        EmbeddingModel {
            id: self.model.clone(),
            dimension: dim,
            max_tokens,
        }
    }

    /// Get the API URL for embedding.
    fn get_embed_url(&self) -> String {
        format!(
            "{}/models/{}:embedContent?key={}",
            self.base_url, self.model, self.api_key
        )
    }

    /// Get the API URL for batch embedding.
    fn get_batch_embed_url(&self) -> String {
        format!(
            "{}/models/{}:batchEmbedContents?key={}",
            self.base_url, self.model, self.api_key
        )
    }
}

/// Request body for single embedding.
#[derive(Serialize)]
struct EmbedRequest {
    content: Content,
}

/// Request body for batch embedding.
#[derive(Serialize)]
struct BatchEmbedRequest {
    requests: Vec<EmbedRequest>,
}

/// Content for embedding.
#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

/// Part of content.
#[derive(Serialize)]
struct Part {
    text: String,
}

/// Response for single embedding.
#[derive(Deserialize)]
struct EmbedResponse {
    embedding: EmbeddingValue,
}

/// Response for batch embedding.
#[derive(Deserialize)]
struct BatchEmbedResponse {
    embeddings: Vec<EmbeddingValue>,
}

/// Embedding value.
#[derive(Deserialize)]
struct EmbeddingValue {
    values: Vec<f32>,
}

#[async_trait]
impl Embedder for GoogleEmbedder {
    fn name(&self) -> &str {
        "google"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Apply rate limiting before making the request
        if let Some(limiter) = &self.rate_limiter {
            let estimated_tokens = estimate_batch_tokens(&[text]);
            trace!("Acquiring rate limit for {} tokens", estimated_tokens);
            limiter.lock().await.acquire(estimated_tokens).await;
        }

        let request = EmbedRequest {
            content: Content {
                parts: vec![Part {
                    text: text.to_string(),
                }],
            },
        };

        // Retry loop for 429 errors
        let mut retries = 0;
        loop {
            let response = self
                .client
                .post(self.get_embed_url())
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Http(format!("Google API request failed: {}", e)))?;

            let status = response.status();

            if status.is_success() {
                let result: EmbedResponse = response
                    .json()
                    .await
                    .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

                let mut embedding = result.embedding.values;
                embedding.resize(self.dimension, 0.0);

                return Ok(embedding);
            }

            // Handle 429 rate limit errors with retry
            if status.as_u16() == 429 && retries < self.max_retries {
                retries += 1;
                let delay_ms = self.retry_base_delay_ms * (1 << (retries - 1)); // Exponential backoff
                warn!(
                    "Google API rate limited (429), retrying in {}ms (attempt {}/{})",
                    delay_ms, retries, self.max_retries
                );
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            let body = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(format!(
                "Google API error ({}): {}",
                status, body
            )));
        }
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Google's batch API has a limit, so we process in smaller batches
        let batch_size = 100;
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(batch_size) {
            // Apply rate limiting before making the request
            if let Some(limiter) = &self.rate_limiter {
                let estimated_tokens = estimate_batch_tokens(chunk);
                trace!("Acquiring rate limit for {} tokens", estimated_tokens);
                limiter.lock().await.acquire(estimated_tokens).await;
            }

            let requests: Vec<EmbedRequest> = chunk
                .iter()
                .map(|text| EmbedRequest {
                    content: Content {
                        parts: vec![Part {
                            text: text.to_string(),
                        }],
                    },
                })
                .collect();

            let request = BatchEmbedRequest { requests };

            // Retry loop for 429 errors
            let mut retries = 0;
            loop {
                let response = self
                    .client
                    .post(self.get_batch_embed_url())
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| Error::Http(format!("Google API request failed: {}", e)))?;

                let status = response.status();

                if status.is_success() {
                    let result: BatchEmbedResponse = response
                        .json()
                        .await
                        .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

                    for mut emb in result.embeddings.into_iter().map(|e| e.values) {
                        emb.resize(self.dimension, 0.0);
                        all_embeddings.push(emb);
                    }

                    break;
                }

                // Handle 429 rate limit errors with retry
                if status.as_u16() == 429 && retries < self.max_retries {
                    retries += 1;
                    let delay_ms = self.retry_base_delay_ms * (1 << (retries - 1)); // Exponential backoff
                    warn!(
                        "Google API rate limited (429), retrying in {}ms (attempt {}/{})",
                        delay_ms, retries, self.max_retries
                    );
                    sleep(Duration::from_millis(delay_ms)).await;
                    continue;
                }

                let body = response.text().await.unwrap_or_default();
                return Err(Error::Embedding(format!(
                    "Google API error ({}): {}",
                    status, body
                )));
            }
        }

        Ok(all_embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info() {
        let embedder = GoogleEmbedder::new(
            "test-key".to_string(),
            "text-embedding-004".to_string(),
            768,
            None,    // no rate limiter
            3,       // max retries
            1000,    // retry base delay
        )
        .unwrap();

        let info = embedder.model_info();
        assert_eq!(info.id, "text-embedding-004");
        assert_eq!(info.dimension, 768);
    }
}
