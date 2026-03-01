//! Google (Gemini) embedding provider.

use crate::core::{Error, Result};
use crate::embed::{Embedder, EmbeddingModel, GOOGLE_MODELS};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};

/// Google embedding provider.
pub struct GoogleEmbedder {
    client: HttpClient,
    api_key: String,
    model: String,
    dimension: usize,
    base_url: String,
}

impl GoogleEmbedder {
    /// Create a new Google embedder.
    pub fn new(api_key: String, model: String, dimension: usize) -> Result<Self> {
        let client = HttpClient::new();
        
        Ok(Self {
            client,
            api_key,
            model,
            dimension,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        })
    }

    /// Create with custom base URL.
    pub fn with_base_url(api_key: String, model: String, dimension: usize, base_url: String) -> Result<Self> {
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
        let request = EmbedRequest {
            content: Content {
                parts: vec![Part { text: text.to_string() }],
            },
        };
        
        let response = self.client
            .post(self.get_embed_url())
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(format!("Google API request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(format!(
                "Google API error ({}): {}",
                status, body
            )));
        }
        
        let result: EmbedResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;
        
        let mut embedding = result.embedding.values;
        embedding.resize(self.dimension, 0.0);
        
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Google's batch API has a limit, so we process in smaller batches
        let batch_size = 100;
        let mut all_embeddings = Vec::with_capacity(texts.len());
        
        for chunk in texts.chunks(batch_size) {
            let requests: Vec<EmbedRequest> = chunk
                .iter()
                .map(|text| EmbedRequest {
                    content: Content {
                        parts: vec![Part { text: text.to_string() }],
                    },
                })
                .collect();
            
            let request = BatchEmbedRequest { requests };
            
            let response = self.client
                .post(self.get_batch_embed_url())
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Http(format!("Google API request failed: {}", e)))?;
            
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(Error::Embedding(format!(
                    "Google API error ({}): {}",
                    status, body
                )));
            }
            
            let result: BatchEmbedResponse = response
                .json()
                .await
                .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;
            
            for mut emb in result.embeddings.into_iter().map(|e| e.values) {
                emb.resize(self.dimension, 0.0);
                all_embeddings.push(emb);
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
        ).unwrap();
        
        let info = embedder.model_info();
        assert_eq!(info.id, "text-embedding-004");
        assert_eq!(info.dimension, 768);
    }
}
