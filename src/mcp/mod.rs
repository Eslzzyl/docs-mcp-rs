//! MCP (Model Context Protocol) server implementation.

mod server;
mod tools;

pub use server::DocsMcpServer;
pub use tools::{ListLibrariesParams, RemoveLibraryParams, ScrapeDocsParams, SearchDocsParams};
