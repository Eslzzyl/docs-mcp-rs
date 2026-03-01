//! Embedding module for vector generation.

mod types;
mod openai;
mod google;

pub use types::{EmbeddingModel, EmbeddingResult, OPENAI_MODELS, GOOGLE_MODELS};
pub use openai::OpenAIEmbedder;
pub use google::GoogleEmbedder;

use crate::core::{Error, Result};
use crate::core::config::{EmbeddingConfig, EmbeddingProvider};
use async_trait::async_trait;

/// Trait for embedding providers.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Get the name of the embedding provider.
    fn name(&self) -> &str;
    
    /// Get the dimension of the embedding vectors.
    fn dimension(&self) -> usize;
    
    /// Embed a single text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Embed multiple texts in batch.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

/// Create an embedder based on configuration.
pub fn create_embedder(config: &EmbeddingConfig) -> Result<Box<dyn Embedder>> {
    match config.provider {
        EmbeddingProvider::OpenAI => {
            let api_key = config.openai_api_key.clone()
                .ok_or_else(|| Error::Embedding("OpenAI API key not configured".to_string()))?;
            
            Ok(Box::new(OpenAIEmbedder::new(
                api_key,
                config.openai_model.clone(),
                config.dimension,
            )?))
        }
        EmbeddingProvider::Google => {
            let api_key = config.google_api_key.clone()
                .ok_or_else(|| Error::Embedding("Google API key not configured".to_string()))?;
            
            Ok(Box::new(GoogleEmbedder::new(
                api_key,
                config.google_model.clone(),
                config.dimension,
            )?))
        }
    }
}
