//! Text splitting module for chunking documents.

mod text_splitter;
mod markdown_splitter;
mod code_splitter;

pub use text_splitter::{TextSplitter, SplitConfig};
pub use markdown_splitter::MarkdownSplitter;
pub use code_splitter::{CodeSplitter, CodeSplitConfig};

use crate::core::ChunkMetadata;
use serde::{Deserialize, Serialize};

/// A chunk of text from a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    /// The chunk content.
    pub content: String,
    /// Chunk metadata.
    pub metadata: ChunkMetadata,
    /// Sort order within the document.
    pub sort_order: i32,
}

impl TextChunk {
    /// Create a new text chunk.
    pub fn new(content: String, metadata: ChunkMetadata, sort_order: i32) -> Self {
        Self {
            content,
            metadata,
            sort_order,
        }
    }
}
