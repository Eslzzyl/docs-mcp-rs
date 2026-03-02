//! Content scraping module.

mod client;
mod converter;
mod crawler;
mod fetcher;
mod parser;

pub use client::HttpClient;
pub use converter::HtmlToMarkdown;
pub use crawler::{CrawlConfig, CrawlProgress, CrawlResult, Crawler, ProgressCallback};
pub use fetcher::{FetchResult, Fetcher};
pub use parser::{HtmlParser, Link};
