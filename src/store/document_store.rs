//! Document storage operations.

use crate::core::{Document, NewDocument, Result};
use crate::store::Connection;
use chrono::{DateTime, Utc};

/// Store for document operations.
pub struct DocumentStore<'a> {
    conn: &'a Connection,
}

impl<'a> DocumentStore<'a> {
    /// Create a new document store.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new document.
    pub fn create(&self, doc: &NewDocument) -> Result<Document> {
        let metadata_json = serde_json::to_string(&doc.metadata).unwrap_or_else(|_| "{}".to_string());
        let embedding_blob = doc.embedding.as_ref().map(|e| {
            // Convert Vec<f32> to bytes
            let bytes: Vec<u8> = e.iter()
                .flat_map(|f| f.to_le_bytes())
                .collect();
            bytes
        });
        
        self.conn.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO documents (page_id, content, metadata, sort_order, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    doc.page_id,
                    doc.content,
                    metadata_json,
                    doc.sort_order,
                    embedding_blob,
                ],
            )?;
            
            let id = tx.last_insert_rowid();
            
            Ok(Document {
                id,
                page_id: doc.page_id,
                content: doc.content.clone(),
                metadata: doc.metadata.clone(),
                sort_order: doc.sort_order,
                embedding: doc.embedding.clone(),
                created_at: Utc::now(),
            })
        })
    }

    /// Create multiple documents in batch.
    pub fn create_batch(&self, docs: &[NewDocument]) -> Result<Vec<Document>> {
        self.conn.with_transaction(|tx| {
            let mut results = Vec::with_capacity(docs.len());
            
            for doc in docs {
                let metadata_json = serde_json::to_string(&doc.metadata).unwrap_or_else(|_| "{}".to_string());
                let embedding_blob = doc.embedding.as_ref().map(|e| {
                    let bytes: Vec<u8> = e.iter()
                        .flat_map(|f| f.to_le_bytes())
                        .collect();
                    bytes
                });
                
                tx.execute(
                    "INSERT INTO documents (page_id, content, metadata, sort_order, embedding)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        doc.page_id,
                        doc.content,
                        metadata_json,
                        doc.sort_order,
                        embedding_blob,
                    ],
                )?;
                
                let id = tx.last_insert_rowid();
                
                results.push(Document {
                    id,
                    page_id: doc.page_id,
                    content: doc.content.clone(),
                    metadata: doc.metadata.clone(),
                    sort_order: doc.sort_order,
                    embedding: doc.embedding.clone(),
                    created_at: Utc::now(),
                });
            }
            
            Ok(results)
        })
    }

    /// Find a document by ID.
    pub fn find_by_id(&self, id: i64) -> Result<Option<Document>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, page_id, content, metadata, sort_order, embedding, created_at
                 FROM documents WHERE id = ?1"
            )?;
            
            let result = stmt.query_row(rusqlite::params![id], |row| {
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|b| {
                    // Convert bytes back to Vec<f32>
                    b.chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                        .collect()
                });
                
                Ok(Document {
                    id: row.get(0)?,
                    page_id: row.get(1)?,
                    content: row.get(2)?,
                    metadata: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    sort_order: row.get(4)?,
                    embedding,
                    created_at: row.get::<_, String>(6)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            });
            
            match result {
                Ok(doc) => Ok(Some(doc)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Find documents by page ID.
    pub fn find_by_page(&self, page_id: i64) -> Result<Vec<Document>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, page_id, content, metadata, sort_order, embedding, created_at
                 FROM documents WHERE page_id = ?1 ORDER BY sort_order"
            )?;
            
            let docs = stmt.query_map(rusqlite::params![page_id], |row| {
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|b| {
                    b.chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                        .collect()
                });
                
                Ok(Document {
                    id: row.get(0)?,
                    page_id: row.get(1)?,
                    content: row.get(2)?,
                    metadata: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    sort_order: row.get(4)?,
                    embedding,
                    created_at: row.get::<_, String>(6)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?.collect::<std::result::Result<Vec<_>, _>>()?;
            
            Ok(docs)
        })
    }

    /// Delete documents by page ID.
    pub fn delete_by_page(&self, page_id: i64) -> Result<usize> {
        self.conn.with_connection(|conn| {
            conn.execute("DELETE FROM documents WHERE page_id = ?1", rusqlite::params![page_id])
        })
    }

    /// Search documents using full-text search.
    pub fn search_fts(
        &self,
        library_name: &str,
        version_name: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(Document, i64, i64)>> {
        // (Document, page_id for joining, rank)
        let version_filter = version_name
            .map(|_| "AND LOWER(v.name) = LOWER(?)".to_string())
            .unwrap_or_default();
        
        let sql = format!(
            r#"
            SELECT d.id, d.page_id, d.content, d.metadata, d.sort_order, d.created_at,
                   p.id, v.id,
                   bm25(documents_fts) as rank
            FROM documents d
            JOIN pages p ON d.page_id = p.id
            JOIN versions v ON p.version_id = v.id
            JOIN libraries l ON v.library_id = l.id
            JOIN documents_fts fts ON fts.rowid = d.id
            WHERE LOWER(l.name) = LOWER(?)
              {}
              AND documents_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            "#,
            version_filter
        );
        
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            
            let docs = if version_name.is_some() {
                stmt.query_map(
                    rusqlite::params![library_name, version_name, query, limit as i64],
                    |row| {
                        let _embedding_blob: Option<Vec<u8>> = None; // Don't load embedding for search
                        Ok((
                            Document {
                                id: row.get(0)?,
                                page_id: row.get(1)?,
                                content: row.get(2)?,
                                metadata: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                                sort_order: row.get(4)?,
                                embedding: None,
                                created_at: row.get::<_, String>(5)?.parse::<DateTime<Utc>>()
                                    .unwrap_or_else(|_| Utc::now()),
                            },
                            row.get(6)?, // page id for reference
                            row.get(7)?, // version id for reference
                        ))
                    },
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(
                    rusqlite::params![library_name, query, limit as i64],
                    |row| {
                        Ok((
                            Document {
                                id: row.get(0)?,
                                page_id: row.get(1)?,
                                content: row.get(2)?,
                                metadata: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                                sort_order: row.get(4)?,
                                embedding: None,
                                created_at: row.get::<_, String>(5)?.parse::<DateTime<Utc>>()
                                    .unwrap_or_else(|_| Utc::now()),
                            },
                            row.get(6)?,
                            row.get(7)?,
                        ))
                    },
                )?.collect::<std::result::Result<Vec<_>, _>>()?
            };
            
            Ok(docs)
        })
    }

    /// Count documents for a version.
    pub fn count_by_version(&self, version_id: i64) -> Result<i64> {
        self.conn.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM documents d
                 JOIN pages p ON d.page_id = p.id
                 WHERE p.version_id = ?1",
                rusqlite::params![version_id],
                |row| row.get(0),
            )?;
            Ok(count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{run_migrations, LibraryStore, VersionStore, PageStore};
    use crate::core::{NewLibrary, NewVersion, NewPage, ChunkMetadata};

    fn setup_test_data(conn: &Connection) -> (i64, i64, i64) {
        let lib_store = LibraryStore::new(conn);
        let library = lib_store.create(&NewLibrary {
            name: "TestLib".to_string(),
        }).expect("Failed to create library");
        
        let ver_store = VersionStore::new(conn);
        let version = ver_store.create(&NewVersion {
            library_id: library.id,
            name: "1.0.0".to_string(),
            source_url: None,
            scraper_options: None,
        }).expect("Failed to create version");
        
        let page_store = PageStore::new(conn);
        let page = page_store.upsert(&NewPage {
            version_id: version.id,
            url: "https://example.com/docs/page1".to_string(),
            title: Some("Test Page".to_string()),
            etag: None,
            last_modified: None,
            content_type: Some("text/html".to_string()),
            depth: 0,
        }).expect("Failed to create page");
        
        (library.id, version.id, page.id)
    }

    #[test]
    fn test_document_crud() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");
        
        let (_, _, page_id) = setup_test_data(&conn);
        let store = DocumentStore::new(&conn);
        
        // Create
        let doc = store.create(&NewDocument {
            page_id,
            content: "This is test content for the document.".to_string(),
            metadata: ChunkMetadata::default(),
            sort_order: 0,
            embedding: Some(vec![0.1, 0.2, 0.3]),
        }).expect("Failed to create document");
        
        assert_eq!(doc.content, "This is test content for the document.");
        
        // Find by ID
        let found = store.find_by_id(doc.id).expect("Failed to find document");
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.embedding, Some(vec![0.1, 0.2, 0.3]));
        
        // Find by page
        let docs = store.find_by_page(page_id).expect("Failed to find documents");
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn test_document_batch_create() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");
        
        let (_, _, page_id) = setup_test_data(&conn);
        let store = DocumentStore::new(&conn);
        
        // Create batch
        let docs = store.create_batch(&[
            NewDocument {
                page_id,
                content: "Content 1".to_string(),
                metadata: ChunkMetadata::default(),
                sort_order: 0,
                embedding: None,
            },
            NewDocument {
                page_id,
                content: "Content 2".to_string(),
                metadata: ChunkMetadata::default(),
                sort_order: 1,
                embedding: None,
            },
        ]).expect("Failed to create documents");
        
        assert_eq!(docs.len(), 2);
        
        let found = store.find_by_page(page_id).expect("Failed to find documents");
        assert_eq!(found.len(), 2);
    }
}
