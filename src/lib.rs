//! docs-mcp-rs: A Rust implementation of docs-mcp-server
//!
//! This crate provides a Model Context Protocol (MCP) server for indexing
//! and searching documentation websites.

pub mod cli;
pub mod core;
pub mod embed;
pub mod events;
pub mod mcp;
pub mod pipeline;
pub mod scraper;
pub mod splitter;
pub mod store;
pub mod web;
