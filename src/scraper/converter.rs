//! HTML to Markdown converter using dom_smoothie + fast_html2md.

use crate::core::{Error, Result};
use dom_smoothie::{Article, Config, Readability, TextMode};

/// HTML to Markdown conversion result.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Page title.
    pub title: String,
    /// Markdown content.
    pub content: String,
    /// Short excerpt/summary.
    pub excerpt: Option<String>,
    /// Author/byline.
    pub author: Option<String>,
    /// Published time.
    pub published_time: Option<String>,
}

/// HTML to Markdown converter with content extraction.
pub struct HtmlToMarkdown;

impl HtmlToMarkdown {
    /// Convert HTML to Markdown with content extraction.
    pub fn convert(html: &str, url: &str) -> Result<ConversionResult> {
        // Stage 1: Extract main content using dom_smoothie
        let article = Self::extract_content(html, url)?;

        // Stage 2: Convert to Markdown using fast_html2md
        let markdown = Self::to_markdown(&article.content)?;

        Ok(ConversionResult {
            title: article.title,
            content: markdown,
            excerpt: article.excerpt,
            author: article.byline,
            published_time: article.published_time,
        })
    }

    /// Extract main content from HTML using dom_smoothie.
    fn extract_content(html: &str, url: &str) -> Result<Article> {
        let config = Config {
            max_elements_to_parse: 9000,
            text_mode: TextMode::Markdown,
            ..Default::default()
        };

        let mut readability = Readability::new(html, Some(url), Some(config))
            .map_err(|e| Error::ParseError(format!("Readability init error: {}", e)))?;

        readability
            .parse()
            .map_err(|e| Error::ParseError(format!("Readability parse error: {}", e)))
    }

    /// Convert clean HTML to Markdown using fast_html2md.
    fn to_markdown(clean_html: &str) -> Result<String> {
        // fast_html2md::parse_html takes a bool for commonmark mode
        // commonmark=false gives better output for general use
        let md = fast_html2md::parse_html(clean_html, false);

        // Post-process: clean up excessive whitespace
        let processed = Self::post_process(&md);

        Ok(processed)
    }

    /// Post-process the markdown output.
    fn post_process(markdown: &str) -> String {
        let mut result = markdown.to_string();

        // Remove excessive newlines (more than 2)
        while result.contains("\n\n\n\n") {
            result = result.replace("\n\n\n\n", "\n\n\n");
        }

        // Trim whitespace at start and end
        result = result.trim().to_string();

        result
    }
}

impl Default for HtmlToMarkdown {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_basic() {
        let html = "<html>\
            <head><title>Test Page</title></head>\
            <body>\
                <nav>Navigation</nav>\
                <article>\
                    <h1>Main Title</h1>\
                    <p>This is <strong>bold</strong> text.</p>\
                </article>\
                <footer>Footer</footer>\
            </body>\
            </html>";

        let result = HtmlToMarkdown::convert(html, "https://example.com/test");
        assert!(result.is_ok(), "Conversion should succeed");

        let article = result.unwrap();
        assert!(article.title.contains("Test") || article.title.contains("Main"));
        assert!(article.content.contains("bold"));
    }

    #[test]
    fn test_removes_scripts_and_styles() {
        let html = "<html>\
            <head>\
                <title>Test</title>\
                <script>var x = 1;</script>\
                <style>body { color: red; }</style>\
            </head>\
            <body>\
                <main>\
                    <h1>Content</h1>\
                    <p>Real content here.</p>\
                </main>\
            </body>\
            </html>";

        let result = HtmlToMarkdown::convert(html, "https://example.com/test").unwrap();

        // Should not contain script or style content
        assert!(!result.content.contains("var x = 1"));
        assert!(!result.content.contains("color: red"));
        assert!(result.content.contains("Real content"));
    }

    #[test]
    fn test_extracts_main_content() {
        let html = "<html>\
            <head><title>Documentation</title></head>\
            <body>\
                <header>Site Header</header>\
                <nav>Menu</nav>\
                <main>\
                    <h1>Getting Started</h1>\
                    <p>This is the main documentation content.</p>\
                    <h2>Installation</h2>\
                    <p>Install with cargo.</p>\
                </main>\
                <footer>Copyright 2024</footer>\
            </body>\
            </html>";

        let result = HtmlToMarkdown::convert(html, "https://docs.example.com/guide").unwrap();

        // Should contain main content
        assert!(result.content.contains("Getting Started"));
        assert!(result.content.contains("documentation content"));

        // Should not contain navigation/header/footer
        assert!(!result.content.contains("Site Header"));
        assert!(!result.content.contains("Menu"));
        assert!(!result.content.contains("Copyright"));
    }

    #[test]
    fn test_post_process_removes_excess_newlines() {
        let markdown = "Line 1\n\n\n\n\nLine 2";
        let processed = HtmlToMarkdown::post_process(markdown);

        // Should reduce to at most 3 newlines
        assert!(!processed.contains("\n\n\n\n"));
        assert!(processed.contains("Line 1"));
        assert!(processed.contains("Line 2"));
    }
}
