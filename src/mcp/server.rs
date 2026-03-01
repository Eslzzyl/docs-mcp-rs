//! MCP server implementation.

use crate::core::{Config, Error, Result};
use crate::embed::{Embedder, create_embedder};
use crate::mcp::tools::{
    self, ListLibrariesParams, RemoveLibraryParams, ScrapeDocsParams, SearchDocsParams,
};
use crate::store::Connection;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_router,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Docs MCP Server.
pub struct DocsMcpServer {
    config: Arc<Config>,
    connection: Arc<Connection>,
    embedder: Arc<RwLock<Box<dyn Embedder>>>,
    tool_router: ToolRouter<Self>,
}

impl DocsMcpServer {
    /// Create a new docs MCP server.
    pub fn new(config: Config, connection: Connection) -> Result<Self> {
        let config = Arc::new(config);
        let connection = Arc::new(connection);
        let embedder = create_embedder(&config.embedding)?;

        Ok(Self {
            config,
            connection,
            embedder: Arc::new(RwLock::new(embedder)),
            tool_router: Default::default(),
        })
    }

    /// Start the MCP server with stdio transport.
    pub async fn run(self) -> Result<()> {
        use rmcp::ServiceExt;
        use tokio::io::{stdin, stdout};

        let transport = (stdin(), stdout());
        let server = self
            .serve(transport)
            .await
            .map_err(|e| Error::Mcp(e.to_string()))?;

        server
            .waiting()
            .await
            .map_err(|e| Error::Mcp(e.to_string()))?;

        Ok(())
    }

    /// Get the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the database connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    #[tool(description = "Scrape and index a documentation website")]
    async fn scrape_docs(&self, Parameters(params): Parameters<ScrapeDocsParams>) -> String {
        let embedder = self.embedder.read().await;
        let result = tools::scrape_docs(&self.connection, &**embedder, params).await;
        match result {
            Ok(r) => r
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(t) => t.text,
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Search indexed documentation using semantic search")]
    async fn search_docs(&self, Parameters(params): Parameters<SearchDocsParams>) -> String {
        let embedder = self.embedder.read().await;
        let result = tools::search_docs(&self.connection, &**embedder, params).await;
        match result {
            Ok(r) => r
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(t) => t.text,
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "List all indexed documentation libraries")]
    async fn list_libraries(&self, Parameters(_): Parameters<ListLibrariesParams>) -> String {
        let result = tools::list_libraries(&self.connection).await;
        match result {
            Ok(r) => r
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(t) => t.text,
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Remove an indexed library")]
    async fn remove_library(&self, Parameters(params): Parameters<RemoveLibraryParams>) -> String {
        let result = tools::remove_library(&self.connection, params).await;
        match result {
            Ok(r) => r
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(t) => t.text,
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("Error: {}", e),
        }
    }
}

#[tool_router]
impl DocsMcpServer {}

impl ServerHandler for DocsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "This server provides tools for scraping and searching documentation websites."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
