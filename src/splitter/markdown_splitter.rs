//! Markdown text splitter that respects document structure.

use crate::core::ChunkMetadata;
use crate::splitter::{SplitConfig, TextChunk};

/// Markdown splitter that splits by headers and sections.
pub struct MarkdownSplitter {
    config: SplitConfig,
}

impl MarkdownSplitter {
    /// Create a new markdown splitter with default configuration.
    pub fn new() -> Self {
        Self {
            config: SplitConfig::default(),
        }
    }

    /// Create a markdown splitter with custom configuration.
    pub fn with_config(config: SplitConfig) -> Self {
        Self { config }
    }

    /// Split markdown text into chunks, respecting header structure.
    pub fn split(&self, markdown: &str) -> Vec<TextChunk> {
        let sections = self.parse_sections(markdown);
        let mut chunks = Vec::new();
        let mut sort_order = 0;

        for section in sections {
            let section_chunks = self.split_section(&section, &mut sort_order);
            chunks.extend(section_chunks);
        }

        chunks
    }

    /// Parse markdown into sections based on headers.
    fn parse_sections(&self, markdown: &str) -> Vec<Section> {
        let mut sections = Vec::new();
        let mut current_section = Section::default();

        for line in markdown.lines() {
            let trimmed = line.trim();

            // Check for headers
            if let Some(header) = self.parse_header(trimmed) {
                // Save current section if it has content
                if !current_section.content.is_empty() {
                    sections.push(current_section);
                }

                // Start new section
                current_section = Section {
                    level: header.level,
                    title: header.title.clone(),
                    path: self.build_path(&sections, &header),
                    content: format!("{}\n\n", line), // Include header in content
                };
            } else {
                // Add content to current section
                if !current_section.content.is_empty() {
                    current_section.content.push('\n');
                }
                current_section.content.push_str(line);
            }
        }

        // Don't forget the last section
        if !current_section.content.trim().is_empty() {
            sections.push(current_section);
        }

        sections
    }

    /// Parse a header line.
    fn parse_header(&self, line: &str) -> Option<Header> {
        // ATX headers (# Header)
        let hash_count = line.chars().take_while(|&c| c == '#').count();
        if hash_count > 0 && hash_count <= 6 {
            let title = line[hash_count..].trim().to_string();
            return Some(Header {
                level: hash_count as i32,
                title,
            });
        }

        // Setext headers (underline with === or ---)
        if line.starts_with("===") || line.starts_with("---") {
            // This would need context from the previous line
            // For simplicity, we'll skip setext headers
        }

        None
    }

    /// Build the path for a header (breadcrumb of parent headers).
    fn build_path(&self, sections: &[Section], header: &Header) -> Vec<String> {
        let mut path = Vec::new();

        // Find all parent headers
        for section in sections {
            if section.level < header.level {
                if path.len() < (section.level as usize) {
                    path.resize(section.level as usize, String::new());
                }
                path[(section.level as usize) - 1] = section.title.clone();
            }
        }

        // Add current header
        if path.len() < (header.level as usize) {
            path.resize(header.level as usize, String::new());
        }
        path[(header.level as usize) - 1] = header.title.clone();

        // Remove empty entries
        path.retain(|s| !s.is_empty());

        path
    }

    /// Split a section into chunks if it exceeds the chunk size.
    fn split_section(&self, section: &Section, sort_order: &mut i32) -> Vec<TextChunk> {
        let content = section.content.trim();

        // Handle empty content
        if content.is_empty() {
            return Vec::new();
        }

        if content.len() <= self.config.chunk_size {
            return vec![TextChunk::new(
                content.to_string(),
                ChunkMetadata {
                    level: Some(section.level),
                    path: if section.path.is_empty() {
                        None
                    } else {
                        Some(section.path.clone())
                    },
                    types: Some(vec!["text".to_string()]),
                },
                *sort_order,
            )];
        }

        // Split large sections
        let mut chunks = Vec::new();
        let mut start = 0;
        let content_len = content.len();

        while start < content_len {
            let end = self.find_split_point(content, start);
            let chunk_content = content[start..end].trim();

            if !chunk_content.is_empty() {
                chunks.push(TextChunk::new(
                    chunk_content.to_string(),
                    ChunkMetadata {
                        level: Some(section.level),
                        path: if section.path.is_empty() {
                            None
                        } else {
                            Some(section.path.clone())
                        },
                        types: Some(vec!["text".to_string()]),
                    },
                    *sort_order,
                ));
                *sort_order += 1;
            }

            // Prevent infinite loop: ensure we make progress
            let next_start = end.saturating_sub(self.config.chunk_overlap);
            if next_start <= start {
                // If overlap would cause us to not make progress, move forward by chunk_size
                start = (start + self.config.chunk_size).min(content_len);
            } else {
                start = next_start;
            }

            if start >= content_len {
                break;
            }
        }

        chunks
    }

    /// Find a good split point in the content.
    fn find_split_point(&self, content: &str, start: usize) -> usize {
        let content_len = content.len();

        // Bounds check: ensure start is within content
        if start >= content_len {
            return content_len;
        }

        let ideal_end = (start + self.config.chunk_size).min(content_len);

        if ideal_end == content_len {
            return content_len;
        }

        // Try to find paragraph break
        let search_range = &content[start..ideal_end + 200.min(content.len() - ideal_end)];
        if let Some(pos) = search_range.rfind("\n\n") {
            return start + pos + 2;
        }

        // Try to find line break
        if let Some(pos) = search_range.rfind('\n') {
            return start + pos + 1;
        }

        ideal_end
    }
}

impl Default for MarkdownSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// A parsed markdown section.
#[derive(Debug, Clone, Default)]
struct Section {
    level: i32,
    title: String,
    path: Vec<String>,
    content: String,
}

/// A parsed header.
#[derive(Debug, Clone)]
struct Header {
    level: i32,
    title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_simple_markdown() {
        let markdown = r#"# Main Title

This is the introduction.

## Section 1

Content for section 1.

## Section 2

Content for section 2.
"#;
        let splitter = MarkdownSplitter::new();
        let chunks = splitter.split(markdown);

        assert!(!chunks.is_empty());

        // Check that headers are included
        let content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(content.contains("Main Title"));
        assert!(content.contains("Section 1"));
        assert!(content.contains("Section 2"));
    }

    #[test]
    fn test_split_empty() {
        let splitter = MarkdownSplitter::new();
        let chunks = splitter.split("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_metadata_path() {
        let markdown = r#"# Level 1

## Level 2

### Level 3

Content here.
"#;
        let splitter = MarkdownSplitter::new();
        let chunks = splitter.split(markdown);

        // Find the chunk for Level 3
        let level3_chunk = chunks.iter().find(|c| c.content.contains("Level 3"));
        assert!(level3_chunk.is_some());

        let metadata = &level3_chunk.unwrap().metadata;
        assert_eq!(metadata.level, Some(3));
        assert!(metadata.path.is_some());
        let path = metadata.path.as_ref().unwrap();
        assert!(path.contains(&"Level 1".to_string()));
    }
}
