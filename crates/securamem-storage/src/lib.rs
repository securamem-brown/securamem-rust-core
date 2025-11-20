//! SecuraMem Storage - Immutable hash-chain audit log with SQLx

use sqlx::sqlite::{SqlitePool, SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use std::path::Path;
use std::str::FromStr;
use securamem_core::{Result, SecuraMemError};

pub mod hash_chain_store;
pub mod store;

// Export only the store module (the newer implementation)
pub use store::HashChainStore;

// Re-export sqlx for other crates
pub use sqlx;

/// Database wrapper with embedded migrations
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    /// Initialize the database: Create file, set options, run migrations
    pub async fn init(db_path: &Path) -> Result<Self> {
        // 1. Ensure directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(SecuraMemError::Io)?;
        }

        // 2. Configure SQLite options
        // We use WAL mode for high concurrency (async writes)
        let options = SqliteConnectOptions::from_str(db_path.to_str().unwrap())
            .map_err(|e| SecuraMemError::Database(e.to_string()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);

        // 3. Connect
        let pool = SqlitePool::connect_with(options).await
            .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        // 4. Run Migrations (Embedded in binary!)
        // This looks for the "migrations" folder relative to CARGO_MANIFEST_DIR
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .map_err(|e| SecuraMemError::Database(format!("Migration failed: {}", e)))?;

        tracing::info!("Database initialized at {:?}", db_path);

        Ok(Self { pool })
    }

    /// Check health
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| SecuraMemError::Database(e.to_string()))?;
        Ok(())
    }
}
