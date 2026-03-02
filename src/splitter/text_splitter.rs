//! Basic text splitter.

use crate::core::ChunkMetadata;
use crate::splitter::TextChunk;

/// Configuration for text splitting.
#[derive(Debug, Clone)]
pub struct SplitConfig {
    /// Target chunk size in characters.
    pub chunk_size: usize,
    /// Overlap between chunks in characters.
    pub chunk_overlap: usize,
    /// Whether to preserve word boundaries.
    pub preserve_words: bool,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1500,
            chunk_overlap: 200,
            preserve_words: true,
        }
    }
}

/// Basic text splitter.
pub struct TextSplitter {
    config: SplitConfig,
}

impl TextSplitter {
    /// Create a new text splitter with default configuration.
    pub fn new() -> Self {
        Self {
            config: SplitConfig::default(),
        }
    }

    /// Create a text splitter with custom configuration.
    pub fn with_config(config: SplitConfig) -> Self {
        Self { config }
    }

    /// Split text into chunks.
    pub fn split(&self, text: &str) -> Vec<TextChunk> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let mut sort_order = 0;

        while start < text.len() {
            let end = self.find_chunk_end(text, start);

            let chunk_text = text[start..end].trim();
            if !chunk_text.is_empty() {
                chunks.push(TextChunk::new(
                    chunk_text.to_string(),
                    ChunkMetadata::default(),
                    sort_order,
                ));
                sort_order += 1;
            }

            // Move start with overlap
            let next_start = if end >= text.len() {
                text.len()
            } else {
                end.saturating_sub(self.config.chunk_overlap)
            };

            if next_start <= start {
                // Prevent infinite loop
                start = end;
            } else {
                start = next_start;
            }
        }

        chunks
    }

    /// Find the end position of a chunk.
    fn find_chunk_end(&self, text: &str, start: usize) -> usize {
        let ideal_end = start + self.config.chunk_size;

        if ideal_end >= text.len() {
            return text.len();
        }

        if self.config.preserve_words {
            // Try to find a word boundary
            let search_start = ideal_end.saturating_sub(100);
            let search_text = &text[search_start..ideal_end + 100.min(text.len() - ideal_end)];

            // Look for word boundary
            for (i, c) in search_text.char_indices() {
                if i + search_start >= ideal_end && c.is_whitespace() {
                    return i + search_start;
                }
            }
        }

        ideal_end
    }

    /// Split text by separators (recursive character text splitter style).
    pub fn split_by_separators(&self, text: &str, separators: &[&str]) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut sort_order = 0;

        fn split_recursive(
            text: &str,
            separators: &[&str],
            chunk_size: usize,
            chunk_overlap: usize,
        ) -> Vec<String> {
            let mut result = Vec::new();

            if text.len() <= chunk_size {
                if !text.trim().is_empty() {
                    result.push(text.trim().to_string());
                }
                return result;
            }

            // Try each separator
            for (_i, separator) in separators.iter().enumerate() {
                if text.contains(separator) {
                    let splits: Vec<&str> = text.split(separator).collect();
                    let mut current_chunk = String::new();

                    for split in splits {
                        if current_chunk.len() + split.len() + separator.len() <= chunk_size {
                            if !current_chunk.is_empty() {
                                current_chunk.push_str(separator);
                            }
                            current_chunk.push_str(split);
                        } else {
                            if !current_chunk.is_empty() {
                                result.push(current_chunk.trim().to_string());
                            }
                            current_chunk = split.to_string();
                        }
                    }

                    if !current_chunk.is_empty() {
                        result.push(current_chunk.trim().to_string());
                    }

                    return result;
                }
            }

            // No separator found, split by character limit
            let mut start = 0;
            while start < text.len() {
                let end = (start + chunk_size).min(text.len());
                result.push(text[start..end].trim().to_string());
                start = end.saturating_sub(chunk_overlap);
            }

            result
        }

        let split_texts = split_recursive(
            text,
            separators,
            self.config.chunk_size,
            self.config.chunk_overlap,
        );

        for split_text in split_texts {
            if !split_text.is_empty() {
                chunks.push(TextChunk::new(
                    split_text,
                    ChunkMetadata::default(),
                    sort_order,
                ));
                sort_order += 1;
            }
        }

        chunks
    }

    /// Get the configuration.
    pub fn config(&self) -> &SplitConfig {
        &self.config
    }
}

impl Default for TextSplitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_basic() {
        let text = "This is a test. ".repeat(100);
        let splitter = TextSplitter::new();
        let chunks = splitter.split(&text);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.content.len() <= splitter.config.chunk_size + 100); // Allow some flexibility
        }
    }

    #[test]
    fn test_split_empty() {
        let splitter = TextSplitter::new();
        let chunks = splitter.split("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_small_text() {
        let text = "This is a small text.";
        let splitter = TextSplitter::new();
        let chunks = splitter.split(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
    }

    #[test]
    fn test_split_by_separators() {
        let text = "Paragraph 1.\n\nParagraph 2.\n\nParagraph 3.";
        let splitter = TextSplitter::new();
        let chunks = splitter.split_by_separators(text, &["\n\n", "\n", ". ", " "]);

        assert!(!chunks.is_empty());
    }
}
