//! No-op embedder for fallback when no API key is configured.

use crate::core::{Error, Result};
use async_trait::async_trait;
use crate::embed::Embedder;

/// A no-op embedder that returns an error when embedding is attempted.
/// This is used as a fallback when no embedding API key is configured.
pub struct NoneEmbedder;

impl NoneEmbedder {
    /// Create a new NoneEmbedder.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoneEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embedder for NoneEmbedder {
    fn name(&self) -> &str {
        "none"
    }

    fn dimension(&self) -> usize {
        0 // No dimension
    }

    fn is_available(&self) -> bool {
        false // Not available - will trigger FTS-only search
    }

    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(Error::Embedding(
            "Embedding not available: no API key configured. Use FTS-only search instead.".to_string()
        ))
    }

    async fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(Error::Embedding(
            "Embedding not available: no API key configured. Use FTS-only search instead.".to_string()
        ))
    }
}
