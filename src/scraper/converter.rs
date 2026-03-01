//! HTML to Markdown converter.

use crate::core::Result;
use html2md::parse_html;

/// HTML to Markdown converter.
pub struct HtmlToMarkdown {
    /// Whether to preserve links.
    preserve_links: bool,
    /// Whether to preserve images.
    preserve_images: bool,
}

impl HtmlToMarkdown {
    /// Create a new converter.
    pub fn new() -> Self {
        Self {
            preserve_links: true,
            preserve_images: true,
        }
    }

    /// Set whether to preserve links.
    pub fn with_preserve_links(mut self, preserve: bool) -> Self {
        self.preserve_links = preserve;
        self
    }

    /// Set whether to preserve images.
    pub fn with_preserve_images(mut self, preserve: bool) -> Self {
        self.preserve_images = preserve;
        self
    }

    /// Convert HTML to Markdown.
    pub fn convert(&self, html: &str) -> Result<String> {
        let markdown = parse_html(html);
        
        // Post-process
        let processed = self.post_process(&markdown);
        
        Ok(processed)
    }

    /// Convert HTML to Markdown with custom handling.
    pub fn convert_with_base_url(&self, html: &str, base_url: &str) -> Result<String> {
        let mut markdown = parse_html(html);
        
        // Convert relative links to absolute if needed
        if self.preserve_links {
            markdown = self.make_links_absolute(&markdown, base_url);
        }
        
        let processed = self.post_process(&markdown);
        Ok(processed)
    }

    /// Post-process the markdown output.
    fn post_process(&self, markdown: &str) -> String {
        let mut result = markdown.to_string();
        
        // Remove excessive newlines
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }
        
        // Trim whitespace at the start and end
        result = result.trim().to_string();
        
        result
    }

    /// Make relative links absolute.
    fn make_links_absolute(&self, markdown: &str, base_url: &str) -> String {
        // Simple regex to find markdown links
        let re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
        
        re.replace_all(markdown, |caps: &regex::Captures| {
            let text = &caps[1];
            let url = &caps[2];
            
            // Skip if already absolute
            if url.starts_with("http://") || url.starts_with("https://") {
                return format!("[{}]({})", text, url);
            }
            
            // Make absolute
            if url.starts_with('/') {
                // Absolute path
                if let Ok(base) = url::Url::parse(base_url) {
                    if let Some(origin) = base.host_str() {
                        return format!("[{}]({}://{}{})", text, base.scheme(), origin, url);
                    }
                }
            } else {
                // Relative path
                if let Ok(base) = url::Url::parse(base_url) {
                    if let Ok(full_url) = base.join(url) {
                        return format!("[{}]({})", text, full_url);
                    }
                }
            }
            
            // Fallback to original
            format!("[{}]({})", text, url)
        }).to_string()
    }

    /// Clean HTML for text extraction (remove scripts, styles, etc.).
    pub fn clean_html(html: &str) -> String {
        let mut result = html.to_string();
        
        // Remove script tags and content
        let script_re = regex::Regex::new(r"<script[^>]*>[\s\S]*?</script>").unwrap();
        result = script_re.replace_all(&result, "").to_string();
        
        // Remove style tags and content
        let style_re = regex::Regex::new(r"<style[^>]*>[\s\S]*?</style>").unwrap();
        result = style_re.replace_all(&result, "").to_string();
        
        // Remove HTML comments
        let comment_re = regex::Regex::new(r"<!--[\s\S]*?-->").unwrap();
        result = comment_re.replace_all(&result, "").to_string();
        
        // Remove nav elements
        let nav_re = regex::Regex::new(r"<nav[^>]*>[\s\S]*?</nav>").unwrap();
        result = nav_re.replace_all(&result, "").to_string();
        
        // Remove header elements (but not h1-h6)
        let header_re = regex::Regex::new(r"<header[^>]*>[\s\S]*?</header>").unwrap();
        result = header_re.replace_all(&result, "").to_string();
        
        // Remove footer elements
        let footer_re = regex::Regex::new(r"<footer[^>]*>[\s\S]*?</footer>").unwrap();
        result = footer_re.replace_all(&result, "").to_string();
        
        result
    }
}

impl Default for HtmlToMarkdown {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_basic() {
        let html = "<h1>Title</h1><p>This is <strong>bold</strong> text.</p>";
        let converter = HtmlToMarkdown::new();
        let md = converter.convert(html).unwrap();
        
        assert!(md.contains("Title"));
        assert!(md.contains("bold"));
    }

    #[test]
    fn test_clean_html() {
        let html = r#"
            <html>
            <head><script>var x = 1;</script></head>
            <body>
                <nav>Navigation</nav>
                <main>Content</main>
                <footer>Footer</footer>
            </body>
            </html>
        "#;
        
        let cleaned = HtmlToMarkdown::clean_html(html);
        assert!(!cleaned.contains("script"));
        assert!(!cleaned.contains("Navigation"));
        assert!(cleaned.contains("Content"));
    }
}
