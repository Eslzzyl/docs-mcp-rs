//! Library storage operations.

use crate::core::{Library, NewLibrary, Result};
use crate::store::Connection;
use chrono::{DateTime, Utc};

/// Store for library operations.
pub struct LibraryStore<'a> {
    conn: &'a Connection,
}

impl<'a> LibraryStore<'a> {
    /// Create a new library store.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new library.
    pub fn create(&self, library: &NewLibrary) -> Result<Library> {
        let name = library.name.to_lowercase();
        
        self.conn.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO libraries (name) VALUES (?1)",
                rusqlite::params![name],
            )?;
            
            let id = tx.last_insert_rowid();
            
            Ok(Library {
                id,
                name,
                created_at: Utc::now(),
            })
        })
    }

    /// Find a library by ID.
    pub fn find_by_id(&self, id: i64) -> Result<Option<Library>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at FROM libraries WHERE id = ?1"
            )?;
            
            let result = stmt.query_row(rusqlite::params![id], |row| {
                Ok(Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get::<_, String>(2)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            });
            
            match result {
                Ok(library) => Ok(Some(library)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Find a library by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Result<Option<Library>> {
        let name_lower = name.to_lowercase();
        
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at FROM libraries WHERE LOWER(name) = LOWER(?1)"
            )?;
            
            let result = stmt.query_row(rusqlite::params![name_lower], |row| {
                Ok(Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get::<_, String>(2)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            });
            
            match result {
                Ok(library) => Ok(Some(library)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// List all libraries.
    pub fn list(&self) -> Result<Vec<Library>> {
        self.conn.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at FROM libraries ORDER BY name"
            )?;
            
            let libraries = stmt.query_map([], |row| {
                Ok(Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get::<_, String>(2)?.parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?.collect::<std::result::Result<Vec<_>, _>>()?;
            
            Ok(libraries)
        })
    }

    /// Delete a library by ID.
    pub fn delete(&self, id: i64) -> Result<bool> {
        let rows_affected = self.conn.with_connection(|conn| {
            conn.execute("DELETE FROM libraries WHERE id = ?1", rusqlite::params![id])
        })?;
        
        Ok(rows_affected > 0)
    }

    /// Check if a library exists.
    pub fn exists(&self, name: &str) -> Result<bool> {
        let name_lower = name.to_lowercase();
        
        self.conn.with_connection(|conn| {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM libraries WHERE LOWER(name) = LOWER(?1))",
                rusqlite::params![name_lower],
                |row| row.get(0),
            )?;
            Ok(exists)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::run_migrations;

    #[test]
    fn test_library_crud() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");
        
        let store = LibraryStore::new(&conn);
        
        // Create
        let library = store.create(&NewLibrary {
            name: "TestLibrary".to_string(),
        }).expect("Failed to create library");
        
        assert_eq!(library.name, "testlibrary"); // Should be lowercase
        
        // Find by ID
        let found = store.find_by_id(library.id).expect("Failed to find library");
        assert!(found.is_some());
        
        // Find by name
        let found = store.find_by_name("TestLibrary").expect("Failed to find library");
        assert!(found.is_some());
        
        // List
        let libraries = store.list().expect("Failed to list libraries");
        assert_eq!(libraries.len(), 1);
        
        // Delete
        let deleted = store.delete(library.id).expect("Failed to delete library");
        assert!(deleted);
        
        // Verify deleted
        let found = store.find_by_id(library.id).expect("Failed to find library");
        assert!(found.is_none());
    }
}
