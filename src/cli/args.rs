//! CLI argument definitions using clap.

use clap::{Parser, Subcommand};

/// docs-mcp-rs: A Rust MCP server for indexing and searching documentation.
#[derive(Parser, Debug)]
#[command(name = "docs-mcp-rs")]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to the database file.
    #[arg(short, long, global = true, default_value = "docs.db")]
    pub database: String,

    /// Embedding model to use (e.g., "openai:text-embedding-3-small" or "google:text-embedding-004").
    #[arg(short, long, global = true, default_value = "openai:text-embedding-3-small")]
    pub model: String,

    /// OpenAI API key (or set OPENAI_API_KEY env var).
    #[arg(long, global = true, env = "OPENAI_API_KEY")]
    pub openai_key: Option<String>,

    /// Google API key (or set GOOGLE_API_KEY env var).
    #[arg(long, global = true, env = "GOOGLE_API_KEY")]
    pub google_key: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the MCP server (stdio mode by default).
    Serve {
        /// Port for HTTP mode (if specified, runs as HTTP server instead of stdio).
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable MCP Streamable HTTP transport endpoint at /mcp.
        /// Only applicable when --port is specified.
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        mcp: bool,
    },

    /// Scrape and index documentation from a URL.
    Scrape {
        /// Library name.
        library: String,

        /// URL to scrape.
        url: String,

        /// Version of the library.
        #[arg(short = 'v', long)]
        version: Option<String>,

        /// Maximum pages to scrape.
        #[arg(short = 'p', long, default_value = "1000")]
        max_pages: usize,

        /// Maximum crawl depth.
        #[arg(short = 'd', long, default_value = "3")]
        max_depth: usize,

        /// Maximum concurrent requests.
        #[arg(short = 'c', long, default_value = "5")]
        concurrency: usize,
    },

    /// Search indexed documentation.
    Search {
        /// Library name.
        library: String,

        /// Search query.
        query: String,

        /// Version of the library.
        #[arg(short = 'v', long)]
        version: Option<String>,

        /// Maximum number of results.
        #[arg(short = 'l', long, default_value = "5")]
        limit: usize,
    },

    /// List all indexed libraries.
    List,

    /// Remove a library from the index.
    Remove {
        /// Library name.
        library: String,

        /// Version of the library (removes all versions if not specified).
        #[arg(short = 'v', long)]
        version: Option<String>,
    },
}
