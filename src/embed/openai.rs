//! OpenAI embedding provider.

use crate::core::{Error, Result};
use crate::embed::{
    rate_limiter::{estimate_batch_tokens, SharedRateLimiter},
    Embedder, EmbeddingModel, OPENAI_MODELS,
};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{trace, warn};

/// Maximum batch size for embedding requests.
/// SiliconFlow API has a limit of 64 items per batch.
const MAX_BATCH_SIZE: usize = 64;

/// OpenAI embedding provider.
pub struct OpenAIEmbedder {
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

impl OpenAIEmbedder {
    /// Create a new OpenAI embedder.
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
            base_url: "https://api.openai.com/v1".to_string(),
            rate_limiter,
            max_retries,
            retry_base_delay_ms,
        })
    }

    /// Create with a custom base URL (for Azure or custom endpoints).
    pub fn with_base_url(
        api_key: String,
        base_url: String,
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
        let (dim, max_tokens) = OPENAI_MODELS
            .iter()
            .find(|(id, _, _)| *id == self.model)
            .map(|(_, d, t)| (*d, *t))
            .unwrap_or((self.dimension, 8191));

        EmbeddingModel {
            id: self.model.clone(),
            dimension: dim,
            max_tokens,
        }
    }

    /// Get the API URL for embeddings.
    fn get_embed_url(&self) -> String {
        format!("{}/embeddings", self.base_url)
    }

    /// Embed a single chunk of texts (max MAX_BATCH_SIZE items).
    async fn embed_batch_chunk(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Apply rate limiting before making the request
        if let Some(limiter) = &self.rate_limiter {
            let estimated_tokens = estimate_batch_tokens(texts);
            trace!("Acquiring rate limit for {} tokens", estimated_tokens);
            limiter.lock().await.acquire(estimated_tokens).await;
        }

        let input = if texts.len() == 1 {
            EmbeddingInput::Single(texts[0].to_string())
        } else {
            EmbeddingInput::Multiple(texts.iter().map(|s| s.to_string()).collect())
        };

        let request = EmbeddingRequest {
            model: self.model.clone(),
            input,
            dimensions: Some(self.dimension),
        };

        // Retry loop for 429 errors
        let mut retries = 0;
        loop {
            let response = self
                .client
                .post(self.get_embed_url())
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Http(format!("OpenAI API request failed: {}", e)))?;

            let status = response.status();

            if status.is_success() {
                let result: EmbeddingResponse = response
                    .json()
                    .await
                    .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

                // Sort by index to maintain order
                let mut embeddings: Vec<(i32, Vec<f32>)> = result
                    .data
                    .into_iter()
                    .map(|e| {
                        let embedding = e.embedding.into_iter().map(|f| f as f32).collect();
                        (e.index, embedding)
                    })
                    .collect();

                embeddings.sort_by_key(|(i, _)| *i);

                // Pad or truncate to expected dimension
                let result: Vec<Vec<f32>> = embeddings
                    .into_iter()
                    .map(|(_, mut emb)| {
                        emb.resize(self.dimension, 0.0);
                        emb
                    })
                    .collect();

                return Ok(result);
            }

            // Handle 429 rate limit errors with retry
            if status.as_u16() == 429 && retries < self.max_retries {
                retries += 1;
                let delay_ms = self.retry_base_delay_ms * (1 << (retries - 1)); // Exponential backoff: 1s, 2s, 4s
                warn!(
                    "OpenAI API rate limited (429), retrying in {}ms (attempt {}/{})",
                    delay_ms, retries, self.max_retries
                );
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            // For other errors or max retries exceeded, return the error
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(format!(
                "OpenAI API error ({}): {}",
                status, body
            )));
        }
    }
}

/// Request body for OpenAI embeddings.
#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: EmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

/// Input for embedding request.
#[derive(Serialize)]
#[serde(untagged)]
enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

/// Response from OpenAI embeddings API.
#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)]
    usage: Option<Usage>,
}

/// Embedding data from response.
#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f64>,
    index: i32,
}

/// Usage information.
#[derive(Deserialize)]
struct Usage {
    #[allow(dead_code)]
    total_tokens: i32,
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    fn name(&self) -> &str {
        "openai"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Error::Embedding("No embedding returned".to_string()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // If batch is small enough, process directly
        if texts.len() <= MAX_BATCH_SIZE {
            return self.embed_batch_chunk(texts).await;
        }

        // Split large batches into smaller chunks
        let mut all_embeddings = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(MAX_BATCH_SIZE) {
            let chunk_embeddings = self.embed_batch_chunk(chunk).await?;
            all_embeddings.extend(chunk_embeddings);
        }

        Ok(all_embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info() {
        let embedder = OpenAIEmbedder::new(
            "test-key".to_string(),
            "text-embedding-3-small".to_string(),
            1536,
            None,    // no rate limiter
            3,       // max retries
            1000,    // retry base delay
        )
        .unwrap();

        let info = embedder.model_info();
        assert_eq!(info.id, "text-embedding-3-small");
        assert_eq!(info.dimension, 1536);
    }
}