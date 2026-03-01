//! Configuration management.

use crate::core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Database storage path.
    #[serde(default = "default_store_path")]
    pub store_path: PathBuf,

    /// Server configuration.
    #[serde(default)]
    pub server: ServerConfig,

    /// Scraper configuration.
    #[serde(default)]
    pub scraper: ScraperConfig,

    /// Splitter configuration.
    #[serde(default)]
    pub splitter: SplitterConfig,

    /// Embedding configuration.
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

fn default_store_path() -> PathBuf {
    PathBuf::from("./data/docs.db")
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP server host.
    #[serde(default = "default_host")]
    pub host: String,

    /// HTTP server port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Scraper configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraperConfig {
    /// Maximum number of pages to scrape.
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,

    /// Maximum crawl depth.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// Maximum concurrent requests.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// User agent string.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

fn default_max_pages() -> usize {
    1000
}

fn default_max_depth() -> usize {
    3
}

fn default_max_concurrency() -> usize {
    5
}

fn default_timeout() -> u64 {
    30
}

fn default_user_agent() -> String {
    format!("docs-mcp-rs/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for ScraperConfig {
    fn default() -> Self {
        Self {
            max_pages: default_max_pages(),
            max_depth: default_max_depth(),
            max_concurrency: default_max_concurrency(),
            timeout_secs: default_timeout(),
            user_agent: default_user_agent(),
        }
    }
}

/// Splitter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitterConfig {
    /// Default chunk size for text splitting.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Overlap between chunks.
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Code chunk size.
    #[serde(default = "default_code_chunk_size")]
    pub code_chunk_size: usize,
}

fn default_chunk_size() -> usize {
    1000
}

fn default_chunk_overlap() -> usize {
    200
}

fn default_code_chunk_size() -> usize {
    1500
}

impl Default for SplitterConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            code_chunk_size: default_code_chunk_size(),
        }
    }
}

/// Embedding provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding provider type.
    #[serde(default = "default_provider")]
    pub provider: EmbeddingProvider,

    /// OpenAI API key.
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// OpenAI embedding model.
    #[serde(default = "default_openai_model")]
    pub openai_model: String,

    /// Google API key.
    #[serde(default)]
    pub google_api_key: Option<String>,

    /// Google embedding model.
    #[serde(default = "default_google_model")]
    pub google_model: String,

    /// Embedding dimension (for vector storage).
    #[serde(default = "default_embedding_dim")]
    pub dimension: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    #[default]
    OpenAI,
    Google,
}

fn default_provider() -> EmbeddingProvider {
    EmbeddingProvider::OpenAI
}

fn default_openai_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_google_model() -> String {
    "text-embedding-004".to_string()
}

fn default_embedding_dim() -> usize {
    1536 // Default for text-embedding-3-small
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            openai_api_key: None,
            openai_model: default_openai_model(),
            google_api_key: None,
            google_model: default_google_model(),
            dimension: default_embedding_dim(),
        }
    }
}

impl Config {
    /// Create a new configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config file: {}", e)))?;

        Ok(config)
    }

    /// Get the database path.
    pub fn database_path(&self) -> PathBuf {
        self.store_path.clone()
    }

    /// Ensure the storage directory exists.
    pub fn ensure_storage_dir(&self) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Config(format!("Failed to create storage directory: {}", e)))?;
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            store_path: default_store_path(),
            server: ServerConfig::default(),
            scraper: ScraperConfig::default(),
            splitter: SplitterConfig::default(),
            embedding: EmbeddingConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.store_path, PathBuf::from("./data/docs.db"));
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.scraper.max_pages, 1000);
        assert_eq!(config.splitter.chunk_size, 1000);
        assert_eq!(config.embedding.provider, EmbeddingProvider::OpenAI);
    }
}
