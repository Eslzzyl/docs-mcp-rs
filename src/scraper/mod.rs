//! Content scraping module.

mod client;
mod converter;
mod crawler;
mod fetcher;
mod link_extractor;
mod parser;

pub use client::HttpClient;
pub use converter::{ConversionResult, HtmlToMarkdown};
pub use crawler::{CrawlConfig, CrawlProgress, CrawlResult, Crawler, ProgressCallback};
pub use fetcher::{FetchResult, Fetcher};
pub use link_extractor::{Link, LinkExtractor};
pub use parser::HtmlParser;
