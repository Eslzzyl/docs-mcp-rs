//! HTTP handlers for the web API.

use crate::embed::Embedder;
use crate::events::{EventBus, Job};
use crate::pipeline::PipelineManager;
use crate::store::{Connection, LibraryStore, SearchOptions, VersionStore};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Static files embedded at compile time.
#[derive(RustEmbed)]
#[folder = "public/"]
struct StaticAssets;

/// Serve embedded static files.
async fn serve_static_file(path: Path<String>) -> impl IntoResponse {
    let path = path.0;
    let path = if path.is_empty() || path == "/" {
        "index.html"
    } else {
        &path
    };

    match StaticAssets::get(path) {
        Some(content) => {
            let mime_type = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime_type.as_ref())],
                content.data,
            )
                .into_response()
        }
        None => (
            axum::http::StatusCode::NOT_FOUND,
            "Not Found",
        )
            .into_response(),
    }
}

/// Application state shared across handlers.
#[derive(Clone)]
pub struct AppState {
    pub connection: Arc<Connection>,
    pub embedder: Arc<RwLock<Box<dyn Embedder>>>,
    pub pipeline: Arc<PipelineManager>,
    pub event_bus: EventBus,
}

/// Query parameters for search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Scrape request body.
#[derive(Debug, Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
    pub library: String,
    #[serde(default)]
    pub version: String,
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_max_pages() -> usize {
    1000
}

fn default_max_depth() -> usize {
    3
}

/// Search result item.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub library: String,
    pub version: String,
    pub url: String,
    pub title: String,
    pub content: String,
    pub score: f64,
}

/// API response for operations.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// Library info for API.
#[derive(Debug, Serialize)]
pub struct LibraryInfo {
    pub name: String,
    pub versions: Vec<VersionInfo>,
}

/// Version info for API.
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub name: String,
    pub status: String,
    pub page_count: usize,
}

/// Create the web router.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // API routes
        .route("/api/libraries", get(list_libraries))
        .route("/api/libraries/{name}", get(get_library))
        .route(
            "/api/libraries/{name}/versions/{version}",
            delete(delete_version),
        )
        .route("/api/libraries/{name}/search", get(search_library))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs", post(create_job))
        .route("/api/jobs/{id}/cancel", post(cancel_job))
        .route("/api/jobs/clear", post(clear_jobs))
        .route("/api/events", get(crate::web::sse::sse_handler))
        // Web UI - serve embedded static files
        .route("/", get(|| serve_static_file(Path(String::new()))))
        .route("/{*path}", get(serve_static_file))
        .with_state(state)
}

/// Create the web router with MCP HTTP transport endpoint.
pub fn create_router_with_mcp(state: AppState, mcp_service: crate::mcp::McpHttpService) -> Router {
    Router::new()
        // MCP Streamable HTTP endpoint
        .nest_service("/mcp", mcp_service)
        // API routes
        .route("/api/libraries", get(list_libraries))
        .route("/api/libraries/{name}", get(get_library))
        .route(
            "/api/libraries/{name}/versions/{version}",
            delete(delete_version),
        )
        .route("/api/libraries/{name}/search", get(search_library))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs", post(create_job))
        .route("/api/jobs/{id}/cancel", post(cancel_job))
        .route("/api/jobs/clear", post(clear_jobs))
        .route("/api/events", get(crate::web::sse::sse_handler))
        // Web UI - serve embedded static files
        .route("/", get(|| serve_static_file(Path(String::new()))))
        .route("/{*path}", get(serve_static_file))
        .with_state(state)
}

/// GET /api/libraries - List all libraries.
async fn list_libraries(State(state): State<AppState>) -> Json<ApiResponse<Vec<LibraryInfo>>> {
    let lib_store = LibraryStore::new(&state.connection);
    let ver_store = VersionStore::new(&state.connection);

    match lib_store.list() {
        Ok(libraries) => {
            let mut result = Vec::new();
            for lib in libraries {
                let versions = match ver_store.find_by_library(lib.id) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let version_infos: Vec<VersionInfo> = versions
                    .into_iter()
                    .map(|v| {
                        let page_count = state
                            .connection
                            .with_connection(|conn| {
                                conn.query_row(
                                    "SELECT COUNT(*) FROM pages p 
                                 JOIN versions v ON p.version_id = v.id 
                                 WHERE v.library_id = ?1 AND v.name = ?2",
                                    rusqlite::params![lib.id, v.name],
                                    |row| row.get::<_, i64>(0),
                                )
                            })
                            .unwrap_or(0) as usize;

                        VersionInfo {
                            name: v.name,
                            status: format!("{:?}", v.status).to_lowercase(),
                            page_count,
                        }
                    })
                    .collect();

                result.push(LibraryInfo {
                    name: lib.name,
                    versions: version_infos,
                });
            }
            Json(ApiResponse::success(result))
        }
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// GET /api/libraries/:name - Get library details.
async fn get_library(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<ApiResponse<LibraryInfo>> {
    let lib_store = LibraryStore::new(&state.connection);
    let ver_store = VersionStore::new(&state.connection);

    match lib_store.find_by_name(&name) {
        Ok(Some(lib)) => {
            let versions = match ver_store.find_by_library(lib.id) {
                Ok(v) => v,
                Err(e) => return Json(ApiResponse::error(e.to_string())),
            };

            let version_infos: Vec<VersionInfo> = versions
                .into_iter()
                .map(|v| {
                    let page_count = state
                        .connection
                        .with_connection(|conn| {
                            conn.query_row(
                                "SELECT COUNT(*) FROM pages p 
                             JOIN versions v ON p.version_id = v.id 
                             WHERE v.library_id = ?1 AND v.name = ?2",
                                rusqlite::params![lib.id, v.name],
                                |row| row.get::<_, i64>(0),
                            )
                        })
                        .unwrap_or(0) as usize;

                    VersionInfo {
                        name: v.name,
                        status: format!("{:?}", v.status).to_lowercase(),
                        page_count,
                    }
                })
                .collect();

            Json(ApiResponse::success(LibraryInfo {
                name: lib.name,
                versions: version_infos,
            }))
        }
        Ok(None) => Json(ApiResponse::error(format!("Library '{}' not found", name))),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// DELETE /api/libraries/:name/versions/:version - Delete a version.
async fn delete_version(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    let lib_store = LibraryStore::new(&state.connection);
    let ver_store = VersionStore::new(&state.connection);

    match lib_store.find_by_name(&name) {
        Ok(Some(lib)) => {
            match ver_store.find_by_library_and_name(lib.id, &version) {
                Ok(Some(ver)) => {
                    // Delete all pages and documents for this version
                    if let Err(e) = state.connection.with_transaction(|tx| {
                        tx.execute(
                            "DELETE FROM documents WHERE page_id IN (SELECT id FROM pages WHERE version_id = ?1)",
                            rusqlite::params![ver.id],
                        )?;
                        tx.execute(
                            "DELETE FROM pages WHERE version_id = ?1",
                            rusqlite::params![ver.id],
                        )?;
                        tx.execute(
                            "DELETE FROM versions WHERE id = ?1",
                            rusqlite::params![ver.id],
                        )?;
                        Ok(())
                    }) {
                        return Json(ApiResponse::error(e.to_string()));
                    }

                    // Emit library change event
                    state.event_bus.emit(crate::events::Event::library_change());

                    Json(ApiResponse::success(()))
                }
                Ok(None) => Json(ApiResponse::error(format!(
                    "Version '{}' not found",
                    version
                ))),
                Err(e) => Json(ApiResponse::error(e.to_string())),
            }
        }
        Ok(None) => Json(ApiResponse::error(format!("Library '{}' not found", name))),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// GET /api/libraries/:name/search - Search within a library.
async fn search_library(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Json<ApiResponse<Vec<SearchResult>>> {
    use crate::store::VectorSearch;

    let lib_store = LibraryStore::new(&state.connection);
    let ver_store = VersionStore::new(&state.connection);

    // Find library
    let lib = match lib_store.find_by_name(&name) {
        Ok(Some(l)) => l,
        Ok(None) => return Json(ApiResponse::error(format!("Library '{}' not found", name))),
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Get latest version
    let versions = match ver_store.find_by_library(lib.id) {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let latest_version = match versions.into_iter().next() {
        Some(v) => v,
        None => return Json(ApiResponse::error("No versions found")),
    };

    // Search
    let vector_search = VectorSearch::with_options(
        &state.connection,
        SearchOptions {
            limit: query.limit,
            ..Default::default()
        },
    );

    // Check if embedder is available
    let embedder = state.embedder.read().await;
    let is_embedding_available = embedder.is_available();

    let results = if is_embedding_available {
        // Try to generate embedding for query
        match embedder.embed(&query.q).await {
            Ok(query_embedding) => {
                // Use hybrid search (vector + FTS)
                drop(embedder);
                vector_search
                    .search(
                        &name,
                        Some(&latest_version.name),
                        &query_embedding,
                        &query.q,
                    )
                    .await
            }
            Err(_) => {
                // Fallback to FTS-only if embedding fails
                drop(embedder);
                vector_search
                    .search_fts_only(&name, Some(&latest_version.name), &query.q)
                    .await
            }
        }
    } else {
        // Use FTS-only search
        drop(embedder);
        vector_search
            .search_fts_only(&name, Some(&latest_version.name), &query.q)
            .await
    };

    match results {
        Ok(results) => {
            let search_results: Vec<SearchResult> = results
                .into_iter()
                .map(|r| SearchResult {
                    library: r.library.name,
                    version: r.version.name,
                    url: r.page.url,
                    title: r.page.title.unwrap_or_default(),
                    content: r.document.content,
                    score: r.score as f64,
                })
                .collect();
            Json(ApiResponse::success(search_results))
        }
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// GET /api/jobs - List all jobs.
async fn list_jobs(State(state): State<AppState>) -> Json<ApiResponse<Vec<Job>>> {
    let jobs = state.pipeline.get_jobs().await;
    Json(ApiResponse::success(jobs))
}

/// POST /api/jobs - Create a new scraping job.
async fn create_job(
    State(state): State<AppState>,
    Json(req): Json<ScrapeRequest>,
) -> Json<ApiResponse<String>> {
    use crate::core::ScraperOptions;

    let options = ScraperOptions {
        max_pages: Some(req.max_pages),
        max_depth: Some(req.max_depth),
        ..Default::default()
    };

    match state
        .pipeline
        .enqueue(req.library, req.version, req.url, options)
        .await
    {
        Ok(job_id) => Json(ApiResponse::success(job_id)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// POST /api/jobs/:id/cancel - Cancel a job.
async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    match state.pipeline.cancel_job(&id).await {
        Ok(()) => Json(ApiResponse::success(())),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// POST /api/jobs/clear - Clear completed jobs.
async fn clear_jobs(State(state): State<AppState>) -> Json<ApiResponse<usize>> {
    let count = state.pipeline.clear_completed().await;
    Json(ApiResponse::success(count))
}
