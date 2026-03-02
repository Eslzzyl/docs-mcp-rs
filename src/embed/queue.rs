//! Embedding request queue for serializing API calls.
//!
//! This module provides a queue-based embedding system that ensures all embedding
//! requests are processed serially, preventing concurrent API calls that could
//! trigger rate limiting.

use crate::core::{Error, Result};
use crate::embed::{
    rate_limiter::{estimate_batch_tokens, SharedRateLimiter},
    Embedder,
};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, trace, warn};

/// Request to be processed by the embedding worker.
struct EmbeddingRequest {
    /// Texts to embed
    texts: Vec<String>,
    /// Channel to send the result back
    response_tx: oneshot::Sender<Result<Vec<Vec<f32>>>>,
}

/// A queue-based embedder that serializes all embedding requests.
///
/// This ensures that only one embedding API call is in flight at any time,
/// which makes rate limiting effective and prevents 429 errors.
pub struct EmbeddingQueue {
    /// Sender channel for submitting requests
    sender: mpsc::UnboundedSender<EmbeddingRequest>,
}

impl EmbeddingQueue {
    /// Create a new embedding queue with the given embedder and rate limiter.
    ///
    /// This spawns a background worker task that will process requests serially.
    pub fn new(
        embedder: Box<dyn Embedder>,
        rate_limiter: Option<SharedRateLimiter>,
        max_retries: u32,
        retry_base_delay_ms: u64,
    ) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<EmbeddingRequest>();

        // Spawn the worker task
        tokio::spawn(async move {
            info!(
                "EmbeddingQueue worker started (max_retries={}, retry_base_delay={}ms)",
                max_retries, retry_base_delay_ms
            );

            let mut request_count = 0u64;

            while let Some(request) = receiver.recv().await {
                request_count += 1;
                trace!(
                    "Processing embedding request #{} ({} texts)",
                    request_count,
                    request.texts.len()
                );

                // Apply rate limiting before making the request
                if let Some(ref limiter) = rate_limiter {
                    let texts_refs: Vec<&str> = request.texts.iter().map(|s| s.as_str()).collect();
                    let estimated_tokens = estimate_batch_tokens(&texts_refs);
                    trace!(
                        "Request #{}: acquiring rate limit for {} tokens",
                        request_count,
                        estimated_tokens
                    );
                    limiter.lock().await.acquire(estimated_tokens).await;
                }

                // Make the embedding request with retry logic
                let texts_refs: Vec<&str> = request.texts.iter().map(|s| s.as_str()).collect();
                let result = Self::embed_with_retry(
                    embedder.as_ref(),
                    &texts_refs,
                    max_retries,
                    retry_base_delay_ms,
                )
                .await;

                // Send result back
                if let Err(ref e) = result {
                    warn!("Embedding request #{} failed: {}", request_count, e);
                }

                if request.response_tx.send(result).is_err() {
                    error!("Failed to send embedding result - receiver dropped");
                }
            }

            info!("EmbeddingQueue worker stopped (processed {} requests)", request_count);
        });

        Self { sender }
    }

    /// Make an embedding request with retry logic for 429 errors.
    async fn embed_with_retry(
        embedder: &dyn Embedder,
        texts: &[&str],
        max_retries: u32,
        base_delay_ms: u64,
    ) -> Result<Vec<Vec<f32>>> {
        let mut retries = 0;

        loop {
            match embedder.embed_batch(texts).await {
                Ok(embeddings) => return Ok(embeddings),
                Err(e) => {
                    let error_msg = e.to_string();

                    // Check if this is a 429 error
                    if error_msg.contains("429") && retries < max_retries {
                        retries += 1;
                        let delay_ms = base_delay_ms * (1 << (retries - 1)); // Exponential backoff
                        warn!(
                            "Embedding API rate limited (429), retrying in {}ms (attempt {}/{})",
                            delay_ms, retries, max_retries
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }

                    // Not a 429 or max retries exceeded
                    return Err(e);
                }
            }
        }
    }

    /// Embed a batch of texts.
    ///
    /// This method sends the request to the worker queue and waits for the result.
    /// All requests are processed serially by the worker.
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = EmbeddingRequest {
            texts: texts.iter().map(|s| s.to_string()).collect(),
            response_tx,
        };

        // Send request to the worker
        self.sender
            .send(request)
            .map_err(|_| Error::Embedding("Embedding queue closed".to_string()))?;

        // Wait for the result
        response_rx
            .await
            .map_err(|_| Error::Embedding("Embedding worker dropped".to_string()))?
    }

    /// Embed a single text.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Error::Embedding("No embedding returned".to_string()))
    }
}

/// Create a queued embedder that wraps a regular embedder with rate limiting.
///
/// This is a convenience function that creates an EmbeddingQueue with the given
/// configuration.
pub fn create_queued_embedder(
    embedder: Box<dyn Embedder>,
    rate_limiter: Option<SharedRateLimiter>,
    max_retries: u32,
    retry_base_delay_ms: u64,
) -> EmbeddingQueue {
    EmbeddingQueue::new(embedder, rate_limiter, max_retries, retry_base_delay_ms)
}

#[async_trait]
impl Embedder for EmbeddingQueue {
    fn name(&self) -> &str {
        "queued"
    }

    fn dimension(&self) -> usize {
        // Return a default dimension - the actual embedder's dimension
        // will be used when processing
        1536
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text).await
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::none::NoneEmbedder;

    #[tokio::test]
    async fn test_embedding_queue_basic() {
        let embedder = NoneEmbedder::new();
        let queue = EmbeddingQueue::new(Box::new(embedder), None, 3, 1000);

        let texts = vec!["Hello world", "Test text"];
        let result = queue.embed_batch(&texts).await;

        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
    }

    #[tokio::test]
    async fn test_embedding_queue_single() {
        let embedder = NoneEmbedder::new();
        let queue = EmbeddingQueue::new(Box::new(embedder), None, 3, 1000);

        let result = queue.embed("Hello").await;

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
    }
}
