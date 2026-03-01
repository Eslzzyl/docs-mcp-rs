//! Database connection management.

use crate::core::{Config, Error, Result};
use rusqlite::Connection as RusqliteConnection;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A thread-safe database connection wrapper.
#[derive(Clone)]
pub struct Connection {
    inner: Arc<Mutex<RusqliteConnection>>,
}

impl Connection {
    /// Open a new database connection at the specified path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        let conn = RusqliteConnection::open(path).map_err(|e| {
            Error::Database(format!("Failed to open database at {:?}: {}", path, e))
        })?;

        // Enable foreign keys
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;",
        )
        .map_err(|e| Error::Database(format!("Failed to set pragmas: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory database (for testing).
    pub fn in_memory() -> Result<Self> {
        let conn = RusqliteConnection::open_in_memory()
            .map_err(|e| Error::Database(format!("Failed to create in-memory database: {}", e)))?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| Error::Database(format!("Failed to set pragmas: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open a connection from config.
    pub fn from_config(config: &Config) -> Result<Self> {
        Self::open(&config.store_path)
    }

    /// Get a reference to the underlying connection.
    pub fn inner(&self) -> &Arc<Mutex<RusqliteConnection>> {
        &self.inner
    }

    /// Execute a closure with the connection.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&RusqliteConnection) -> rusqlite::Result<T>,
    {
        let conn = self
            .inner
            .lock()
            .map_err(|e| Error::Database(format!("Failed to acquire connection lock: {}", e)))?;
        f(&conn).map_err(|e| Error::Database(e.to_string()))
    }

    /// Execute a closure with a mutable transaction.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T>,
    {
        let mut conn = self
            .inner
            .lock()
            .map_err(|e| Error::Database(format!("Failed to acquire connection lock: {}", e)))?;
        let tx = conn
            .transaction()
            .map_err(|e| Error::Database(format!("Failed to begin transaction: {}", e)))?;
        let result = f(&tx).map_err(|e| Error::Database(e.to_string()))?;
        tx.commit()
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;
        Ok(result)
    }

    /// Execute a SQL statement.
    pub fn execute(&self, sql: &str) -> Result<usize> {
        self.with_connection(|conn| conn.execute(sql, []))
    }

    /// Execute a SQL batch.
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        self.with_connection(|conn| conn.execute_batch(sql))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_connection() {
        let conn = Connection::in_memory().expect("Failed to create in-memory connection");
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)")
            .expect("Failed to create table");
    }
}
