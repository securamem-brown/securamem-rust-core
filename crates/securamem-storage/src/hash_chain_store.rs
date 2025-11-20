//! Hash-chained audit log storage using SQLx
//!
//! This is the core of the immutable ledger - every entry contains a hash of
//! the previous entry, creating a tamper-evident blockchain-style chain.

use securamem_core::{Result, SecuraMemError};
use securamem_crypto::hash_chain::compute_hash_chain_link;
use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Option<i64>,
    pub receipt_id: String,
    pub timestamp: String,
    pub actor_user_id: String,
    pub operation_type: String,
    pub audit_data: serde_json::Value,
    pub prev_hash: Option<String>,
    pub entry_hash: String,
    pub signature: String,
    pub signature_key_id: String,
}

pub struct HashChainStore {
    pool: SqlitePool,
}

impl HashChainStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Append a new entry to the hash chain
    pub async fn append_entry(
        &self,
        receipt_id: String,
        actor_user_id: String,
        operation_type: String,
        audit_data: serde_json::Value,
        signature: String,
        signature_key_id: String,
    ) -> Result<AuditEntry> {
        // Step 1: Get the hash of the last entry (for chaining)
        let prev_hash = self.get_last_entry_hash().await?;

        // Step 2: Compute canonical representation of current entry data
        let canonical_data = self.compute_canonical_entry(
            &receipt_id,
            &actor_user_id,
            &operation_type,
            &audit_data,
        )?;

        // Step 3: Compute entry hash = SHA256(prev_hash || canonical_data)
        let entry_hash = compute_hash_chain_link(prev_hash.as_deref(), &canonical_data)?;

        // Step 4: Insert into database
        let result = sqlx::query(
            r#"
            INSERT INTO audit_log (
                receipt_id, actor_user_id, operation_type, audit_data,
                prev_hash, entry_hash, signature, signature_key_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&receipt_id)
        .bind(&actor_user_id)
        .bind(&operation_type)
        .bind(audit_data.to_string())
        .bind(&prev_hash)
        .bind(&entry_hash)
        .bind(&signature)
        .bind(&signature_key_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        // Step 5: Return the created entry
        Ok(AuditEntry {
            id: Some(result.last_insert_rowid()),
            receipt_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor_user_id,
            operation_type,
            audit_data,
            prev_hash,
            entry_hash,
            signature,
            signature_key_id,
        })
    }

    /// Get the hash of the last entry in the chain
    async fn get_last_entry_hash(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        Ok(row.map(|r| r.get::<String, _>("entry_hash")))
    }

    /// Compute canonical entry representation (for hashing)
    fn compute_canonical_entry(
        &self,
        receipt_id: &str,
        actor_user_id: &str,
        operation_type: &str,
        audit_data: &serde_json::Value,
    ) -> Result<Vec<u8>> {
        // Use deterministic JSON serialization (sorted keys)
        let canonical = serde_json::json!({
            "receipt_id": receipt_id,
            "actor_user_id": actor_user_id,
            "operation_type": operation_type,
            "audit_data": audit_data,
        });

        serde_json::to_vec(&canonical)
            .map_err(|e| SecuraMemError::Json(e))
    }

    /// Verify the entire hash chain integrity
    pub async fn verify_chain(&self) -> Result<bool> {
        let mut expected_prev_hash: Option<String> = None;

        // Stream all entries in order
        let mut rows = sqlx::query(
            "SELECT id, receipt_id, actor_user_id, operation_type, audit_data, prev_hash, entry_hash
             FROM audit_log ORDER BY id ASC"
        )
        .fetch(&self.pool);

        use futures::StreamExt;

        while let Some(row) = rows.next().await {
            let row = row.map_err(|e| SecuraMemError::Database(e.to_string()))?;

            let id: i64 = row.get("id");
            let receipt_id: String = row.get("receipt_id");
            let actor_user_id: String = row.get("actor_user_id");
            let operation_type: String = row.get("operation_type");
            let audit_data_str: String = row.get("audit_data");
            let prev_hash: Option<String> = row.get("prev_hash");
            let entry_hash: String = row.get("entry_hash");

            // Verify prev_hash matches expected
            if prev_hash != expected_prev_hash {
                tracing::error!(
                    entry_id = id,
                    expected_prev = ?expected_prev_hash,
                    actual_prev = ?prev_hash,
                    "Hash chain broken: prev_hash mismatch"
                );
                return Ok(false);
            }

            // Recompute entry hash
            let audit_data: serde_json::Value = serde_json::from_str(&audit_data_str)?;
            let canonical_data = self.compute_canonical_entry(
                &receipt_id,
                &actor_user_id,
                &operation_type,
                &audit_data,
            )?;

            let computed_hash = compute_hash_chain_link(
                prev_hash.as_deref(),
                &canonical_data,
            )?;

            // Verify computed hash matches stored hash
            if computed_hash != entry_hash {
                tracing::error!(
                    entry_id = id,
                    expected_hash = computed_hash,
                    actual_hash = entry_hash,
                    "Hash chain broken: entry_hash mismatch"
                );
                return Ok(false);
            }

            // Update expected prev_hash for next iteration
            expected_prev_hash = Some(entry_hash);
        }

        Ok(true)
    }

    /// Get a specific audit entry by receipt ID
    pub async fn get_entry(&self, receipt_id: &str) -> Result<Option<AuditEntry>> {
        let row = sqlx::query(
            r#"
            SELECT id, receipt_id, timestamp, actor_user_id, operation_type,
                   audit_data, prev_hash, entry_hash, signature, signature_key_id
            FROM audit_log
            WHERE receipt_id = ?
            "#
        )
        .bind(receipt_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SecuraMemError::Database(e.to_string()))?;

        if let Some(row) = row {
            let audit_data_str: String = row.get("audit_data");
            let audit_data: serde_json::Value = serde_json::from_str(&audit_data_str)?;

            Ok(Some(AuditEntry {
                id: Some(row.get("id")),
                receipt_id: row.get("receipt_id"),
                timestamp: row.get("timestamp"),
                actor_user_id: row.get("actor_user_id"),
                operation_type: row.get("operation_type"),
                audit_data,
                prev_hash: row.get("prev_hash"),
                entry_hash: row.get("entry_hash"),
                signature: row.get("signature"),
                signature_key_id: row.get("signature_key_id"),
            }))
        } else {
            Ok(None)
        }
    }
}
