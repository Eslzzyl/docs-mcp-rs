//! Vector search operations.

use crate::core::{Error, Result, SearchResult};
use crate::store::Connection;
use std::collections::HashMap;
use std::str::FromStr;

/// Vector search options.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Number of results to return.
    pub limit: usize,
    /// Weight for vector search results (0.0 - 1.0).
    pub vector_weight: f32,
    /// Weight for full-text search results (0.0 - 1.0).
    pub fts_weight: f32,
    /// RRF constant K for fusion ranking.
    pub rrf_k: u32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            vector_weight: 0.5,
            fts_weight: 0.5,
            rrf_k: 60,
        }
    }
}

/// Hybrid search combining vector and full-text search.
pub struct VectorSearch<'a> {
    conn: &'a Connection,
    options: SearchOptions,
}

impl<'a> VectorSearch<'a> {
    /// Create a new vector search instance.
    pub fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            options: SearchOptions::default(),
        }
    }

    /// Create with custom options.
    pub fn with_options(conn: &'a Connection, options: SearchOptions) -> Self {
        Self { conn, options }
    }

    /// Perform hybrid search (vector + FTS) using RRF fusion.
    pub async fn search(
        &self,
        library_name: &str,
        version_name: Option<&str>,
        query_vector: &[f32],
        query_text: &str,
    ) -> Result<Vec<SearchResult>> {
        // Get vector search results
        let vector_results = self
            .vector_search(library_name, version_name, query_vector)
            .await?;

        // Get FTS results
        let fts_results = self
            .fts_search(library_name, version_name, query_text)
            .await?;

        // Combine using RRF
        let combined = self.reciprocal_rank_fusion(vector_results, fts_results);

        // Load full entities and build search results
        self.build_search_results(combined).await
    }

    /// Perform FTS-only search (when embedding is not available).
    pub async fn search_fts_only(
        &self,
        library_name: &str,
        version_name: Option<&str>,
        query_text: &str,
    ) -> Result<Vec<SearchResult>> {
        // Get FTS results only
        let fts_results = self
            .fts_search(library_name, version_name, query_text)
            .await?;

        // Use FTS results directly (no fusion needed)
        self.build_search_results(fts_results).await
    }

    /// Perform vector similarity search.
    async fn vector_search(
        &self,
        library_name: &str,
        version_name: Option<&str>,
        query_vector: &[f32],
    ) -> Result<Vec<(i64, f32)>> {
        // Convert query vector to JSON for sqlite-vec
        let vector_json =
            serde_json::to_string(query_vector).map_err(|e| Error::Serialization(e))?;

        let version_filter = version_name
            .map(|_| "AND LOWER(v.name) = LOWER(?2)".to_string())
            .unwrap_or_default();

        let sql = format!(
            r#"
            SELECT d.id, dv.distance
            FROM documents d
            JOIN pages p ON d.page_id = p.id
            JOIN versions v ON p.version_id = v.id
            JOIN libraries l ON v.library_id = l.id,
                 documents_vec dv
            WHERE dv.rowid = d.id
              AND LOWER(l.name) = LOWER(?1)
              {}
              AND dv.embedding MATCH ?
              AND dv.k = ?
            ORDER BY dv.distance
            LIMIT ?
            "#,
            version_filter
        );

        let limit = self.options.limit * 3; // Get more for fusion

        // Clone values to satisfy 'static lifetime requirement for spawn_blocking
        let library_name = library_name.to_string();
        let version_name = version_name.map(|s| s.to_string());

        self.conn.with_connection_async(move |conn| {
            let mut stmt = conn.prepare(&sql)?;

            let results = if version_name.is_some() {
                stmt.query_map(
                    rusqlite::params![
                        library_name,
                        version_name,
                        vector_json,
                        limit as i64,
                        limit as i64,
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(
                    rusqlite::params![library_name, vector_json, limit as i64, limit as i64,],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?
            };

            Ok(results)
        }).await
    }

    /// Perform full-text search.
    async fn fts_search(
        &self,
        library_name: &str,
        version_name: Option<&str>,
        query: &str,
    ) -> Result<Vec<(i64, f32)>> {
        let version_filter = version_name
            .map(|_| "AND LOWER(v.name) = LOWER(?2)".to_string())
            .unwrap_or_default();

        // Escape FTS special characters
        let escaped_query = self.escape_fts_query(query);

        let sql = format!(
            r#"
            SELECT d.id, bm25(documents_fts) as score
            FROM documents d
            JOIN pages p ON d.page_id = p.id
            JOIN versions v ON p.version_id = v.id
            JOIN libraries l ON v.library_id = l.id
            JOIN documents_fts fts ON fts.rowid = d.id
            WHERE LOWER(l.name) = LOWER(?1)
              {}
              AND documents_fts MATCH ?
            ORDER BY score
            LIMIT ?
            "#,
            version_filter
        );

        let limit = self.options.limit * 3;

        // Clone values to satisfy 'static lifetime requirement for spawn_blocking
        let library_name = library_name.to_string();
        let version_name = version_name.map(|s| s.to_string());

        self.conn.with_connection_async(move |conn| {
            let mut stmt = conn.prepare(&sql)?;

            let results = if version_name.is_some() {
                stmt.query_map(
                    rusqlite::params![library_name, version_name, escaped_query, limit as i64],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let score: f32 = row.get(1)?;
                        Ok((id, -score)) // Negate because bm25 returns negative for better matches
                    },
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(
                    rusqlite::params![library_name, escaped_query, limit as i64],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let score: f32 = row.get(1)?;
                        Ok((id, -score))
                    },
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?
            };

            Ok(results)
        }).await
    }

    /// Combine results using Reciprocal Rank Fusion.
    fn reciprocal_rank_fusion(
        &self,
        vector_results: Vec<(i64, f32)>,
        fts_results: Vec<(i64, f32)>,
    ) -> Vec<(i64, f32)> {
        let mut scores: HashMap<i64, f32> = HashMap::new();
        let k = self.options.rrf_k as f32;

        // Add vector search scores
        for (rank, (id, _)) in vector_results.iter().enumerate() {
            let rrf_score = self.options.vector_weight / (k + (rank + 1) as f32);
            *scores.entry(*id).or_insert(0.0) += rrf_score;
        }

        // Add FTS scores
        for (rank, (id, _)) in fts_results.iter().enumerate() {
            let rrf_score = self.options.fts_weight / (k + (rank + 1) as f32);
            *scores.entry(*id).or_insert(0.0) += rrf_score;
        }

        // Sort by combined score
        let mut combined: Vec<_> = scores.into_iter().collect();
        combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        combined.truncate(self.options.limit);
        combined
    }

    /// Build search results from document IDs.
    /// Optimized: Uses single JOIN query instead of N+1 queries.
    async fn build_search_results(&self, doc_scores: Vec<(i64, f32)>) -> Result<Vec<SearchResult>> {
        if doc_scores.is_empty() {
            return Ok(Vec::new());
        }

        // Build score map for ordering
        let _score_map: HashMap<i64, f32> = doc_scores.iter().cloned().collect();

        // Build IN clause with parameter placeholders
        let ids: Vec<i64> = doc_scores.iter().map(|(id, _)| *id).collect();
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let in_clause = placeholders.join(", ");

        // Single query to fetch all documents with their relations
        let sql = format!(
            r#"
            SELECT
                d.id, d.page_id, d.content, d.metadata, d.sort_order, d.embedding, d.created_at,
                p.id as page_id, p.version_id, p.url, p.title, p.etag, p.last_modified, p.content_type, p.depth, p.created_at as page_created_at, p.updated_at as page_updated_at,
                v.id as version_id, v.library_id, v.name as version_name, v.status, v.progress_pages, v.progress_max_pages, v.error_message, v.scraper_options, v.source_url, v.started_at, v.created_at as version_created_at, v.updated_at as version_updated_at,
                l.id as library_id, l.name as library_name, l.created_at as library_created_at
            FROM documents d
            JOIN pages p ON d.page_id = p.id
            JOIN versions v ON p.version_id = v.id
            JOIN libraries l ON v.library_id = l.id
            WHERE d.id IN ({})
            "#,
            in_clause
        );

        // Clone values to satisfy 'static lifetime requirement for spawn_blocking
        let sql = sql.clone();
        let ids = ids.clone();

        let results = self.conn.with_connection_async(move |conn| {
            let mut stmt = conn.prepare(&sql)?;

            let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                // Parse document
                let metadata_json: String = row.get(3)?;
                let metadata: crate::core::ChunkMetadata =
                    serde_json::from_str(&metadata_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| {
                            let mut bytes = [0u8; 4];
                            bytes.copy_from_slice(chunk);
                            f32::from_le_bytes(bytes)
                        })
                        .collect()
                });

                let doc = crate::core::Document {
                    id: row.get(0)?,
                    page_id: row.get(1)?,
                    content: row.get(2)?,
                    metadata,
                    sort_order: row.get(4)?,
                    embedding,
                    created_at: row.get(6)?,
                };

                // Parse page
                let page = crate::core::Page {
                    id: row.get(7)?,
                    version_id: row.get(8)?,
                    url: row.get(9)?,
                    title: row.get(10)?,
                    etag: row.get(11)?,
                    last_modified: row.get(12)?,
                    content_type: row.get(13)?,
                    depth: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                };

                // Parse version
                let version = crate::core::Version {
                    id: row.get(17)?,
                    library_id: row.get(18)?,
                    name: row.get(19)?,
                    status: crate::core::VersionStatus::from_str(&row.get::<_, String>(20)?)
                        .unwrap_or_default(),
                    progress_pages: row.get(21)?,
                    progress_max_pages: row.get(22)?,
                    error_message: row.get(23)?,
                    scraper_options: row
                        .get::<_, Option<String>>(24)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    source_url: row.get(25)?,
                    started_at: row.get(26)?,
                    created_at: row.get(27)?,
                    updated_at: row.get(28)?,
                };

                // Parse library
                let library = crate::core::Library {
                    id: row.get(29)?,
                    name: row.get(30)?,
                    created_at: row.get(31)?,
                };

                Ok((doc.id, (doc, page, version, library)))
            })?;

            let mut map: HashMap<i64, (crate::core::Document, crate::core::Page, crate::core::Version, crate::core::Library)> =
                HashMap::new();
            for row in rows {
                let (id, data) = row.map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                map.insert(id, data);
            }

            Ok(map)
        }).await?;

        // Build results in the original order
        let mut search_results = Vec::with_capacity(doc_scores.len());
        for (doc_id, score) in doc_scores {
            if let Some((doc, page, version, library)) = results.get(&doc_id) {
                search_results.push(SearchResult {
                    document: doc.clone(),
                    page: page.clone(),
                    version: version.clone(),
                    library: library.clone(),
                    score,
                });
            }
        }

        Ok(search_results)
    }

    /// Escape special characters for FTS query.
    fn escape_fts_query(&self, query: &str) -> String {
        // Remove or escape FTS special characters
        let escaped: String = query
            .chars()
            .map(|c| match c {
                '"' | '\'' | '(' | ')' | '*' | '^' => ' ',
                _ => c,
            })
            .collect();

        // Add wildcard for prefix matching
        let words: Vec<&str> = escaped.split_whitespace().collect();
        if words.is_empty() {
            return "*".to_string();
        }

        // Join with OR for broader matching
        words.join(" OR ")
    }

    /// Get the search options.
    pub fn options(&self) -> &SearchOptions {
        &self.options
    }
}
