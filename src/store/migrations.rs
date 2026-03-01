//! Database migrations.

use crate::core::Result;
use crate::store::Connection;

/// Migration definition.
struct Migration {
    version: i32,
    name: &'static str,
    sql: &'static str,
}

/// All migrations in order.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../../migrations/001_initial_schema.sql"),
}];

/// Run all pending migrations.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create migrations table if it doesn't exist
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Get current version
    let current_version: i32 = conn
        .with_connection(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
        })
        .unwrap_or(0);

    // Run pending migrations
    for migration in MIGRATIONS {
        if migration.version > current_version {
            tracing::info!(
                "Running migration {} ({})",
                migration.version,
                migration.name
            );

            conn.execute_batch(migration.sql)?;

            conn.with_connection(|c| {
                c.execute(
                    "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                    rusqlite::params![migration.version, migration.name],
                )
            })?;

            tracing::info!(
                "Migration {} ({}) completed",
                migration.version,
                migration.name
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations() {
        let conn = Connection::in_memory().expect("Failed to create connection");
        run_migrations(&conn).expect("Migrations should succeed");
    }
}
