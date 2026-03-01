//! HTML parsing and link extraction.

use crate::core::{Error, Result};
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

/// A extracted link.
#[derive(Debug, Clone)]
pub struct Link {
    /// The link URL.
    pub url: String,
    /// The link text.
    pub text: String,
    /// Whether the link is internal (same domain).
    pub is_internal: bool,
}

/// HTML parser for content extraction.
pub struct HtmlParser {
    link_selector: Selector,
    title_selector: Selector,
    meta_description_selector: Selector,
}

impl HtmlParser {
    /// Create a new HTML parser.
    pub fn new() -> Self {
        Self {
            link_selector: Selector::parse("a[href]").expect("Invalid selector"),
            title_selector: Selector::parse("title").expect("Invalid selector"),
            meta_description_selector: Selector::parse("meta[name=\"description\"]")
                .expect("Invalid selector"),
        }
    }

    /// Parse HTML document.
    pub fn parse(&self, html: &str) -> Html {
        Html::parse_document(html)
    }

    /// Extract the title from HTML.
    pub fn extract_title(&self, document: &Html) -> Option<String> {
        document
            .select(&self.title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
    }

    /// Extract meta description.
    pub fn extract_description(&self, document: &Html) -> Option<String> {
        document
            .select(&self.meta_description_selector)
            .next()
            .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
    }

    /// Extract main content from HTML (excluding navigation, header, footer, etc.).
    pub fn extract_main_content(&self, document: &Html) -> String {
        // Try to find main content areas
        let main_selectors = [
            Selector::parse("main").ok(),
            Selector::parse("article").ok(),
            Selector::parse("[role=\"main\"]").ok(),
            Selector::parse(".content").ok(),
            Selector::parse(".documentation").ok(),
            Selector::parse(".docs").ok(),
            Selector::parse("#content").ok(),
            Selector::parse("#main").ok(),
        ];

        for selector in main_selectors.iter().flatten() {
            if let Some(el) = document.select(selector).next() {
                return el.text().collect::<String>();
            }
        }

        // Fallback: extract body text
        if let Ok(body_selector) = Selector::parse("body") {
            if let Some(body) = document.select(&body_selector).next() {
                return body.text().collect::<String>();
            }
        }

        // Last resort: all text
        document.root_element().text().collect()
    }

    /// Extract links from HTML.
    pub fn extract_links(&self, document: &Html, base_url: &str) -> Result<Vec<Link>> {
        let base = Url::parse(base_url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid base URL: {}", e)))?;

        let mut links = Vec::new();
        let mut seen = HashSet::new();

        for element in document.select(&self.link_selector) {
            if let Some(href) = element.value().attr("href") {
                // Skip empty, javascript, and anchor links
                if href.is_empty() || href.starts_with("javascript:") || href.starts_with('#') {
                    continue;
                }

                // Resolve relative URLs
                let resolved_url = if href.starts_with("http://") || href.starts_with("https://") {
                    href.to_string()
                } else if href.starts_with('/') {
                    format!(
                        "{}://{}{}",
                        base.scheme(),
                        base.host_str().unwrap_or(""),
                        href
                    )
                } else {
                    // Relative URL
                    match base.join(href) {
                        Ok(url) => url.to_string(),
                        Err(_) => continue,
                    }
                };

                // Skip duplicates
                if seen.contains(&resolved_url) {
                    continue;
                }
                seen.insert(resolved_url.clone());

                // Check if internal
                let is_internal = self.is_internal_link(&resolved_url, &base);

                // Get link text
                let text = element.text().collect::<String>().trim().to_string();

                links.push(Link {
                    url: resolved_url,
                    text,
                    is_internal,
                });
            }
        }

        Ok(links)
    }

    /// Check if a URL is internal (same domain).
    fn is_internal_link(&self, url: &str, base: &Url) -> bool {
        if let Ok(parsed) = Url::parse(url) {
            parsed.host_str() == base.host_str()
        } else {
            false
        }
    }

    /// Filter links to only include those matching patterns.
    pub fn filter_links(
        links: &[Link],
        include_patterns: &[Regex],
        exclude_patterns: &[Regex],
    ) -> Vec<Link> {
        links
            .iter()
            .filter(|link| {
                // Check exclude patterns first
                if exclude_patterns.iter().any(|p| p.is_match(&link.url)) {
                    return false;
                }

                // If include patterns are specified, check them
                if !include_patterns.is_empty() {
                    return include_patterns.iter().any(|p| p.is_match(&link.url));
                }

                true
            })
            .cloned()
            .collect()
    }
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let html = r#"<html><head><title>Test Page</title></head><body></body></html>"#;
        let parser = HtmlParser::new();
        let doc = parser.parse(html);

        let title = parser.extract_title(&doc);
        assert_eq!(title, Some("Test Page".to_string()));
    }

    #[test]
    fn test_extract_links() {
        let html = r#"
            <html><body>
                <a href="https://example.com/page1">Page 1</a>
                <a href="/page2">Page 2</a>
                <a href="https://other.com/page3">External</a>
                <a href="javascript:void(0)">Skip</a>
            </body></html>
        "#;
        let parser = HtmlParser::new();
        let doc = parser.parse(html);

        let links = parser.extract_links(&doc, "https://example.com/").unwrap();
        assert_eq!(links.len(), 3);

        assert!(
            links
                .iter()
                .any(|l| l.url == "https://example.com/page1" && l.is_internal)
        );
        assert!(
            links
                .iter()
                .any(|l| l.url == "https://example.com/page2" && l.is_internal)
        );
        assert!(
            links
                .iter()
                .any(|l| l.url == "https://other.com/page3" && !l.is_internal)
        );
    }
}
