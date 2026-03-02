//! Database connection management with connection pooling.

use crate::core::{Config, Error, Result};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection as RusqliteConnection;
use rusqlite::ffi::sqlite3_auto_extension;
use std::path::Path;
use std::sync::Arc;
use std::sync::Once;

/// Static initializer for sqlite-vec extension.
static VEC_INIT: Once = Once::new();

/// Register sqlite-vec extension to be auto-loaded for all new connections.
fn ensure_vec_extension() {
    VEC_INIT.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

/// Connection pool size - SQLite benefits from multiple readers in WAL mode.
const DEFAULT_POOL_SIZE: u32 = 5;

/// A thread-safe database connection pool wrapper.
#[derive(Clone)]
pub struct Connection {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl Connection {
    /// Open a new database connection pool at the specified path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Ensure sqlite-vec extension is registered
        ensure_vec_extension();

        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        // Create connection manager with initialization
        let manager = SqliteConnectionManager::file(path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                     PRAGMA journal_mode = WAL;
                     PRAGMA synchronous = NORMAL;
                     PRAGMA cache_size = -64000;
                     PRAGMA temp_store = MEMORY;
                     PRAGMA mmap_size = 268435456;",
            )?;
            Ok(())
        });

        // Build pool with configuration
        let pool = Pool::builder()
            .max_size(DEFAULT_POOL_SIZE)
            .build(manager)
            .map_err(|e| Error::Database(format!("Failed to create connection pool: {}", e)))?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Open an in-memory database pool (for testing).
    pub fn in_memory() -> Result<Self> {
        // Ensure sqlite-vec extension is registered
        ensure_vec_extension();

        let manager = SqliteConnectionManager::memory().with_init(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            Ok(())
        });

        let pool = Pool::builder()
            .max_size(2) // Smaller pool for in-memory
            .build(manager)
            .map_err(|e| Error::Database(format!("Failed to create in-memory pool: {}", e)))?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Open a connection pool from config.
    pub fn from_config(config: &Config) -> Result<Self> {
        Self::open(&config.store_path)
    }

    /// Get a connection from the pool.
    fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| Error::Database(format!("Failed to get connection from pool: {}", e)))
    }

    /// Execute a closure with the connection (sync version).
    pub fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&RusqliteConnection) -> rusqlite::Result<T>,
    {
        let conn = self.get_conn()?;
        f(&conn).map_err(|e| Error::Database(e.to_string()))
    }

    /// Execute a closure with the connection (async version, non-blocking).
    /// Uses spawn_blocking to avoid blocking the async thread pool.
    pub async fn with_connection_async<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&RusqliteConnection) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                Error::Database(format!("Failed to get connection from pool: {}", e))
            })?;
            f(&conn).map_err(|e| Error::Database(e.to_string()))
        })
        .await
        .map_err(|e| Error::Database(format!("Database task panicked: {}", e)))?
    }

    /// Execute a closure with a mutable transaction (sync version).
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T>,
    {
        let mut conn = self.get_conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| Error::Database(format!("Failed to begin transaction: {}", e)))?;
        let result = f(&tx).map_err(|e| Error::Database(e.to_string()))?;
        tx.commit()
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;
        Ok(result)
    }

    /// Execute a closure with a mutable transaction (async version, non-blocking).
    /// Uses spawn_blocking to avoid blocking the async thread pool.
    pub async fn with_transaction_async<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| {
                Error::Database(format!("Failed to get connection from pool: {}", e))
            })?;
            let tx = conn
                .transaction()
                .map_err(|e| Error::Database(format!("Failed to begin transaction: {}", e)))?;
            let result = f(&tx).map_err(|e| Error::Database(e.to_string()))?;
            tx.commit()
                .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;
            Ok(result)
        })
        .await
        .map_err(|e| Error::Database(format!("Database task panicked: {}", e)))?
    }

    /// Execute a SQL statement.
    pub fn execute(&self, sql: &str) -> Result<usize> {
        self.with_connection(|conn| conn.execute(sql, []))
    }

    /// Execute a SQL batch.
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        self.with_connection(|conn| conn.execute_batch(sql))
    }

    /// Get pool status for monitoring.
    pub fn pool_status(&self) -> r2d2::State {
        self.pool.state()
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

    #[test]
    fn test_pool_multiple_connections() {
        let conn = Connection::in_memory().expect("Failed to create connection pool");

        // Test multiple operations in parallel
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)")
            .expect("Failed to create table");

        conn.execute("INSERT INTO test (value) VALUES ('test1')")
            .expect("Failed to insert");

        let count: i64 = conn
            .with_connection(|c| c.query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0)))
            .expect("Failed to count");

        assert_eq!(count, 1);
    }
}
