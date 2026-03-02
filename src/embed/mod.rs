//! Embedding module for vector generation.

pub mod rate_limiter;

mod google;
mod none;
mod openai;
mod queue;
mod types;

pub use google::GoogleEmbedder;
pub use none::NoneEmbedder;
pub use openai::OpenAIEmbedder;
pub use queue::{EmbeddingQueue, create_queued_embedder};
pub use rate_limiter::{RateLimiter, RateLimiterStats, SharedRateLimiter, create_rate_limiter};
pub use types::{EmbeddingModel, EmbeddingResult, GOOGLE_MODELS, OPENAI_MODELS};

use crate::core::Result;
use crate::core::config::{EmbeddingConfig, EmbeddingProvider};
use async_trait::async_trait;

/// Trait for embedding providers.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Get the name of the embedding provider.
    fn name(&self) -> &str;

    /// Get the dimension of the embedding vectors.
    fn dimension(&self) -> usize;

    /// Check if this embedder is available (has valid configuration).
    fn is_available(&self) -> bool {
        true
    }

    /// Embed a single text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Embed multiple texts in batch.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

/// Create an embedder based on configuration.
/// Returns NoneEmbedder if no API key is configured.
///
/// This creates an EmbeddingQueue that wraps the actual embedder to ensure
/// all embedding requests are serialized, preventing concurrent API calls
/// that could trigger rate limiting.
pub fn create_embedder(config: &EmbeddingConfig) -> Result<Box<dyn Embedder>> {
    // Create rate limiter if limits are configured
    let rate_limiter = if config.max_rpm > 0 && config.max_tpm > 0 {
        Some(create_rate_limiter(
            config.max_rpm,
            config.max_tpm,
            config.request_delay_ms,
        ))
    } else {
        None
    };

    // Create the base embedder
    let base_embedder: Box<dyn Embedder> = match config.provider {
        EmbeddingProvider::OpenAI => match &config.openai_api_key {
            Some(api_key) if !api_key.is_empty() => {
                let embedder = if let Some(base_url) = &config.openai_api_base {
                    OpenAIEmbedder::with_base_url(
                        api_key.clone(),
                        base_url.clone(),
                        config.openai_model.clone(),
                        config.dimension,
                        None, // Rate limiting is handled by the queue
                        0,    // Retries are handled by the queue
                        0,
                    )?
                } else {
                    OpenAIEmbedder::new(
                        api_key.clone(),
                        config.openai_model.clone(),
                        config.dimension,
                        None, // Rate limiting is handled by the queue
                        0,    // Retries are handled by the queue
                        0,
                    )?
                };
                Box::new(embedder)
            }
            _ => {
                tracing::warn!("OpenAI API key not configured, using fallback (FTS-only search)");
                Box::new(NoneEmbedder::new())
            }
        },
        EmbeddingProvider::Google => match &config.google_api_key {
            Some(api_key) if !api_key.is_empty() => {
                let embedder = if let Some(base_url) = &config.google_api_base {
                    GoogleEmbedder::with_base_url(
                        api_key.clone(),
                        config.google_model.clone(),
                        config.dimension,
                        base_url.clone(),
                        None, // Rate limiting is handled by the queue
                        0,    // Retries are handled by the queue
                        0,
                    )?
                } else {
                    GoogleEmbedder::new(
                        api_key.clone(),
                        config.google_model.clone(),
                        config.dimension,
                        None, // Rate limiting is handled by the queue
                        0,    // Retries are handled by the queue
                        0,
                    )?
                };
                Box::new(embedder)
            }
            _ => {
                tracing::warn!("Google API key not configured, using fallback (FTS-only search)");
                Box::new(NoneEmbedder::new())
            }
        },
    };

    // Wrap with EmbeddingQueue to serialize all requests
    // The queue handles rate limiting and retries
    let queued_embedder = EmbeddingQueue::new(
        base_embedder,
        rate_limiter,
        config.max_retries,
        config.retry_base_delay_ms,
    );

    Ok(Box::new(queued_embedder))
}
