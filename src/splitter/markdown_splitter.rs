//! Markdown text splitter that respects document structure.

use crate::core::ChunkMetadata;
use crate::splitter::{SplitConfig, TextChunk};
use tracing::{debug, trace, warn};

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
        let input_len = markdown.len();
        debug!("Starting markdown split: input_length={}", input_len);

        if markdown.trim().is_empty() {
            warn!("Attempted to split empty markdown content");
            return Vec::new();
        }

        let sections = self.parse_sections(markdown);
        debug!("Parsed {} sections from markdown", sections.len());

        let mut chunks = Vec::new();
        let mut sort_order = 0;

        for (idx, section) in sections.iter().enumerate() {
            trace!("Processing section {}: level={}, title='{}', content_length={}",
                   idx, section.level, section.title, section.content.len());
            let section_chunks = self.split_section(section, &mut sort_order);
            trace!("Section {} produced {} chunks", idx, section_chunks.len());
            chunks.extend(section_chunks);
        }

        debug!("Markdown split complete: {} chunks produced from {} sections", chunks.len(), sections.len());
        chunks
    }

    /// Parse markdown into sections based on headers.
    /// Sections that only contain a header (no content) will be merged with the next section.
    fn parse_sections(&self, markdown: &str) -> Vec<Section> {
        let mut sections: Vec<Section> = Vec::new();
        let mut pending_header_sections: Vec<Section> = Vec::new();
        let mut current_section = Section::default();

        for line in markdown.lines() {
            let trimmed = line.trim();

            // Check for headers
            if let Some(header) = self.parse_header(trimmed) {
                // Check if current section has content beyond just the header
                // A section with only header has content like "# Title\n\n" (ends with \n\n and no other text)
                let content_trimmed = current_section.content.trim();
                let has_real_content = !content_trimmed.is_empty() &&
                    // Check if there's content after the header (not just the header line)
                    (current_section.level == 0 || // Content before first header always counts
                     content_trimmed.len() > current_section.title.len() + 3); // +3 for "# " and possible extra chars

                if has_real_content {
                    // Prepend any pending header-only sections to this section
                    let mut final_content = current_section.content.clone();
                    let mut final_level = current_section.level;
                    let mut final_title = current_section.title.clone();
                    let mut final_path = current_section.path.clone();

                    // Merge pending headers in reverse order (oldest first)
                    for pending in pending_header_sections.drain(..).rev() {
                        final_content = format!("{}\n\n{}", pending.content.trim(), final_content.trim());
                        // Update the path to include the parent headers
                        if final_level > pending.level {
                            let mut new_path = pending.path.clone();
                            new_path.push(pending.title.clone());
                            final_path = new_path;
                        }
                        // If the current section was just content (level 0), adopt the first pending header's level
                        if final_level == 0 {
                            final_level = pending.level;
                            final_title = pending.title.clone();
                        }
                    }

                    sections.push(Section {
                        level: final_level,
                        title: final_title,
                        path: final_path,
                        content: final_content,
                    });
                } else if current_section.level > 0 {
                    // Current section is header-only, add to pending list
                    pending_header_sections.push(current_section);
                }
                // If level 0 (content before first header) and no real content, just discard

                // Start new section with this header
                current_section = Section {
                    level: header.level,
                    title: header.title.clone(),
                    path: self.build_path(&sections, &header),
                    content: format!("{}\n\n", line),
                };
            } else {
                // Add content to current section
                if !current_section.content.is_empty() {
                    current_section.content.push('\n');
                }
                current_section.content.push_str(line);
            }
        }

        // Handle the last section
        let content_trimmed = current_section.content.trim();
        let has_real_content = !content_trimmed.is_empty() &&
            (current_section.level == 0 ||
             content_trimmed.len() > current_section.title.len() + 3);

        if has_real_content {
            // Prepend any pending header-only sections
            let mut final_content = current_section.content.clone();
            let mut final_level = current_section.level;
            let mut final_title = current_section.title.clone();
            let mut final_path = current_section.path.clone();

            for pending in pending_header_sections.drain(..).rev() {
                final_content = format!("{}\n\n{}", pending.content.trim(), final_content.trim());
                if final_level > pending.level {
                    let mut new_path = pending.path.clone();
                    new_path.push(pending.title.clone());
                    final_path = new_path;
                }
                if final_level == 0 {
                    final_level = pending.level;
                    final_title = pending.title.clone();
                }
            }

            sections.push(Section {
                level: final_level,
                title: final_title,
                path: final_path,
                content: final_content,
            });
        } else if current_section.level > 0 {
            // Last section is header-only, merge with previous section if exists
            pending_header_sections.push(current_section);
            if let Some(last) = sections.last_mut() {
                for pending in pending_header_sections.drain(..) {
                    last.content = format!("{}\n\n{}", last.content.trim(), pending.content.trim());
                }
            }
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

        // Skip if header level is 0 or negative (invalid)
        if header.level <= 0 {
            trace!("Skipping path build for invalid header level: {}", header.level);
            return path;
        }

        // Find all parent headers
        for section in sections {
            if section.level < header.level && section.level > 0 {
                let level_idx = section.level as usize;
                if path.len() < level_idx {
                    path.resize(level_idx, String::new());
                }
                path[level_idx - 1] = section.title.clone();
            }
        }

        // Add current header
        let header_level = header.level as usize;
        if path.len() < header_level {
            path.resize(header_level, String::new());
        }
        path[header_level - 1] = header.title.clone();

        // Remove empty entries
        path.retain(|s| !s.is_empty());

        path
    }

    /// Split a section into chunks if it exceeds the chunk size.
    fn split_section(&self, section: &Section, sort_order: &mut i32) -> Vec<TextChunk> {
        let content = section.content.trim();
        let content_len = content.len();

        trace!("Splitting section: title='{}', level={}, content_length={}",
               section.title, section.level, content_len);

        // Handle empty content
        if content.is_empty() {
            trace!("Skipping empty section: '{}'", section.title);
            return Vec::new();
        }

        // Handle sections with invalid level (content before first header)
        if section.level <= 0 {
            trace!("Processing content section with level 0: {} bytes", content_len);
        }

        if content_len <= self.config.chunk_size {
            trace!("Section '{}' fits in single chunk ({} <= {})",
                   section.title, content_len, self.config.chunk_size);
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

        debug!("Section '{}' needs splitting: {} bytes > chunk_size {}",
               section.title, content_len, self.config.chunk_size);

        // Split large sections
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut chunk_num = 0;

        while start < content_len {
            // Ensure start is at a character boundary
            start = align_to_char_boundary(content, start);
            if start >= content_len {
                break;
            }

            let end = self.find_split_point(content, start);
            // Ensure end is at a character boundary
            let end = align_to_char_boundary(content, end);
            let chunk_content = content[start..end].trim();

            trace!("Creating chunk {}: bytes [{}..{}] ({} bytes)",
                   chunk_num, start, end, chunk_content.len());

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
                chunk_num += 1;
            } else {
                trace!("Skipping empty chunk at bytes [{}..{}]", start, end);
            }

            // Prevent infinite loop: ensure we make progress
            let next_start = end.saturating_sub(self.config.chunk_overlap);
            if next_start <= start {
                // If overlap would cause us to not make progress, move forward by chunk_size
                let forced_start = (start + self.config.chunk_size).min(content_len);
                trace!("Forcing progress: {} -> {} (avoiding infinite loop)", start, forced_start);
                start = forced_start;
            } else {
                start = next_start;
            }

            if start >= content_len {
                trace!("Reached end of content at byte {}", start);
                break;
            }
        }

        debug!("Section '{}' split into {} chunks", section.title, chunks.len());
        chunks
    }

    /// Find a good split point in the content.
    fn find_split_point(&self, content: &str, start: usize) -> usize {
        let content_len = content.len();

        // Align start to character boundary
        let start = align_to_char_boundary(content, start);

        // Bounds check: ensure start is within content
        if start >= content_len {
            trace!("find_split_point: start ({}) >= content_len ({}), returning end",
                   start, content_len);
            return content_len;
        }

        let ideal_end = (start + self.config.chunk_size).min(content_len);

        if ideal_end == content_len {
            trace!("find_split_point: ideal_end at content end ({}), returning end", content_len);
            return content_len;
        }

        // Try to find paragraph break
        let search_end = ideal_end + 200.min(content.len() - ideal_end);
        let search_end = align_to_char_boundary(content, search_end);
        let search_range = &content[start..search_end];

        if let Some(pos) = search_range.rfind("\n\n") {
            let split_point = start + pos + 2;
            let split_point = align_to_char_boundary(content, split_point);
            trace!("find_split_point: found paragraph break at byte {} (ideal: {})",
                   split_point, ideal_end);
            return split_point;
        }

        // Try to find line break
        if let Some(pos) = search_range.rfind('\n') {
            let split_point = start + pos + 1;
            let split_point = align_to_char_boundary(content, split_point);
            trace!("find_split_point: found line break at byte {} (ideal: {})",
                   split_point, ideal_end);
            return split_point;
        }

        trace!("find_split_point: no natural break found, using ideal_end: {}", ideal_end);
        align_to_char_boundary(content, ideal_end)
    }
}

impl Default for MarkdownSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Align a byte index to the nearest character boundary.
/// If the index is not on a character boundary, move it forward to the next boundary.
fn align_to_char_boundary(content: &str, index: usize) -> usize {
    if index >= content.len() {
        return content.len();
    }

    // Check if already on a boundary
    if content.is_char_boundary(index) {
        return index;
    }

    // Move forward to find the next boundary
    let mut new_index = index + 1;
    while new_index < content.len() && !content.is_char_boundary(new_index) {
        new_index += 1;
    }

    new_index.min(content.len())
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
    fn test_merge_consecutive_empty_headers() {
        // Test that consecutive headers without content are merged with the next section
        let markdown = "# Main Title\n\n## Empty Section 1\n\n## Empty Section 2\n\n### Actual Content\n\nThis is the real content that should include all the parent headers.\n\n## Another Section\n\nMore content here.";

        let splitter = MarkdownSplitter::new();
        let chunks = splitter.split(markdown);

        // Should not have chunks with just headers (content shorter than header itself)
        for chunk in &chunks {
            let content = chunk.content.trim();
            // A chunk should have content beyond just the header line
            // Header line is like "# Title", content should be longer than that
            assert!(
                content.len() > 20,
                "Chunk content too short ({} chars), likely header-only: {}",
                content.len(),
                content
            );
        }

        // The actual content section should include parent headers
        let actual_content_chunk = chunks
            .iter()
            .find(|c| c.content.contains("real content"));
        assert!(actual_content_chunk.is_some(), "Should have chunk with actual content");

        let content = &actual_content_chunk.unwrap().content;
        // Should include the parent headers
        assert!(content.contains("Empty Section 1") || content.contains("Empty Section 2"),
                "Content should include parent headers");
    }

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

    #[test]
    fn test_content_before_header() {
        // Test markdown that starts with content before any header
        let markdown = r#"This is some introductory text.

It has multiple paragraphs.

# First Header

Content under first header.

## Second Header

More content.
"#;
        let splitter = MarkdownSplitter::new();
        let chunks = splitter.split(markdown);

        // Should have chunks for the intro text and each section
        assert!(!chunks.is_empty());

        // Check that intro text is included
        let intro_chunk = chunks.iter().find(|c| c.content.contains("introductory text"));
        assert!(intro_chunk.is_some(), "Should have chunk with intro text");

        // Check that headers are included
        let first_header_chunk = chunks.iter().find(|c| c.content.contains("First Header"));
        assert!(first_header_chunk.is_some(), "Should have chunk with first header");
    }

    #[test]
    fn test_multibyte_utf8_characters() {
        // Test markdown with multi-byte UTF-8 characters (like emojis, Chinese, special symbols)
        let markdown = r#"# Test Header

This content has multi-byte characters: … (ellipsis), 🎉 (emoji), 中文 (Chinese).

## Section with more special chars

Some more content here with various Unicode characters: → ← ↑ ↓ ✓ × ÷ • ·

And a longer paragraph with repeated special characters: … … … … … … … … … …

# Another Header

Final section with emojis: 🚀 🌟 💡 🔧 📝
"#;
        let splitter = MarkdownSplitter::new();
        // This should not panic
        let chunks = splitter.split(markdown);

        // Should produce chunks
        assert!(!chunks.is_empty(), "Should produce at least one chunk");

        // Check that content is preserved
        let all_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(all_content.contains("…"), "Should preserve ellipsis character");
        assert!(all_content.contains("中文"), "Should preserve Chinese characters");
        assert!(all_content.contains("🎉"), "Should preserve emoji");
    }

    #[test]
    fn test_align_to_char_boundary() {
        // Test the helper function
        let text = "Hello…World"; // … is 3 bytes
        // H-e-l-l-o-…-W-o-r-l-d
        // 0-1-2-3-4-5-6-7-8-9-10 (bytes)
        // … occupies bytes 5,6,7

        assert_eq!(align_to_char_boundary(text, 0), 0);
        assert_eq!(align_to_char_boundary(text, 5), 5); // Start of …
        assert_eq!(align_to_char_boundary(text, 6), 8); // Middle of …, should align to 8 (W)
        assert_eq!(align_to_char_boundary(text, 7), 8); // End of …, should align to 8 (W)
        assert_eq!(align_to_char_boundary(text, 8), 8); // W
        assert_eq!(align_to_char_boundary(text, 100), text.len()); // Beyond end
    }
}
