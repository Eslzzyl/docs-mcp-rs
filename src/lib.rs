//! docs-mcp-rs: A Rust implementation of docs-mcp-server
//!
//! This crate provides a Model Context Protocol (MCP) server for indexing
//! and searching documentation websites.

pub mod core;
pub mod store;
pub mod scraper;
pub mod splitter;
pub mod embed;
pub mod events;
pub mod pipeline;
pub mod mcp;
pub mod cli;
