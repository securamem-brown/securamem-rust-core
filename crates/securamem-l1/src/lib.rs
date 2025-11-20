//! L1 Compliance Layer - Audit Orchestration and Receipt Management

use securamem_storage::{Database, HashChainStore};
use securamem_crypto::SecuraMemSigningKey;
use securamem_core::Result;
use serde_json::json;
use uuid::Uuid;

/// Audit orchestrator that connects cryptography to storage
pub struct AuditOrchestrator<'a> {
    store: HashChainStore<'a>,
    signer: SecuraMemSigningKey,
}

impl<'a> AuditOrchestrator<'a> {
    pub fn new(db: &'a Database, signer: SecuraMemSigningKey) -> Self {
        Self {
            store: HashChainStore::new(db),
            signer,
        }
    }

    /// Log an event to the audit chain
    ///
    /// This orchestrates the full workflow:
    /// 1. Generate unique receipt ID
    /// 2. Prepare audit data
    /// 3. Sign the data with ED25519
    /// 4. Append to hash chain
    pub async fn log_event(&self, actor: &str, operation: &str, message: &str) -> Result<String> {
        // 1. Prepare Data
        let receipt_id = Uuid::new_v4().to_string();
        let data = json!({
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // 2. Sign Data
        // Sign the canonical JSON bytes for proof of authorship
        let data_str = data.to_string();
        let signature = self.signer.sign(data_str.as_bytes());

        // 3. Append to Chain
        let hash = self.store.append(
            &receipt_id,
            actor,
            operation,
            data,
            &signature,
            self.signer.key_id()
        ).await?;

        tracing::info!("Logged event: {} (hash: {:.8}...)", receipt_id, hash);
        Ok(receipt_id)
    }

    /// Verify the integrity of the entire audit chain
    pub async fn verify_integrity(&self) -> Result<bool> {
        self.store.verify_chain().await
    }

    /// Get the count of audit entries
    pub async fn count_entries(&self) -> Result<i64> {
        self.store.count_entries().await
    }
}
