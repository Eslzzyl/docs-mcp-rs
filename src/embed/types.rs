//! Embedding types.

use serde::{Deserialize, Serialize};

/// Embedding model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    /// Model identifier.
    pub id: String,
    /// Dimension of the embedding vectors.
    pub dimension: usize,
    /// Maximum input tokens.
    pub max_tokens: usize,
}

/// Result of an embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// The embedding vector.
    pub embedding: Vec<f32>,
    /// Number of tokens used.
    pub tokens_used: Option<usize>,
}

impl EmbeddingResult {
    /// Create a new embedding result.
    pub fn new(embedding: Vec<f32>) -> Self {
        Self {
            embedding,
            tokens_used: None,
        }
    }

    /// Create with token count.
    pub fn with_tokens(embedding: Vec<f32>, tokens: usize) -> Self {
        Self {
            embedding,
            tokens_used: Some(tokens),
        }
    }
}

/// Known embedding models.
pub const OPENAI_MODELS: &[(&str, usize, usize)] = &[
    // (model_id, dimension, max_tokens)
    ("text-embedding-3-small", 1536, 8191),
    ("text-embedding-3-large", 3072, 8191),
    ("text-embedding-ada-002", 1536, 8191),
];

pub const GOOGLE_MODELS: &[(&str, usize, usize)] = &[
    ("text-embedding-004", 768, 2048),
    ("embedding-001", 768, 2048),
];

/// Get the dimension for a model.
#[allow(dead_code)]
pub fn get_model_dimension(model_id: &str) -> Option<usize> {
    OPENAI_MODELS
        .iter()
        .chain(GOOGLE_MODELS.iter())
        .find(|(id, _, _)| *id == model_id)
        .map(|(_, dim, _)| *dim)
}

/// Check if a dimension matches a model.
#[allow(dead_code)]
pub fn is_valid_dimension(model_id: &str, dimension: usize) -> bool {
    get_model_dimension(model_id)
        .map(|d| d == dimension)
        .unwrap_or(true) // Allow unknown models
}
