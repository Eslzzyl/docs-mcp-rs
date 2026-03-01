//! Error types for the application.

use std::path::PathBuf;
use thiserror::Error;

/// The main error type for the application.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Database migration error: {0}")]
    Migration(String),

    #[error("Database connection error for path: {0}")]
    DatabaseConnection(PathBuf),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Entity already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Content parsing error: {0}")]
    ContentParsing(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Scraper error: {0}")]
    Scraper(String),

    #[error("Scraping error: {0}")]
    Scraping(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Invalid state transition: {0}")]
    InvalidState(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// A specialized `Result` type for the application.
pub type Result<T> = std::result::Result<T, Error>;
