//! OpenAI embedding provider.

use crate::core::{Error, Result};
use crate::embed::{Embedder, EmbeddingModel, OPENAI_MODELS};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};

/// OpenAI embedding provider.
pub struct OpenAIEmbedder {
    client: HttpClient,
    api_key: String,
    model: String,
    dimension: usize,
    base_url: String,
}

impl OpenAIEmbedder {
    /// Create a new OpenAI embedder.
    pub fn new(api_key: String, model: String, dimension: usize) -> Result<Self> {
        let client = HttpClient::new();
        
        Ok(Self {
            client,
            api_key,
            model,
            dimension,
            base_url: "https://api.openai.com/v1".to_string(),
        })
    }

    /// Create with a custom base URL (for Azure or custom endpoints).
    pub fn with_base_url(api_key: String, base_url: String, model: String, dimension: usize) -> Result<Self> {
        let client = HttpClient::new();
        
        Ok(Self {
            client,
            api_key,
            model,
            dimension,
            base_url,
        })
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
        embeddings.into_iter().next()
            .ok_or_else(|| Error::Embedding("No embedding returned".to_string()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
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
        
        let response = self.client
            .post(self.get_embed_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(format!("OpenAI API request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(format!(
                "OpenAI API error ({}): {}",
                status, body
            )));
        }
        
        let result: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;
        
        // Sort by index to maintain order
        let mut embeddings: Vec<(i32, Vec<f32>)> = result
            .data
            .into_iter()
            .map(|e| {
                let embedding = e.embedding.into_iter()
                    .map(|f| f as f32)
                    .collect();
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
        
        Ok(result)
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
        ).unwrap();
        
        let info = embedder.model_info();
        assert_eq!(info.id, "text-embedding-3-small");
        assert_eq!(info.dimension, 1536);
    }
}