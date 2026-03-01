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
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Docs MCP Server.
pub struct DocsMcpServer {
    config: Arc<Config>,
    connection: Arc<Connection>,
    embedder: Arc<RwLock<Box<dyn Embedder>>>,
    #[allow(dead_code)]
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
            tool_router: Self::tool_router(),
        })
    }

    /// Create a new docs MCP server with shared resources.
    pub fn new_shared(
        config: Arc<Config>,
        connection: Arc<Connection>,
        embedder: Arc<RwLock<Box<dyn Embedder>>>,
    ) -> Self {
        Self {
            config,
            connection,
            embedder,
            tool_router: Self::tool_router(),
        }
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

    /// Create a StreamableHttpService for HTTP transport.
    pub fn create_http_service(
        config: Arc<Config>,
        connection: Arc<Connection>,
        embedder: Arc<RwLock<Box<dyn Embedder>>>,
    ) -> StreamableHttpService<Self, LocalSessionManager> {
        let http_config = StreamableHttpServerConfig {
            sse_keep_alive: Some(std::time::Duration::from_secs(15)),
            sse_retry: Some(std::time::Duration::from_secs(3)),
            stateful_mode: true,
            json_response: false,
            ..Default::default()
        };

        StreamableHttpService::new(
            move || {
                Ok(Self::new_shared(
                    config.clone(),
                    connection.clone(),
                    embedder.clone(),
                ))
            },
            Arc::new(LocalSessionManager::default()),
            http_config,
        )
    }

    /// Get the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the database connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

#[tool_router(router = tool_router)]
impl DocsMcpServer {
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
