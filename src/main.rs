//! docs-mcp-rs: A Rust implementation of docs-mcp-server
//!
//! A Model Context Protocol (MCP) server for indexing and searching documentation.

use docs_mcp_rs::core::Config;
use docs_mcp_rs::store::{Connection, run_migrations};

fn main() {
    println!("docs-mcp-rs v{}", env!("CARGO_PKG_VERSION"));
    
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = Config::default();
    
    // Initialize database
    match Connection::from_config(&config) {
        Ok(conn) => {
            tracing::info!("Database connection established");
            
            match run_migrations(&conn) {
                Ok(_) => tracing::info!("Database migrations completed"),
                Err(e) => tracing::error!("Migration error: {}", e),
            }
        }
        Err(e) => {
            tracing::error!("Failed to connect to database: {}", e);
        }
    }
}