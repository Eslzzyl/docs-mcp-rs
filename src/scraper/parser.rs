//! HTML parsing and link extraction.

use crate::core::{Error, Result};
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use tracing::{debug, trace};
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
        let title = document
            .select(&self.title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string());
        trace!("Extracted title: {:?}", title);
        title
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
            ("main", Selector::parse("main").ok()),
            ("article", Selector::parse("article").ok()),
            ("[role='main']", Selector::parse("[role=\"main\"]").ok()),
            (".content", Selector::parse(".content").ok()),
            (".documentation", Selector::parse(".documentation").ok()),
            (".docs", Selector::parse(".docs").ok()),
            ("#content", Selector::parse("#content").ok()),
            ("#main", Selector::parse("#main").ok()),
        ];

        for (name, selector) in main_selectors.iter() {
            if let Some(sel) = selector {
                if let Some(el) = document.select(sel).next() {
                    let content = el.text().collect::<String>();
                    debug!("Extracted main content using selector '{}': {} chars", name, content.len());
                    return content;
                }
            }
        }

        // Fallback: extract body text
        if let Ok(body_selector) = Selector::parse("body") {
            if let Some(body) = document.select(&body_selector).next() {
                let content = body.text().collect::<String>();
                debug!("Extracted main content from body: {} chars", content.len());
                return content;
            }
        }

        // Last resort: all text
        let content: String = document.root_element().text().collect();
        debug!("Extracted main content from root: {} chars", content.len());
        content
    }

    /// Extract links from HTML.
    pub fn extract_links(&self, document: &Html, base_url: &str) -> Result<Vec<Link>> {
        let base = Url::parse(base_url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid base URL: {}", e)))?;

        trace!("Extracting links from HTML with base URL: {}", base_url);

        let mut links = Vec::new();
        let mut seen = HashSet::new();
        let mut skipped_empty = 0;
        let mut skipped_javascript = 0;
        let mut skipped_anchor = 0;
        let mut skipped_duplicate = 0;

        for element in document.select(&self.link_selector) {
            if let Some(href) = element.value().attr("href") {
                // Skip empty, javascript, and anchor links
                if href.is_empty() {
                    skipped_empty += 1;
                    continue;
                }
                if href.starts_with("javascript:") {
                    skipped_javascript += 1;
                    continue;
                }
                if href.starts_with('#') {
                    skipped_anchor += 1;
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
                        Err(e) => {
                            trace!("Failed to resolve relative URL '{}': {}", href, e);
                            continue;
                        }
                    }
                };

                // Skip duplicates
                if seen.contains(&resolved_url) {
                    skipped_duplicate += 1;
                    continue;
                }
                seen.insert(resolved_url.clone());

                // Check if internal
                let is_internal = self.is_internal_link(&resolved_url, &base);

                // Get link text
                let text = element.text().collect::<String>().trim().to_string();

                trace!("Found link: {} -> {} (internal: {})", href, resolved_url, is_internal);

                links.push(Link {
                    url: resolved_url,
                    text,
                    is_internal,
                });
            }
        }

        debug!("Link extraction complete: {} unique links found (skipped: {} empty, {} javascript, {} anchors, {} duplicates)",
               links.len(), skipped_empty, skipped_javascript, skipped_anchor, skipped_duplicate);

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
