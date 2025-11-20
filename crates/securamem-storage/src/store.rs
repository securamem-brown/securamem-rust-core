//! Hash Chain Store - Immutable append-only ledger with cryptographic verification

use crate::Database;
use securamem_core::{Result, SecuraMemError};
use securamem_crypto::hash_chain::compute_hash_chain_link;
use sqlx::Row;
use serde_json::Value;
use futures::StreamExt;

/// Hash chain store for immutable audit log entries
pub struct HashChainStore<'a> {
    db: &'a Database,
}

impl<'a> HashChainStore<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Append a new entry to the hash chain
    ///
    /// This is the critical function that:
    /// 1. Retrieves the last entry's hash
    /// 2. Computes SHA256(prev_hash || canonical_data)
    /// 3. Inserts the new immutable record
    pub async fn append(
        &self,
        receipt_id: &str,
        actor: &str,
        operation: &str,
        data: Value,
        signature: &str,
        key_id: &str,
    ) -> Result<String> {
        // 1. Get the Last Hash (The "Link")
        let last_row = sqlx::query("SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1")
            .fetch_optional(&self.db.pool)
            .await
            .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        let prev_hash = match last_row {
            Some(row) => Some(row.get::<String, _>("entry_hash")),
            None => return Err(SecuraMemError::IntegrityViolation("Genesis block missing!".into())),
        };

        // 2. Compute the New Hash (The "Anchor")
        // Use canonical JSON string for deterministic hashing
        let canonical_data = data.to_string();
        let entry_hash = compute_hash_chain_link(
            prev_hash.as_deref(),
            canonical_data.as_bytes()
        )?;

        // 3. Insert the Immutable Record
        sqlx::query(
            r#"
            INSERT INTO audit_log (
                receipt_id, actor_user_id, operation_type, audit_data,
                prev_hash, entry_hash, signature, signature_key_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(receipt_id)
        .bind(actor)
        .bind(operation)
        .bind(&canonical_data)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .bind(signature)
        .bind(key_id)
        .execute(&self.db.pool)
        .await
        .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        tracing::info!("Logged Receipt: {} | Hash: {:.8}...", receipt_id, entry_hash);
        Ok(entry_hash)
    }

    /// Verify the entire hash chain integrity
    ///
    /// Re-calculates every hash from genesis to prove:
    /// - No entries were deleted
    /// - No entries were modified
    /// - Chain linkage is intact
    pub async fn verify_chain(&self) -> Result<bool> {
        let mut rows = sqlx::query("SELECT * FROM audit_log ORDER BY id ASC")
            .fetch(&self.db.pool);

        let mut expected_prev_hash: Option<String> = None;
        let mut entry_count = 0;

        while let Some(row_result) = rows.next().await {
            let row = row_result.map_err(|e| SecuraMemError::Database(e.to_string()))?;

            let id: i64 = row.get("id");
            let operation: String = row.get("operation_type");
            let stored_prev: Option<String> = row.get("prev_hash");
            let stored_hash: String = row.get("entry_hash");
            let data_str: String = row.get("audit_data");

            entry_count += 1;

            // 1. Check Chain Linkage
            if stored_prev != expected_prev_hash {
                tracing::error!(
                    "BROKEN CHAIN at ID {}: Expected prev={:?}, Got prev={:?}",
                    id, expected_prev_hash, stored_prev
                );
                return Ok(false);
            }

            // 2. Re-calculate Hash (Proof of Integrity)
            // For genesis, we trust the initial hash (it's a bootstrap)
            if operation != "genesis" {
                let recalc_hash = compute_hash_chain_link(
                    expected_prev_hash.as_deref(),
                    data_str.as_bytes()
                )?;

                if recalc_hash != stored_hash {
                    tracing::error!(
                        "TAMPER DETECTED at ID {}: Expected hash={}, Got hash={}",
                        id, recalc_hash, stored_hash
                    );
                    return Ok(false);
                }
            }

            // Set up for next iteration
            expected_prev_hash = Some(stored_hash);
        }

        tracing::info!("✓ Chain verified: {} entries intact", entry_count);
        Ok(true)
    }

    /// Get the count of audit entries (excluding genesis)
    pub async fn count_entries(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE operation_type != 'genesis'")
            .fetch_one(&self.db.pool)
            .await
            .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        Ok(row.0)
    }
}
