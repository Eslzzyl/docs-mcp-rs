//! Storage layer module.

mod connection;
mod migrations;
mod library_store;
mod version_store;
mod page_store;
mod document_store;
mod vector_search;

pub use connection::Connection;
pub use migrations::run_migrations;
pub use library_store::LibraryStore;
pub use version_store::VersionStore;
pub use page_store::PageStore;
pub use document_store::DocumentStore;
pub use vector_search::{VectorSearch, SearchOptions};

// Re-export core types for convenience
pub use crate::core::{Library, Version, Page, Document};
