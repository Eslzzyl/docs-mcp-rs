//! Vector search operations.

use crate::core::{Error, Result, SearchResult};
use crate::store::{Connection, DocumentStore, LibraryStore, PageStore, VersionStore};
use std::collections::HashMap;

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
        let vector_results = self.vector_search(library_name, version_name, query_vector).await?;
        
        // Get FTS results
        let fts_results = self.fts_search(library_name, version_name, query_text).await?;
        
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
        let fts_results = self.fts_search(library_name, version_name, query_text).await?;
        
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
        let vector_json = serde_json::to_string(query_vector)
            .map_err(|e| Error::Serialization(e))?;
        
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
        
        self.conn.with_connection(|conn| {
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
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(
                    rusqlite::params![
                        library_name,
                        vector_json,
                        limit as i64,
                        limit as i64,
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            };
            
            Ok(results)
        })
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
        
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            
            let results = if version_name.is_some() {
                stmt.query_map(
                    rusqlite::params![library_name, version_name, escaped_query, limit as i64],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let score: f32 = row.get(1)?;
                        Ok((id, -score)) // Negate because bm25 returns negative for better matches
                    },
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(
                    rusqlite::params![library_name, escaped_query, limit as i64],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let score: f32 = row.get(1)?;
                        Ok((id, -score))
                    },
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            };
            
            Ok(results)
        })
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
    async fn build_search_results(&self, doc_scores: Vec<(i64, f32)>) -> Result<Vec<SearchResult>> {
        let doc_store = DocumentStore::new(self.conn);
        let page_store = PageStore::new(self.conn);
        let version_store = VersionStore::new(self.conn);
        let library_store = LibraryStore::new(self.conn);
        
        let mut results = Vec::with_capacity(doc_scores.len());
        
        for (doc_id, score) in doc_scores {
            // Load document
            let doc = match doc_store.find_by_id(doc_id)? {
                Some(d) => d,
                None => continue,
            };
            
            // Load page
            let page = match page_store.find_by_id(doc.page_id)? {
                Some(p) => p,
                None => continue,
            };
            
            // Load version
            let version = match version_store.find_by_id(page.version_id)? {
                Some(v) => v,
                None => continue,
            };
            
            // Load library
            let library = match library_store.find_by_id(version.library_id)? {
                Some(l) => l,
                None => continue,
            };
            
            results.push(SearchResult {
                document: doc,
                page,
                version,
                library,
                score,
            });
        }
        
        Ok(results)
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
