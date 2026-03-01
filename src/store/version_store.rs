//! Version storage operations.

use crate::core::{NewVersion, Result, Version, VersionStatus};
use crate::store::Connection;
use chrono::{DateTime, Utc};

/// Store for version operations.
pub struct VersionStore<'a> {
    conn: &'a Connection,
}

impl<'a> VersionStore<'a> {
    /// Create a new version store.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new version.
    pub fn create(&self, version: &NewVersion) -> Result<Version> {
        let name = version.name.as_str();
        let scraper_options = version
            .scraper_options
            .as_ref()
            .map(|o| serde_json::to_string(o).unwrap_or_default());

        self.conn.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO versions (library_id, name, source_url, scraper_options, status)
                 VALUES (?1, ?2, ?3, ?4, 'not_indexed')",
                rusqlite::params![
                    version.library_id,
                    name,
                    version.source_url,
                    scraper_options,
                ],
            )?;

            let id = tx.last_insert_rowid();

            Ok(Version {
                id,
                library_id: version.library_id,
                name: version.name.clone(),
                status: VersionStatus::NotIndexed,
                progress_pages: 0,
                progress_max_pages: 0,
                error_message: None,
                source_url: version.source_url.clone(),
                scraper_options: version
                    .scraper_options
                    .as_ref()
                    .and_then(|o| serde_json::to_value(o).ok()),
                started_at: None,
                created_at: Utc::now(),
                updated_at: None,
            })
        })
    }

    /// Find a version by ID.
    pub fn find_by_id(&self, id: i64) -> Result<Option<Version>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, library_id, name, status, progress_pages, progress_max_pages,
                        error_message, source_url, scraper_options, started_at, created_at, updated_at
                 FROM versions WHERE id = ?1"
            )?;

            let result = stmt.query_row(rusqlite::params![id], |row| {
                Ok(Version {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    name: row.get(2)?,
                    status: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                    progress_pages: row.get(4)?,
                    progress_max_pages: row.get(5)?,
                    error_message: row.get(6)?,
                    source_url: row.get(7)?,
                    scraper_options: row.get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    started_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                    created_at: row.get::<_, String>(10)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(11)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            });

            match result {
                Ok(version) => Ok(Some(version)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Find versions by library ID.
    pub fn find_by_library(&self, library_id: i64) -> Result<Vec<Version>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, library_id, name, status, progress_pages, progress_max_pages,
                        error_message, source_url, scraper_options, started_at, created_at, updated_at
                 FROM versions WHERE library_id = ?1 ORDER BY created_at DESC"
            )?;

            let versions = stmt.query_map(rusqlite::params![library_id], |row| {
                Ok(Version {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    name: row.get(2)?,
                    status: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                    progress_pages: row.get(4)?,
                    progress_max_pages: row.get(5)?,
                    error_message: row.get(6)?,
                    source_url: row.get(7)?,
                    scraper_options: row.get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    started_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                    created_at: row.get::<_, String>(10)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(11)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            })?.collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(versions)
        })
    }

    /// Find a specific version of a library by name (case-insensitive).
    pub fn find_by_library_and_name(&self, library_id: i64, name: &str) -> Result<Option<Version>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, library_id, name, status, progress_pages, progress_max_pages,
                        error_message, source_url, scraper_options, started_at, created_at, updated_at
                 FROM versions WHERE library_id = ?1 AND LOWER(name) = LOWER(?2)"
            )?;

            let result = stmt.query_row(rusqlite::params![library_id, name], |row| {
                Ok(Version {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    name: row.get(2)?,
                    status: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                    progress_pages: row.get(4)?,
                    progress_max_pages: row.get(5)?,
                    error_message: row.get(6)?,
                    source_url: row.get(7)?,
                    scraper_options: row.get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    started_at: row.get::<_, Option<String>>(9)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                    created_at: row.get::<_, String>(10)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row.get::<_, Option<String>>(11)?
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                })
            });

            match result {
                Ok(version) => Ok(Some(version)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Update version status.
    pub fn update_status(&self, id: i64, status: VersionStatus) -> Result<()> {
        let status_str = status.to_string();
        let now = Utc::now().to_rfc3339();

        self.conn.with_connection(|conn| {
            conn.execute(
                "UPDATE versions SET status = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![status_str, now, id],
            )
        })?;

        Ok(())
    }

    /// Update version progress.
    pub fn update_progress(&self, id: i64, pages: i64, max_pages: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        self.conn.with_connection(|conn| {
            conn.execute(
                "UPDATE versions SET progress_pages = ?1, progress_max_pages = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![pages, max_pages, now, id],
            )
        })?;

        Ok(())
    }

    /// Set version error.
    pub fn set_error(&self, id: i64, error: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        self.conn.with_connection(|conn| {
            conn.execute(
                "UPDATE versions SET status = 'failed', error_message = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![error, now, id],
            )
        })?;

        Ok(())
    }

    /// Delete a version by ID.
    pub fn delete(&self, id: i64) -> Result<bool> {
        let rows_affected = self.conn.with_connection(|conn| {
            conn.execute("DELETE FROM versions WHERE id = ?1", rusqlite::params![id])
        })?;

        Ok(rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NewLibrary;
    use crate::store::{LibraryStore, run_migrations};

    #[test]
    fn test_version_crud() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");

        // Create a library first
        let lib_store = LibraryStore::new(&conn);
        let library = lib_store
            .create(&NewLibrary {
                name: "TestLib".to_string(),
            })
            .expect("Failed to create library");

        let store = VersionStore::new(&conn);

        // Create
        let version = store
            .create(&NewVersion {
                library_id: library.id,
                name: "1.0.0".to_string(),
                source_url: Some("https://example.com/docs".to_string()),
                scraper_options: None,
            })
            .expect("Failed to create version");

        assert_eq!(version.name, "1.0.0");
        assert_eq!(version.status, VersionStatus::NotIndexed);

        // Find by ID
        let found = store
            .find_by_id(version.id)
            .expect("Failed to find version");
        assert!(found.is_some());

        // Find by library
        let versions = store
            .find_by_library(library.id)
            .expect("Failed to find versions");
        assert_eq!(versions.len(), 1);

        // Update status
        store
            .update_status(version.id, VersionStatus::Running)
            .expect("Failed to update status");
        let found = store
            .find_by_id(version.id)
            .expect("Failed to find version")
            .unwrap();
        assert_eq!(found.status, VersionStatus::Running);

        // Delete
        let deleted = store.delete(version.id).expect("Failed to delete version");
        assert!(deleted);
    }
}
