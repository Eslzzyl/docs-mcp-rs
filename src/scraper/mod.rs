//! Content scraping module.

mod browser_fetcher;
mod client;
mod converter;
mod crawler;
mod fetcher;
mod link_extractor;
mod parser;

pub use browser_fetcher::{
    BrowserFetchConfig, BrowserFetcher, BrowserPool, FetchOptions, TabFetcher,
};
pub use client::HttpClient;
pub use converter::{ConversionResult, HtmlToMarkdown};
pub use crawler::{CrawlConfig, CrawlProgress, CrawlResult, Crawler, ProgressCallback};
pub use fetcher::{FetchResult, Fetcher};
pub use link_extractor::{Link, LinkExtractor};
pub use parser::HtmlParser;
