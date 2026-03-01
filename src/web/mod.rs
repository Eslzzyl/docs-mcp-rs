//! Web API module for docs-mcp-rs.
//!
//! Provides HTTP endpoints and a simple web UI for managing documentation.

mod handlers;
mod sse;

pub use handlers::{AppState, create_router, create_router_with_mcp};
