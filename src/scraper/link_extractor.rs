//! Link extraction from HTML.

use scraper::{Html, Selector};
use std::collections::HashSet;
use tracing::{debug, trace};

/// An extracted link.
#[derive(Debug, Clone)]
pub struct Link {
    /// The link URL.
    pub url: String,
    /// The link text.
    pub text: String,
    /// Whether the link is internal (same domain).
    pub is_internal: bool,
}

/// Link extractor for HTML documents.
pub struct LinkExtractor;

impl LinkExtractor {
    /// Extract links from HTML.
    pub fn extract(html: &str, base_url: &str) -> Vec<Link> {
        let document = Html::parse_document(html);
        let selector = Selector::parse("a[href]").expect("Invalid selector");

        let base = url::Url::parse(base_url).ok();
        let base_domain = base.as_ref().and_then(|u| u.host_str());

        let mut links = Vec::new();
        let mut seen = HashSet::new();
        let mut skipped_count = 0;

        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                // Skip empty, anchor, javascript, mailto, tel links
                if Self::should_skip_href(href) {
                    skipped_count += 1;
                    continue;
                }

                // Resolve URL
                let resolved = Self::resolve_url(href, base_url, &base);

                // Skip duplicates
                if !seen.insert(resolved.clone()) {
                    continue;
                }

                // Determine if internal
                let is_internal = if let Some(domain) = base_domain {
                    url::Url::parse(&resolved)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h == domain))
                        .unwrap_or(false)
                } else {
                    false
                };

                // Get link text
                let text = element.text().collect::<String>().trim().to_string();

                trace!(
                    "Found link: {} -> {} (internal: {}, text: '{}')",
                    href, resolved, is_internal, text
                );

                links.push(Link {
                    url: resolved,
                    text,
                    is_internal,
                });
            }
        }

        debug!(
            "Link extraction complete: {} unique links found ({} skipped)",
            links.len(),
            skipped_count
        );

        links
    }

    /// Check if an href should be skipped.
    fn should_skip_href(href: &str) -> bool {
        let href = href.trim();

        if href.is_empty() {
            return true;
        }

        if href.starts_with('#') {
            return true;
        }

        if href.starts_with("javascript:") {
            return true;
        }

        if href.starts_with("mailto:") {
            return true;
        }

        if href.starts_with("tel:") {
            return true;
        }

        if href.starts_with("data:") {
            return true;
        }

        false
    }

    /// Resolve a URL relative to a base URL.
    fn resolve_url(href: &str, base_url: &str, base: &Option<url::Url>) -> String {
        // Already absolute
        if href.starts_with("http://") || href.starts_with("https://") {
            return href.to_string();
        }

        // Use pre-parsed base if available
        if let Some(base) = base {
            return base
                .join(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string());
        }

        // Parse base URL on the fly
        url::Url::parse(base_url)
            .and_then(|b| b.join(href))
            .map(|u| u.to_string())
            .unwrap_or_else(|_| href.to_string())
    }
}

impl Default for LinkExtractor {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic_links() {
        let html = "<html><body>\
                <a href=\"https://example.com/page1\">Page 1</a>\
                <a href=\"/page2\">Page 2</a>\
                <a href=\"page3\">Page 3</a>\
            </body></html>";

        let links = LinkExtractor::extract(html, "https://example.com/");

        assert_eq!(links.len(), 3);
        assert!(links.iter().any(|l| l.url == "https://example.com/page1"));
        assert!(links.iter().any(|l| l.url == "https://example.com/page2"));
        assert!(links.iter().any(|l| l.url == "https://example.com/page3"));
    }

    #[test]
    fn test_skip_unwanted_links() {
        let html = "<html><body>\
                <a href=\"#section\">Anchor</a>\
                <a href=\"javascript:void(0)\">JS</a>\
                <a href=\"mailto:test@example.com\">Email</a>\
                <a href=\"tel:+123\">Phone</a>\
                <a href=\"data:text/plain,test\">Data</a>\
                <a href=\"\">Empty</a>\
                <a href=\"valid\">Valid Link</a>\
            </body></html>";

        let links = LinkExtractor::extract(html, "https://example.com/");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/valid");
    }

    #[test]
    fn test_internal_external_detection() {
        let html = "<html><body>\
                <a href=\"/internal\">Internal</a>\
                <a href=\"https://example.com/page\">Same Domain</a>\
                <a href=\"https://other.com/page\">External</a>\
            </body></html>";

        let links = LinkExtractor::extract(html, "https://example.com/");

        assert_eq!(links.len(), 3);

        let internal = links.iter().find(|l| l.url.contains("/internal")).unwrap();
        assert!(internal.is_internal);

        let same_domain = links
            .iter()
            .find(|l| l.url == "https://example.com/page")
            .unwrap();
        assert!(same_domain.is_internal);

        let external = links
            .iter()
            .find(|l| l.url == "https://other.com/page")
            .unwrap();
        assert!(!external.is_internal);
    }

    #[test]
    fn test_deduplication() {
        let html = "<html><body>\
                <a href=\"/page\">Link 1</a>\
                <a href=\"/page\">Link 2</a>\
                <a href=\"/page\">Link 3</a>\
            </body></html>";

        let links = LinkExtractor::extract(html, "https://example.com/");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/page");
    }

    #[test]
    fn test_link_text() {
        let html = "<html><body>\
                <a href=\"/page\">Click Here</a>\
            </body></html>";

        let links = LinkExtractor::extract(html, "https://example.com/");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "Click Here");
    }
}
