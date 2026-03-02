//! Page storage operations.

use crate::core::{NewPage, Page, Result};
use crate::store::Connection;
use chrono::{DateTime, Utc};

/// Store for page operations.
pub struct PageStore<'a> {
    conn: &'a Connection,
}

impl<'a> PageStore<'a> {
    /// Create a new page store.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new page (or update if exists).
    pub fn upsert(&self, page: &NewPage) -> Result<Page> {
        let now = Utc::now().to_rfc3339();

        self.conn.with_connection(|conn| {
            // Use RETURNING clause to get the correct ID for both INSERT and UPDATE cases
            let id: i64 = conn.query_row(
                "INSERT INTO pages (version_id, url, title, etag, last_modified, content_type, depth, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(version_id, url) DO UPDATE SET
                    title = excluded.title,
                    etag = excluded.etag,
                    last_modified = excluded.last_modified,
                    content_type = excluded.content_type,
                    depth = excluded.depth,
                    updated_at = excluded.updated_at
                 RETURNING id",
                rusqlite::params![
                    page.version_id,
                    page.url,
                    page.title,
                    page.etag,
                    page.last_modified,
                    page.content_type,
                    page.depth,
                    now,
                ],
                |row| row.get(0),
            )?;

            Ok(Page {
                id,
                version_id: page.version_id,
                url: page.url.clone(),
                title: page.title.clone(),
                etag: page.etag.clone(),
                last_modified: page.last_modified.clone(),
                content_type: page.content_type.clone(),
                depth: page.depth,
                created_at: Utc::now(),
                updated_at: Some(Utc::now()),
            })
        })
    }

    /// Find a page by ID.
    pub fn find_by_id(&self, id: i64) -> Result<Option<Page>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, version_id, url, title, etag, last_modified, content_type, depth, created_at, updated_at
                 FROM pages WHERE id = ?1"
            )?;

            let result = stmt.query_row(rusqlite::params![id], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    version_id: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    etag: row.get(4)?,
                    last_modified: row.get(5)?,
                    content_type: row.get(6)?,
                    depth: row.get(7)?,
                    created_at: row.get::<_, String>(8)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            });

            match result {
                Ok(page) => Ok(Some(page)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Find pages by version ID.
    pub fn find_by_version(&self, version_id: i64) -> Result<Vec<Page>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, version_id, url, title, etag, last_modified, content_type, depth, created_at, updated_at
                 FROM pages WHERE version_id = ?1 ORDER BY created_at"
            )?;

            let pages = stmt.query_map(rusqlite::params![version_id], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    version_id: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    etag: row.get(4)?,
                    last_modified: row.get(5)?,
                    content_type: row.get(6)?,
                    depth: row.get(7)?,
                    created_at: row.get::<_, String>(8)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            })?.collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(pages)
        })
    }

    /// Find a page by version and URL.
    pub fn find_by_version_and_url(&self, version_id: i64, url: &str) -> Result<Option<Page>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, version_id, url, title, etag, last_modified, content_type, depth, created_at, updated_at
                 FROM pages WHERE version_id = ?1 AND url = ?2"
            )?;

            let result = stmt.query_row(rusqlite::params![version_id, url], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    version_id: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    etag: row.get(4)?,
                    last_modified: row.get(5)?,
                    content_type: row.get(6)?,
                    depth: row.get(7)?,
                    created_at: row.get::<_, String>(8)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            });

            match result {
                Ok(page) => Ok(Some(page)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Count pages for a version.
    pub fn count_by_version(&self, version_id: i64) -> Result<i64> {
        self.conn.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pages WHERE version_id = ?1",
                rusqlite::params![version_id],
                |row| row.get(0),
            )?;
            Ok(count)
        })
    }

    /// Delete all pages for a version.
    pub fn delete_by_version(&self, version_id: i64) -> Result<usize> {
        self.conn.with_connection(|conn| {
            conn.execute(
                "DELETE FROM pages WHERE version_id = ?1",
                rusqlite::params![version_id],
            )
        })
    }

    /// Delete a page by ID.
    pub fn delete(&self, id: i64) -> Result<usize> {
        self.conn.with_connection(|conn| {
            conn.execute("DELETE FROM pages WHERE id = ?1", rusqlite::params![id])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{NewLibrary, NewVersion};
    use crate::store::{LibraryStore, VersionStore, run_migrations};

    #[test]
    fn test_page_crud() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");

        // Create library and version first
        let lib_store = LibraryStore::new(&conn);
        let library = lib_store
            .create(&NewLibrary {
                name: "TestLib".to_string(),
            })
            .expect("Failed to create library");

        let ver_store = VersionStore::new(&conn);
        let version = ver_store
            .create(&NewVersion {
                library_id: library.id,
                name: "1.0.0".to_string(),
                source_url: None,
                scraper_options: None,
            })
            .expect("Failed to create version");

        let store = PageStore::new(&conn);

        // Create
        let page = store
            .upsert(&NewPage {
                version_id: version.id,
                url: "https://example.com/docs/page1".to_string(),
                title: Some("Page 1".to_string()),
                etag: None,
                last_modified: None,
                content_type: Some("text/html".to_string()),
                depth: 0,
            })
            .expect("Failed to create page");

        assert_eq!(page.url, "https://example.com/docs/page1");

        // Find by ID
        let found = store.find_by_id(page.id).expect("Failed to find page");
        assert!(found.is_some());

        // Find by version
        let pages = store
            .find_by_version(version.id)
            .expect("Failed to find pages");
        assert_eq!(pages.len(), 1);

        // Upsert should update
        let updated = store
            .upsert(&NewPage {
                version_id: version.id,
                url: "https://example.com/docs/page1".to_string(),
                title: Some("Updated Title".to_string()),
                etag: Some("abc123".to_string()),
                last_modified: None,
                content_type: Some("text/html".to_string()),
                depth: 0,
            })
            .expect("Failed to update page");

        assert_eq!(updated.title, Some("Updated Title".to_string()));
        let pages = store
            .find_by_version(version.id)
            .expect("Failed to find pages");
        assert_eq!(pages.len(), 1); // Should still be 1
    }
}
