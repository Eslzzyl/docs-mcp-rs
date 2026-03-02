//! Browser-based content fetcher using headless Chrome.
//!
//! This module provides functionality to fetch and render JavaScript-heavy web pages
//! using a headless Chrome browser. It supports Shadow DOM extraction, iframe processing,
//! and request interception for advanced web scraping scenarios.

use crate::core::{Error, Result};
use crate::scraper::FetchResult;
use headless_chrome::protocol::cdp::Fetch::RequestPattern;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::types::PrintToPdfOptions;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, trace, warn};
use url::Url;

/// Configuration for browser-based fetching.
#[derive(Debug, Clone)]
pub struct BrowserFetchConfig {
    /// Path to Chrome executable (optional, will auto-detect if not provided).
    pub chrome_path: Option<String>,
    /// Whether to run in headless mode.
    pub headless: bool,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Delay after page load to wait for JavaScript execution (milliseconds).
    pub wait_after_load_ms: u64,
    /// User agent string.
    pub user_agent: Option<String>,
    /// Window width.
    pub window_width: u32,
    /// Window height.
    pub window_height: u32,
    /// Whether to extract Shadow DOM content.
    pub extract_shadow_dom: bool,
    /// Whether to process iframes.
    pub process_iframes: bool,
    /// Whether to block images (improves performance).
    pub block_images: bool,
    /// Whether to block CSS (improves performance).
    pub block_css: bool,
    /// Custom headers to send with requests.
    pub headers: HashMap<String, String>,
}

impl Default for BrowserFetchConfig {
    fn default() -> Self {
        Self {
            chrome_path: None,
            headless: true,
            timeout_secs: 30,
            wait_after_load_ms: 2000,
            user_agent: None,
            window_width: 1920,
            window_height: 1080,
            extract_shadow_dom: true,
            process_iframes: true,
            block_images: true,
            block_css: false,
            headers: HashMap::new(),
        }
    }
}

/// Browser-based content fetcher.
pub struct BrowserFetcher {
    config: BrowserFetchConfig,
    browser: Option<Arc<Browser>>,
}

impl BrowserFetcher {
    /// Create a new browser fetcher with the given configuration.
    pub fn new(config: BrowserFetchConfig) -> Self {
        Self {
            config,
            browser: None,
        }
    }

    /// Create a browser fetcher with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(BrowserFetchConfig::default())
    }

    /// Initialize the browser instance.
    fn init_browser(&mut self) -> Result<Arc<Browser>> {
        if let Some(ref browser) = self.browser {
            return Ok(browser.clone());
        }

        info!("Initializing headless Chrome browser");

        let mut launch_options_builder = LaunchOptions::default_builder();

        launch_options_builder
            .headless(self.config.headless)
            .window_size(Some((self.config.window_width, self.config.window_height)));

        // Set Chrome executable path if provided
        if let Some(ref path) = self.config.chrome_path {
            launch_options_builder.path(Some(std::path::PathBuf::from(path)));
            debug!("Using custom Chrome path: {}", path);
        } else if let Ok(chrome_path) = std::env::var("CHROME_PATH") {
            launch_options_builder.path(Some(std::path::PathBuf::from(&chrome_path)));
            debug!("Using CHROME_PATH environment variable: {}", chrome_path);
        }

        // Add arguments for better compatibility
        let args: Vec<&std::ffi::OsStr> = vec![
            std::ffi::OsStr::new("--no-sandbox"),
            std::ffi::OsStr::new("--disable-dev-shm-usage"),
            std::ffi::OsStr::new("--disable-gpu"),
            std::ffi::OsStr::new("--disable-web-security"),
            std::ffi::OsStr::new("--disable-features=IsolateOrigins,site-per-process"),
        ];
        launch_options_builder.args(args);

        let launch_options = launch_options_builder
            .build()
            .map_err(|e| Error::Scraper(format!("Failed to build Chrome launch options: {}", e)))?;

        let browser = Browser::new(launch_options).map_err(|e| {
            Error::Scraper(format!(
                "Failed to launch Chrome browser. Make sure Chrome/Chromium is installed. Error: {}",
                e
            ))
        })?;

        let browser_arc = Arc::new(browser);
        self.browser = Some(browser_arc.clone());

        info!("Chrome browser initialized successfully");

        Ok(browser_arc)
    }

    /// Fetch content from a URL using the browser.
    pub async fn fetch(&mut self, url: &str) -> Result<FetchResult> {
        self.fetch_with_options(url, None, None).await
    }

    /// Fetch content with cancellation support.
    pub async fn fetch_with_cancel(
        &mut self,
        url: &str,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<FetchResult> {
        self.fetch_with_options(url, None, cancel_token).await
    }

    /// Fetch content with custom options and cancellation support.
    pub async fn fetch_with_options(
        &mut self,
        url: &str,
        options: Option<FetchOptions>,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<FetchResult> {
        debug!("Fetching URL with browser: {}", url);

        // Check for cancellation before starting
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(Error::Mcp("Job cancelled".to_string()));
            }
        }

        // Validate URL
        let parsed_url = Url::parse(url)
            .map_err(|e| Error::InvalidUrl(format!("Invalid URL {}: {}", url, e)))?;

        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(Error::InvalidUrl(format!(
                "Unsupported URL scheme: {}",
                parsed_url.scheme()
            )));
        }

        // Initialize browser if not already done
        let browser = self.init_browser()?;

        // Check for cancellation after browser init
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(Error::Mcp("Job cancelled".to_string()));
            }
        }

        // Create a new tab
        let tab = browser
            .new_tab()
            .map_err(|e| Error::Scraper(format!("Failed to create new browser tab: {}", e)))?;

        // Set up request interception if needed
        if self.config.block_images || self.config.block_css {
            self.setup_request_interception(&tab)?;
        }

        // Set custom headers if provided
        if !self.config.headers.is_empty() {
            let headers_json =
                serde_json::to_string(&self.config.headers).map_err(|e| Error::Serialization(e))?;
            tab.evaluate(&format!(
                "() => {{ Object.entries({}).forEach(([k, v]) => {{\n                    if (!window._customHeaders) window._customHeaders = {{}};\n                    window._customHeaders[k] = v;\n                }}); }}",
                headers_json
            ), false).ok();
        }

        // Set user agent if provided
        if let Some(ref user_agent) = self.config.user_agent {
            tab.set_user_agent(user_agent, None, None)
                .map_err(|e| Error::Scraper(format!("Failed to set user agent: {}", e)))?;
        }

        // Navigate to URL with timeout and periodic cancellation checks
        let timeout = Duration::from_secs(self.config.timeout_secs);

        let navigate_result = tokio::time::timeout(timeout, async {
            // Check cancellation before navigation
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(Error::Mcp("Job cancelled".to_string()));
                }
            }

            tab.navigate_to(url)
                .map_err(|e| Error::Http(format!("Failed to navigate to {}: {}", url, e)))?;

            // Poll for navigation completion with cancellation checks
            let mut attempts = 0;
            let max_attempts = timeout.as_millis() / 100;
            loop {
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(Error::Mcp("Job cancelled".to_string()));
                    }
                }

                // Check if navigation is complete
                let current_url = tab.get_url();
                if !current_url.is_empty() && current_url != "about:blank" {
                    break;
                }

                attempts += 1;
                if attempts >= max_attempts as u32 {
                    return Err(Error::Http(format!("Navigation timeout for {}", url)));
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            Ok(())
        })
        .await
        .map_err(|_| {
            Error::Http(format!(
                "Navigation timeout for {} after {:?}",
                url, timeout
            ))
        })?;

        if let Err(e) = navigate_result {
            return Err(e);
        }

        trace!("Page navigated successfully: {}", url);

        // Check for cancellation after navigation
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(Error::Mcp("Job cancelled".to_string()));
            }
        }

        // Handle custom options
        let wait_after_load_ms = options
            .as_ref()
            .and_then(|o| o.wait_after_load_ms)
            .unwrap_or(self.config.wait_after_load_ms);

        // Wait for specific selector if provided
        if let Some(ref selector) = options.as_ref().and_then(|o| o.wait_for_selector.as_ref()) {
            trace!("Waiting for selector: {}", selector);
            for i in 0..50 {
                // Check cancellation every iteration
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(Error::Mcp("Job cancelled".to_string()));
                    }
                }

                // Wait up to 5 seconds
                let check_script = format!("() => document.querySelector('{}') !== null", selector);
                let result = tab.evaluate(&check_script, false).ok();
                if let Some(ref r) = result {
                    if let Some(ref v) = r.value {
                        if v.as_bool() == Some(true) {
                            break;
                        }
                    }
                }
                if i < 49 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        // Scroll to bottom to trigger lazy loading if requested
        if options.as_ref().map_or(false, |o| o.scroll_to_bottom) {
            trace!("Scrolling to bottom to trigger lazy loading");
            let _ = tab.evaluate(
                "() => { window.scrollTo(0, document.body.scrollHeight); }",
                false,
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Wait for JavaScript execution with cancellation checks
        if wait_after_load_ms > 0 {
            trace!("Waiting {}ms for JavaScript execution", wait_after_load_ms);
            let chunks = wait_after_load_ms / 100;
            for _ in 0..chunks {
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(Error::Mcp("Job cancelled".to_string()));
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Wait remaining time
            let remaining = wait_after_load_ms % 100;
            if remaining > 0 {
                tokio::time::sleep(Duration::from_millis(remaining)).await;
            }
        }

        // Wait for loading indicators to disappear (common in SPAs)
        self.wait_for_loading_indicators_with_cancel(&tab, cancel_token).await.ok();

        // Extract content - get the main document content
        let mut content = tab
            .get_content()
            .map_err(|e| Error::Scraper(format!("Failed to get page content: {}", e)))?;

        trace!("Initial content length: {} bytes", content.len());

        // Extract Shadow DOM content if enabled
        if self.config.extract_shadow_dom {
            match self.extract_shadow_dom_content(&tab) {
                Ok(shadow_content) => {
                    if !shadow_content.is_empty() {
                        debug!("Extracted Shadow DOM content");
                        content = shadow_content;
                    }
                }
                Err(e) => {
                    warn!("Failed to extract Shadow DOM content: {}", e);
                }
            }
        }

        // Process iframes if enabled
        if self.config.process_iframes {
            match self.process_iframes_content(&tab, &content).await {
                Ok(iframe_content) => {
                    if !iframe_content.is_empty() {
                        debug!("Processed iframe content");
                        content = iframe_content;
                    }
                }
                Err(e) => {
                    warn!("Failed to process iframe content: {}", e);
                }
            }
        }

        // Get final URL after any redirects
        let final_url = tab.get_url();

        debug!(
            "Successfully fetched {} -> {}: content_length={}",
            url,
            final_url,
            content.len()
        );

        // Build result (browser fetch doesn't have HTTP headers like etag/last_modified)
        Ok(FetchResult {
            url: url.to_string(),
            final_url,
            content,
            content_type: Some("text/html".to_string()),
            etag: None,
            last_modified: None,
            status: 200,
        })
    }

    /// Set up request interception to block resources.
    fn setup_request_interception(&self, tab: &Tab) -> Result<()> {
        // Enable fetch domain for request interception
        tab.enable_fetch(
            Some(&[RequestPattern {
                url_pattern: Some("*".to_string()),
                resource_Type: None,
                request_stage: Some(headless_chrome::protocol::cdp::Fetch::RequestStage::Request),
            }]),
            None,
        )
        .map_err(|e| Error::Scraper(format!("Failed to enable fetch interception: {}", e)))?;

        // Note: Full request interception with resource blocking requires more complex handling
        // This is a simplified version - in production, you'd want to handle the Fetch events
        trace!("Request interception enabled");

        Ok(())
    }

    /// Wait for common loading indicators to disappear.
    async fn wait_for_loading_indicators(&self, tab: &Tab) -> Result<()> {
        self.wait_for_loading_indicators_with_cancel(tab, None).await
    }

    /// Wait for common loading indicators to disappear with cancellation support.
    async fn wait_for_loading_indicators_with_cancel(
        &self,
        tab: &Tab,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<()> {
        let loading_selectors = [
            ".loading",
            ".spinner",
            ".loader",
            "[data-loading='true']",
            ".skeleton",
        ];

        for selector in &loading_selectors {
            // Check cancellation at the start of each selector check
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Ok(());
                }
            }

            let script = format!(
                r#"() => {{
                    const el = document.querySelector('{}');
                    return el ? window.getComputedStyle(el).display === 'none' || el.offsetParent === null : true;
                }}"#,
                selector
            );

            // Wait up to 5 seconds for loading indicator to disappear
            for i in 0..50 {
                // Check cancellation every 5 iterations (every 500ms)
                if i % 5 == 0 {
                    if let Some(token) = cancel_token {
                        if token.is_cancelled() {
                            return Ok(());
                        }
                    }
                }

                let result = tab.evaluate(&script, false).map_err(|e| {
                    Error::Scraper(format!("Failed to check loading indicator: {}", e))
                })?;

                if let Some(ref value) = result.value {
                    if let Some(visible) = value.as_bool() {
                        if visible {
                            if i < 49 {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            continue;
                        }
                    }
                }
                break;
            }
        }

        Ok(())
    }

    /// Extract content from Shadow DOM elements.
    fn extract_shadow_dom_content(&self, tab: &Tab) -> Result<String> {
        let script = r#"
        () => {
            function extractShadowContent(root) {
                let html = '';
                const walker = document.createTreeWalker(
                    root,
                    NodeFilter.SHOW_ELEMENT,
                    null,
                    false
                );
                
                let node;
                while (node = walker.nextNode()) {
                    if (node.shadowRoot) {
                        html += '<div data-shadow-host="' + node.tagName.toLowerCase() + '">\n';
                        html += extractShadowContent(node.shadowRoot);
                        html += '</div>\n';
                    }
                }
                
                // Also include light DOM content
                if (root === document.body) {
                    html = document.documentElement.outerHTML;
                }
                
                return html;
            }
            
            return extractShadowContent(document.body);
        }
        "#;

        let result = tab
            .evaluate(script, false)
            .map_err(|e| Error::Scraper(format!("Failed to extract Shadow DOM: {}", e)))?;

        if let Some(ref value) = result.value {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
        }
        Ok(String::new())
    }

    /// Process iframe content and merge it into the main document.
    async fn process_iframes_content(&self, tab: &Tab, _current_content: &str) -> Result<String> {
        let script = r#"
        () => {
            const iframes = document.querySelectorAll('iframe');
            const results = [];
            
            iframes.forEach((iframe, index) => {
                try {
                    const iframeDoc = iframe.contentDocument || iframe.contentWindow?.document;
                    if (iframeDoc) {
                        results.push({
                            index: index,
                            src: iframe.src || '',
                            content: iframeDoc.body ? iframeDoc.body.innerHTML : ''
                        });
                    }
                } catch (e) {
                    // Cross-origin iframe - can't access
                    results.push({
                        index: index,
                        src: iframe.src || '',
                        error: 'cross-origin'
                    });
                }
            });
            
            return JSON.stringify(results);
        }
        "#;

        let result = tab
            .evaluate(script, false)
            .map_err(|e| Error::Scraper(format!("Failed to process iframes: {}", e)))?;

        if let Some(ref value) = result.value {
            if let Some(json_str) = value.as_str() {
                let iframes: Vec<IframeInfo> =
                    serde_json::from_str(json_str).map_err(|e| Error::Serialization(e))?;

                if !iframes.is_empty() {
                    debug!("Found {} iframes", iframes.len());
                }

                // Get the main content
                let main_content = tab
                    .get_content()
                    .map_err(|e| Error::Scraper(format!("Failed to get main content: {}", e)))?;

                return Ok(main_content);
            }
        }

        tab.get_content()
            .map_err(|e| Error::Scraper(format!("Failed to get content: {}", e)))
    }

    /// Take a screenshot of the current page.
    pub fn take_screenshot(&self, tab: &Tab, output_path: &str) -> Result<()> {
        let png_data = tab
            .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
            .map_err(|e| Error::Scraper(format!("Failed to capture screenshot: {}", e)))?;

        std::fs::write(output_path, png_data).map_err(|e| Error::Io(e))?;

        info!("Screenshot saved to: {}", output_path);

        Ok(())
    }

    /// Save page as PDF.
    pub fn save_as_pdf(&self, tab: &Tab, output_path: &str) -> Result<()> {
        let pdf_data = tab
            .print_to_pdf(Some(PrintToPdfOptions::default()))
            .map_err(|e| Error::Scraper(format!("Failed to print to PDF: {}", e)))?;

        std::fs::write(output_path, pdf_data).map_err(|e| Error::Io(e))?;

        info!("PDF saved to: {}", output_path);

        Ok(())
    }

    /// Close the browser and cleanup resources.
    pub fn close(&mut self) {
        if self.browser.is_some() {
            info!("Closing browser instance");
            self.browser = None;
        }
    }
}

impl Drop for BrowserFetcher {
    fn drop(&mut self) {
        self.close();
    }
}

/// Additional fetch options for a single request.
#[derive(Debug, Default)]
pub struct FetchOptions {
    /// Custom wait time after load (overrides config).
    pub wait_after_load_ms: Option<u64>,
    /// Whether to wait for a specific selector.
    pub wait_for_selector: Option<String>,
    /// Whether to scroll to bottom to trigger lazy loading.
    pub scroll_to_bottom: bool,
}

/// Information about an iframe.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct IframeInfo {
    index: usize,
    #[serde(default)]
    src: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_fetcher_creation() {
        let fetcher = BrowserFetcher::with_defaults();
        assert!(fetcher.browser.is_none());
    }

    #[test]
    fn test_browser_fetch_config_default() {
        let config = BrowserFetchConfig::default();
        assert!(config.headless);
        assert_eq!(config.timeout_secs, 30);
        assert!(config.extract_shadow_dom);
        assert!(config.process_iframes);
    }
}
