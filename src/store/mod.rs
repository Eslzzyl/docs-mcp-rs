//! Storage layer module.

mod connection;
mod document_store;
mod library_store;
mod migrations;
mod page_store;
mod vector_search;
mod version_store;

pub use connection::Connection;
pub use document_store::DocumentStore;
pub use library_store::LibraryStore;
pub use migrations::run_migrations;
pub use page_store::PageStore;
pub use vector_search::{SearchOptions, VectorSearch};
pub use version_store::VersionStore;

// Re-export core types for convenience
pub use crate::core::{Document, Library, Page, Version};
