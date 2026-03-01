//! Code splitter for source code files.

use crate::core::ChunkMetadata;
use crate::splitter::{SplitConfig, TextChunk};

/// Code splitter configuration.
#[derive(Debug, Clone)]
pub struct CodeSplitConfig {
    /// Base split configuration.
    pub base: SplitConfig,
    /// Maximum lines per chunk.
    pub max_lines_per_chunk: usize,
    /// Whether to preserve function boundaries.
    pub preserve_functions: bool,
}

impl Default for CodeSplitConfig {
    fn default() -> Self {
        Self {
            base: SplitConfig {
                chunk_size: 1500,
                chunk_overlap: 200,
                preserve_words: true,
            },
            max_lines_per_chunk: 100,
            preserve_functions: true,
        }
    }
}

/// Code splitter for source code files.
pub struct CodeSplitter {
    config: CodeSplitConfig,
}

impl CodeSplitter {
    /// Create a new code splitter with default configuration.
    pub fn new() -> Self {
        Self {
            config: CodeSplitConfig::default(),
        }
    }

    /// Create a code splitter with custom configuration.
    pub fn with_config(config: CodeSplitConfig) -> Self {
        Self { config }
    }

    /// Split code into chunks.
    pub fn split(&self, code: &str, language: &str) -> Vec<TextChunk> {
        let lines: Vec<&str> = code.lines().collect();
        let mut chunks = Vec::new();
        let mut sort_order = 0;

        if lines.is_empty() {
            return chunks;
        }

        // Try to split by functions/classes first
        if self.config.preserve_functions {
            let blocks = self.find_code_blocks(&lines, language);
            
            for block in blocks {
                if block.content.len() <= self.config.base.chunk_size {
                    chunks.push(TextChunk::new(
                        block.content,
                        ChunkMetadata {
                            level: Some(block.level),
                            path: Some(vec![block.name.clone()]),
                            types: Some(vec!["code".to_string(), language.to_string()]),
                        },
                        sort_order,
                    ));
                    sort_order += 1;
                } else {
                    // Split large blocks
                    let sub_chunks = self.split_large_block(&block, &mut sort_order, language);
                    chunks.extend(sub_chunks);
                }
            }
        } else {
            // Simple line-based splitting
            chunks = self.split_by_lines(&lines, &mut sort_order, language);
        }

        chunks
    }

    /// Find code blocks (functions, classes, etc.).
    fn find_code_blocks(&self, lines: &[&str], language: &str) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let mut current_block: Option<CodeBlock> = None;
        let mut brace_count = 0;
        let mut in_block = false;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Check for function/class definitions based on language
            let definition = self.find_definition(trimmed, language, in_block);

            if let Some((name, level)) = definition {
                // Save previous block if exists
                if let Some(block) = current_block.take() {
                    blocks.push(block);
                }
                
                current_block = Some(CodeBlock {
                    name,
                    level,
                    start_line: line_num,
                    content: line.to_string(),
                });
                in_block = true;
                brace_count = self.count_braces(trimmed);
            } else if in_block {
                if let Some(ref mut block) = current_block {
                    block.content.push('\n');
                    block.content.push_str(line);
                    
                    brace_count += self.count_braces(trimmed);
                    
                    // Check if block is complete
                    if brace_count == 0 && self.is_block_end(trimmed, language) {
                        blocks.push(current_block.take().unwrap());
                        in_block = false;
                    }
                }
            } else {
                // Code outside any block - add to a default block
                if current_block.is_none() {
                    current_block = Some(CodeBlock {
                        name: "module".to_string(),
                        level: 0,
                        start_line: line_num,
                        content: line.to_string(),
                    });
                } else if let Some(ref mut block) = current_block {
                    block.content.push('\n');
                    block.content.push_str(line);
                    
                    // Check if default block is too large
                    if block.content.len() > self.config.base.chunk_size {
                        blocks.push(current_block.take().unwrap());
                    }
                }
            }
        }

        // Don't forget the last block
        if let Some(block) = current_block {
            blocks.push(block);
        }

        blocks
    }

    /// Find a function/class definition.
    fn find_definition(&self, line: &str, language: &str, in_block: bool) -> Option<(String, i32)> {
        if in_block {
            return None;
        }

        match language {
            "rust" => self.find_rust_definition(line),
            "python" => self.find_python_definition(line),
            "javascript" | "typescript" => self.find_js_definition(line),
            "go" => self.find_go_definition(line),
            _ => self.find_generic_definition(line),
        }
    }

    fn find_rust_definition(&self, line: &str) -> Option<(String, i32)> {
        let patterns = [
            (regex::Regex::new(r"^fn\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^(pub\s+)?struct\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^(pub\s+)?enum\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^(pub\s+)?trait\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^impl\s+(\w+)").ok(), 1),
        ];

        for (pattern, level) in patterns {
            if let Some(re) = pattern {
                if let Some(caps) = re.captures(line) {
                    let name = caps.iter()
                        .skip(1)
                        .filter_map(|c| c.map(|m| m.as_str()))
                        .last()
                        .unwrap_or("unknown")
                        .to_string();
                    return Some((name, level));
                }
            }
        }
        None
    }

    fn find_python_definition(&self, line: &str) -> Option<(String, i32)> {
        let patterns = [
            (regex::Regex::new(r"^def\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^class\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^async\s+def\s+(\w+)").ok(), 1),
        ];

        for (pattern, level) in patterns {
            if let Some(re) = pattern {
                if let Some(caps) = re.captures(line) {
                    let name = caps.get(1).map(|m| m.as_str().to_string())?;
                    return Some((name, level));
                }
            }
        }
        None
    }

    fn find_js_definition(&self, line: &str) -> Option<(String, i32)> {
        let patterns = [
            (regex::Regex::new(r"^function\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^(const|let|var)\s+(\w+)\s*=\s*(async\s+)?function").ok(), 1),
            (regex::Regex::new(r"^(const|let|var)\s+(\w+)\s*=\s*\(").ok(), 1), // Arrow function
            (regex::Regex::new(r"^class\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^(export\s+)?(async\s+)?function\s+(\w+)").ok(), 1),
        ];

        for (pattern, level) in patterns {
            if let Some(re) = pattern {
                if let Some(caps) = re.captures(line) {
                    // Get the function/class name (last capture group that's not a keyword)
                    for i in (1..caps.len()).rev() {
                        if let Some(m) = caps.get(i) {
                            let name = m.as_str();
                            if !["const", "let", "var", "export", "async", "function"].contains(&name) {
                                return Some((name.to_string(), level));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn find_go_definition(&self, line: &str) -> Option<(String, i32)> {
        let patterns = [
            (regex::Regex::new(r"^func\s+(\w+)").ok(), 1),
            (regex::Regex::new(r"^func\s*\(\w+\s+\*?\w+\)\s+(\w+)").ok(), 2), // Method
            (regex::Regex::new(r"^type\s+(\w+)\s+struct").ok(), 1),
            (regex::Regex::new(r"^type\s+(\w+)\s+interface").ok(), 1),
        ];

        for (pattern, level) in patterns {
            if let Some(re) = pattern {
                if let Some(caps) = re.captures(line) {
                    let name = caps.get(1).map(|m| m.as_str().to_string())?;
                    return Some((name, level));
                }
            }
        }
        None
    }

    fn find_generic_definition(&self, line: &str) -> Option<(String, i32)> {
        // Generic pattern for function-like definitions
        let re = regex::Regex::new(r"(\w+)\s*\([^)]*\)\s*\{").ok()?;
        let caps = re.captures(line)?;
        let name = caps.get(1).map(|m| m.as_str().to_string())?;
        Some((name, 1))
    }

    /// Count braces in a line.
    fn count_braces(&self, line: &str) -> i32 {
        let open = line.matches('{').count() as i32;
        let close = line.matches('}').count() as i32;
        open - close
    }

    /// Check if this line ends a code block.
    fn is_block_end(&self, line: &str, language: &str) -> bool {
        match language {
            "python" => line.starts_with("def ") || line.starts_with("class ") || line.starts_with("@"),
            _ => line.trim().starts_with('}'),
        }
    }

    /// Split a large code block.
    fn split_large_block(&self, block: &CodeBlock, sort_order: &mut i32, language: &str) -> Vec<TextChunk> {
        let lines: Vec<&str> = block.content.lines().collect();
        self.split_by_lines(&lines, sort_order, language)
    }

    /// Split code by lines.
    fn split_by_lines(&self, lines: &[&str], sort_order: &mut i32, language: &str) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut current_lines = Vec::new();
        let mut current_size = 0;

        for line in lines {
            let line_size = line.len() + 1; // +1 for newline

            if current_size + line_size > self.config.base.chunk_size 
                || current_lines.len() >= self.config.max_lines_per_chunk 
            {
                if !current_lines.is_empty() {
                    chunks.push(TextChunk::new(
                        current_lines.join("\n"),
                        ChunkMetadata {
                            level: None,
                            path: None,
                            types: Some(vec!["code".to_string(), language.to_string()]),
                        },
                        *sort_order,
                    ));
                    *sort_order += 1;
                }
                current_lines.clear();
                current_size = 0;
            }

            current_lines.push(*line);
            current_size += line_size;
        }

        if !current_lines.is_empty() {
            chunks.push(TextChunk::new(
                current_lines.join("\n"),
                ChunkMetadata {
                    level: None,
                    path: None,
                    types: Some(vec!["code".to_string(), language.to_string()]),
                },
                *sort_order,
            ));
        }

        chunks
    }

    /// Get the configuration.
    pub fn config(&self) -> &CodeSplitConfig {
        &self.config
    }
}

impl Default for CodeSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// A code block (function, class, etc.).
#[derive(Debug, Clone)]
struct CodeBlock {
    name: String,
    level: i32,
    start_line: usize,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_rust_code() {
        let code = r#"
fn main() {
    println!("Hello, world!");
}

struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
"#;
        let splitter = CodeSplitter::new();
        let chunks = splitter.split(code, "rust");
        
        assert!(!chunks.is_empty());
        
        // Check that function/struct names are in the content
        let content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(content.contains("main"));
    }

    #[test]
    fn test_split_python_code() {
        let code = r#"
def hello():
    print("Hello")

class Person:
    def __init__(self, name):
        self.name = name
    
    def greet(self):
        print(f"Hello, {self.name}")
"#;
        let splitter = CodeSplitter::new();
        let chunks = splitter.split(code, "python");
        
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_split_empty() {
        let splitter = CodeSplitter::new();
        let chunks = splitter.split("", "rust");
        assert!(chunks.is_empty());
    }
}
