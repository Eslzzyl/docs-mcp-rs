//! MCP (Model Context Protocol) server implementation.

mod server;
mod tools;

pub use server::DocsMcpServer;
pub use tools::{ListLibrariesParams, RemoveLibraryParams, ScrapeDocsParams, SearchDocsParams};

/// Type alias for the MCP HTTP service.
pub type McpHttpService = rmcp::transport::streamable_http_server::StreamableHttpService<
    DocsMcpServer,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
>;
