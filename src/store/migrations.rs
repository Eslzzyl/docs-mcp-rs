//! Database migrations.

use crate::core::Result;
use crate::store::Connection;
use half::f16;

/// Migration definition.
struct Migration {
    version: i32,
    name: &'static str,
    sql: &'static str,
    /// Special handler for migrations that need custom Rust code
    handler: Option<fn(&Connection) -> Result<()>>,
}

/// All migrations in order.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../../migrations/001_initial_schema.sql"),
        handler: None,
    },
    Migration {
        version: 2,
        name: "fix_fts_delete_trigger",
        sql: include_str!("../../migrations/002_fix_fts_delete_trigger.sql"),
        handler: None,
    },
    Migration {
        version: 3,
        name: "create_vector_table",
        sql: include_str!("../../migrations/003_create_vector_table.sql"),
        handler: None,
    },
    Migration {
        version: 4,
        name: "convert_embedding_to_f16",
        sql: include_str!("../../migrations/004_convert_embedding_to_f16.sql"),
        handler: Some(convert_embeddings_to_f16),
    },
];

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

            // Run SQL if present
            if !migration.sql.is_empty() {
                conn.execute_batch(migration.sql)?;
            }

            // Run special handler if present
            if let Some(handler) = migration.handler {
                handler(conn)?;
            }

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

/// Convert existing embeddings from f32 to f16 format.
/// This reduces storage size by 50% with negligible precision loss.
fn convert_embeddings_to_f16(conn: &Connection) -> Result<()> {
    tracing::info!("Converting embeddings from f32 to f16 format...");

    // Check if there are any embeddings to convert
    let count: i64 = conn.with_connection(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM documents WHERE embedding IS NOT NULL",
            [],
            |row| row.get(0),
        )
    })?;

    if count == 0 {
        tracing::info!("No embeddings to convert");
        return Ok(());
    }

    tracing::info!("Found {} embeddings to convert", count);

    // Process in batches to avoid memory issues
    const BATCH_SIZE: i64 = 1000;
    let mut processed = 0i64;

    loop {
        let converted = conn.with_transaction(|tx| {
            // Get a batch of embeddings (f32 format: 1536 dims * 4 bytes = 6144 bytes)
            let mut stmt = tx.prepare(
                "SELECT id, embedding FROM documents 
                 WHERE embedding IS NOT NULL 
                 AND LENGTH(embedding) = 6144 
                 LIMIT ?1",
            )?;

            let rows: Vec<(i64, Vec<u8>)> = stmt
                .query_map(rusqlite::params![BATCH_SIZE], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            if rows.is_empty() {
                return Ok(0i64);
            }

            // Convert each embedding
            let count = rows.len();
            for (id, embedding_bytes) in rows {
                // Decode f32 bytes
                let f32_embedding: Vec<f32> = embedding_bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                // Encode as f16 bytes
                let f16_bytes: Vec<u8> = f32_embedding
                    .iter()
                    .flat_map(|&f| f16::from_f32(f).to_le_bytes())
                    .collect();

                // Update the document
                tx.execute(
                    "UPDATE documents SET embedding = ?1 WHERE id = ?2",
                    rusqlite::params![f16_bytes, id],
                )?;
            }

            Ok(count as i64)
        })?;

        processed += converted;

        if converted == 0 {
            break;
        }

        tracing::info!("Converted {} embeddings (total: {})", converted, processed);
    }

    tracing::info!("Completed f16 conversion: {} embeddings processed", processed);

    // Vacuum to reclaim space
    tracing::info!("Running VACUUM to reclaim disk space...");
    conn.execute_batch("VACUUM;")?;
    tracing::info!("VACUUM completed");

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
