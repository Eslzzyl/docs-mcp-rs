//! docs-mcp-rs: A Rust implementation of docs-mcp-server
//
//! A Model Context Protocol (MCP) server for indexing and searching documentation.

use clap::Parser;
use docs_mcp_rs::cli::{Cli, Commands};
use docs_mcp_rs::core::Config;
use docs_mcp_rs::core::config::{EmbeddingConfig, EmbeddingProvider};
use docs_mcp_rs::core::types::VersionStatus;
use docs_mcp_rs::embed::{Embedder, create_embedder};
use docs_mcp_rs::events::EventBus;
use docs_mcp_rs::mcp::DocsMcpServer;
use docs_mcp_rs::pipeline::{PipelineManager, ScraperOptions};
use docs_mcp_rs::store::{
    Connection, DocumentStore, LibraryStore, PageStore, VectorSearch, VersionStore, run_migrations,
};
use docs_mcp_rs::web::{AppState, create_router_with_mcp};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Build configuration
    let mut config = Config::default();

    // Parse embedding model
    // Format: "provider:model" (e.g., "openai:text-embedding-3-small")
    // If no provider prefix, defaults to "openai" and uses the whole string as model name
    let model_parts: Vec<&str> = cli.model.splitn(2, ':').collect();
    let (provider, model_id) = match model_parts.as_slice() {
        [provider, model] => (*provider, (*model).to_string()),
        [model] => {
            // No provider prefix, check if it looks like a provider name or a model name
            let m = (*model).to_lowercase();
            if m == "openai" || m == "google" {
                // It's just a provider name without model, use default model
                ((*model), "default".to_string())
            } else {
                // It's a model name without provider prefix, default to openai
                ("openai", (*model).to_string())
            }
        }
        _ => ("openai", "text-embedding-3-small".to_string()),
    };

    let embedding_provider = match provider.to_lowercase().as_str() {
        "google" => EmbeddingProvider::Google,
        _ => EmbeddingProvider::OpenAI,
    };

    config.embedding = EmbeddingConfig {
        provider: embedding_provider,
        openai_api_key: cli.openai_key.clone(),
        openai_api_base: cli.openai_base.clone(),
        openai_model: if provider == "openai" {
            model_id.clone()
        } else {
            "text-embedding-3-small".to_string()
        },
        google_api_key: cli.google_key.clone(),
        google_api_base: cli.google_base.clone(),
        google_model: if provider == "google" {
            model_id.clone()
        } else {
            "text-embedding-004".to_string()
        },
        ..Default::default()
    };

    // Initialize database
    let conn = match Connection::open(&cli.database) {
        Ok(conn) => {
            tracing::info!("Database connection established: {}", cli.database);
            conn
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    // Run migrations
    if let Err(e) = run_migrations(&conn) {
        eprintln!("❌ Migration error: {}", e);
        std::process::exit(1);
    }

    // Execute command
    let connection = Arc::new(conn);
    let result = match cli.command {
        Commands::Serve { port, stdio } => run_serve(connection, &config, port, stdio).await,
        Commands::Scrape {
            library,
            url,
            version,
            max_pages,
            max_depth,
            concurrency,
        } => {
            run_scrape(
                connection,
                &config,
                library,
                url,
                version,
                max_pages,
                max_depth,
                concurrency,
            )
            .await
        }
        Commands::Search {
            library,
            query,
            version,
            limit,
        } => run_search(connection, &config, library, query, version, limit).await,
        Commands::List => run_list(connection).await,
        Commands::Remove { library, version } => run_remove(connection, library, version).await,
    };

    if let Err(e) = result {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
}

/// Run the MCP server.
async fn run_serve(
    connection: Arc<Connection>,
    config: &Config,
    port: u16,
    stdio: bool,
) -> docs_mcp_rs::core::Result<()> {
    // Create embedder
    let embedder = create_embedder(&config.embedding)?;
    let embedder: Arc<RwLock<Box<dyn Embedder>>> = Arc::new(RwLock::new(embedder));

    // Create event bus
    let event_bus = EventBus::new();

    // Create pipeline manager
    let pipeline = Arc::new(PipelineManager::new(
        connection.clone(),
        embedder.clone(),
        event_bus.clone(),
        config.scraper.max_concurrency,
    ));

    // Start pipeline
    pipeline.start().await;

    if stdio {
        println!("🚀 MCP server starting in stdio mode...");

        // Create MCP server with pipeline
        let server = DocsMcpServer::new(config.clone(), (*connection).clone(), pipeline.clone())?;

        // Run stdio server
        server.run().await
    } else {
        println!("🚀 HTTP server starting on port {}", port);
        println!("📍 Web UI: http://localhost:{}", port);
        println!("📍 MCP HTTP: http://localhost:{}/mcp", port);

        // Create web app state
        let state = AppState {
            connection: connection.clone(),
            embedder: embedder.clone(),
            pipeline: pipeline.clone(),
            event_bus: event_bus.clone(),
        };

        // Create router with MCP endpoint
        let config_arc = Arc::new(config.clone());
        let mcp_service =
            DocsMcpServer::create_http_service(config_arc, connection.clone(), embedder.clone(), pipeline.clone());
        let app = create_router_with_mcp(state, mcp_service);

        // Create TCP listener
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;

        println!("✅ Server listening on http://{}", addr);
        println!("Press Ctrl+C to stop");

        // Run server
        axum::serve(listener, app)
            .await
            .map_err(|e| docs_mcp_rs::core::Error::Mcp(e.to_string()))?;

        // Stop pipeline on shutdown
        pipeline.stop().await;

        Ok(())
    }
}

/// Run the scrape command.
async fn run_scrape(
    connection: Arc<Connection>,
    config: &Config,
    library: String,
    url: String,
    version: Option<String>,
    max_pages: usize,
    max_depth: usize,
    concurrency: usize,
) -> docs_mcp_rs::core::Result<()> {
    println!("⏳ Scraping {}...", url);

    // Create embedder
    let embedder = create_embedder(&config.embedding)?;
    let embedder: Arc<RwLock<Box<dyn Embedder>>> = Arc::new(RwLock::new(embedder));

    // Create event bus
    let event_bus = EventBus::new();

    // Create pipeline manager
    let pipeline = PipelineManager::new(connection.clone(), embedder, event_bus, concurrency);

    // Start pipeline
    pipeline.start().await;

    // Enqueue job
    let options = ScraperOptions {
        max_pages: Some(max_pages),
        max_depth: Some(max_depth),
        ..Default::default()
    };

    let job_id = pipeline
        .enqueue(
            library.clone(),
            version.clone().unwrap_or_default(),
            url.clone(),
            options,
        )
        .await?;

    println!("📝 Job enqueued: {}", job_id);

    // Wait for completion
    pipeline.wait_for_job(&job_id).await?;

    // Get final job status
    if let Some(job) = pipeline.get_job(&job_id).await {
        match job.status {
            docs_mcp_rs::events::JobStatus::Completed => {
                println!("✅ Scraping completed successfully");
            }
            docs_mcp_rs::events::JobStatus::Failed => {
                eprintln!("❌ Scraping failed: {}", job.error.unwrap_or_default());
                return Err(docs_mcp_rs::core::Error::Mcp("Scraping failed".to_string()));
            }
            docs_mcp_rs::events::JobStatus::Cancelled => {
                println!("🚫 Scraping cancelled");
            }
            _ => {}
        }
    }

    // Stop pipeline
    pipeline.stop().await;

    Ok(())
}

/// Run the search command.
async fn run_search(
    connection: Arc<Connection>,
    config: &Config,
    library: String,
    query: String,
    version: Option<String>,
    limit: usize,
) -> docs_mcp_rs::core::Result<()> {
    println!("🔍 Searching for: {}", query);

    // Create embedder for query
    let embedder = create_embedder(&config.embedding)?;
    let embedding = embedder.embed(&query).await?;

    // Search
    let search = VectorSearch::with_options(
        &connection,
        docs_mcp_rs::store::SearchOptions {
            limit,
            ..Default::default()
        },
    );
    let results = search
        .search(&library, version.as_deref(), &embedding, &query)
        .await?;

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    println!("\nFound {} result(s):\n", results.len());

    for (i, result) in results.iter().enumerate() {
        println!("--- Result {} (score: {:.3}) ---", i + 1, result.score);
        if let Some(title) = &result.page.title {
            println!("Title: {}", title);
        }
        println!("URL: {}", result.page.url);

        // Print a preview of the content
        let preview = result
            .document
            .content
            .chars()
            .take(200)
            .collect::<String>();
        println!("Preview: {}...\n", preview);
    }

    Ok(())
}

/// Run the list command.
async fn run_list(connection: Arc<Connection>) -> docs_mcp_rs::core::Result<()> {
    let library_store = LibraryStore::new(&connection);

    let libraries = library_store.list()?;

    if libraries.is_empty() {
        println!("No libraries indexed.");
        return Ok(());
    }

    println!("Indexed libraries:\n");

    for lib in libraries {
        let version_store = VersionStore::new(&connection);
        let versions = version_store.find_by_library(lib.id)?;

        println!("📚 {} ({} version(s))", lib.name, versions.len());

        for v in versions {
            let status = match v.status {
                VersionStatus::Completed => "✅",
                VersionStatus::Running => "🔄",
                VersionStatus::Queued => "⏳",
                VersionStatus::Failed => "❌",
                VersionStatus::Cancelled => "🚫",
                VersionStatus::NotIndexed => "❓",
                VersionStatus::Updating => "🔄",
            };
            println!(
                "   {} {} {}",
                status,
                v.name,
                v.source_url.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    Ok(())
}

/// Run the remove command.
async fn run_remove(
    connection: Arc<Connection>,
    library: String,
    version: Option<String>,
) -> docs_mcp_rs::core::Result<()> {
    let library_store = LibraryStore::new(&connection);

    // Find library
    let lib = library_store.find_by_name(&library)?;

    if let Some(lib) = lib {
        let version_store = VersionStore::new(&connection);

        if let Some(ver_name) = version {
            // Remove specific version
            let ver = version_store.find_by_library_and_name(lib.id, &ver_name)?;
            if let Some(ver) = ver {
                // Delete pages and documents
                let page_store = PageStore::new(&connection);
                let pages = page_store.find_by_version(ver.id)?;
                for page in pages {
                    let doc_store = DocumentStore::new(&connection);
                    doc_store.delete_by_page(page.id)?;
                    page_store.delete(page.id)?;
                }
                version_store.delete(ver.id)?;
                println!("✅ Removed version {} of {}", ver_name, library);
            } else {
                println!("Version {} not found in {}", ver_name, library);
            }
        } else {
            // Remove entire library
            let versions = version_store.find_by_library(lib.id)?;
            for v in versions {
                let page_store = PageStore::new(&connection);
                let pages = page_store.find_by_version(v.id)?;
                for page in pages {
                    let doc_store = DocumentStore::new(&connection);
                    doc_store.delete_by_page(page.id)?;
                    page_store.delete(page.id)?;
                }
                version_store.delete(v.id)?;
            }
            library_store.delete(lib.id)?;
            println!("✅ Removed library {}", library);
        }
    } else {
        println!("Library {} not found", library);
    }

    Ok(())
}
