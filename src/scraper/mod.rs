//! Content scraping module.

mod client;
mod fetcher;
mod parser;
mod converter;
mod crawler;

pub use client::HttpClient;
pub use fetcher::{Fetcher, FetchResult};
pub use parser::{HtmlParser, Link};
pub use converter::HtmlToMarkdown;
pub use crawler::{Crawler, CrawlConfig, CrawlResult};
