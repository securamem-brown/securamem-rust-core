//! L1 Compliance Layer - Audit Orchestration, Risk Assessment, and Session Analysis
//!
//! This layer bridges cryptographic primitives to the storage layer and provides:
//! 1. Audit event orchestration (sign -> hash-chain -> store)
//! 2. Interaction risk scoring from firewall audit data
//! 3. Session compliance summaries for AIGP attestation
//! 4. Chain integrity verification

use securamem_storage::{Database, HashChainStore};
use securamem_crypto::SecuraMemSigningKey;
use securamem_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
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
    /// Orchestrates: UUID generation -> JSON preparation -> ED25519 signing -> hash-chain append
    pub async fn log_event(&self, actor: &str, operation: &str, message: &str) -> Result<String> {
        let receipt_id = Uuid::new_v4().to_string();
        let data = json!({
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let data_str = data.to_string();
        let signature = self.signer.sign(data_str.as_bytes());

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

    /// Get the count of audit entries (excluding genesis)
    pub async fn count_entries(&self) -> Result<i64> {
        self.store.count_entries().await
    }
}

/// Risk level classification for interactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Nominal,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Nominal => write!(f, "nominal"),
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Compliance summary for a set of interactions
#[derive(Debug, Serialize)]
pub struct ComplianceSummary {
    pub generated_at: String,
    pub total_interactions: i64,
    pub blocked_count: i64,
    pub allowed_count: i64,
    pub risk_distribution: RiskDistribution,
    pub avg_coherence: Option<f64>,
    pub chain_integrity: bool,
    pub policy_version: Option<String>,
    pub session_count: i64,
    pub flags: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct RiskDistribution {
    pub nominal: i64,
    pub low: i64,
    pub medium: i64,
    pub high: i64,
    pub critical: i64,
}

/// Compliance analyzer that queries the audit chain for risk assessment
pub struct ComplianceAnalyzer<'a> {
    db: &'a Database,
}

impl<'a> ComplianceAnalyzer<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Generate a compliance summary from the audit chain
    ///
    /// This analyzes all firewall interactions to produce:
    /// - Risk distribution across all interactions
    /// - Average coherence scores (proxy for deceptive alignment detection)
    /// - Block/allow ratios
    /// - Chain integrity verification
    /// - Compliance flags for AIGP attestation
    pub async fn generate_summary(&self) -> Result<ComplianceSummary> {
        // Count total interactions
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_log WHERE operation_type IN ('interaction_audit', 'firewall_decision')"
        )
        .fetch_one(&self.db.pool)
        .await
        .map_err(|e| securamem_core::SecuraMemError::Database(e.to_string()))?;

        // Count blocked prompts
        let blocked: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_log WHERE operation_type = 'prompt_blocked'"
        )
        .fetch_one(&self.db.pool)
        .await
        .map_err(|e| securamem_core::SecuraMemError::Database(e.to_string()))?;

        // Analyze risk distribution and coherence from audit data
        let entries = sqlx::query(
            "SELECT audit_data FROM audit_log WHERE operation_type = 'interaction_audit' ORDER BY id DESC LIMIT 1000"
        )
        .fetch_all(&self.db.pool)
        .await
        .map_err(|e| securamem_core::SecuraMemError::Database(e.to_string()))?;

        let mut risk_dist = RiskDistribution::default();
        let mut coherence_sum = 0.0f64;
        let mut coherence_count = 0i64;
        let mut sessions = std::collections::HashSet::new();
        let mut latest_policy_version: Option<String> = None;

        for row in &entries {
            let data_str: String = row.get("audit_data");
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_str) {
                // Extract message field (our audit data is nested inside "message")
                let audit = if let Some(msg) = data.get("message") {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(
                        msg.as_str().unwrap_or("")
                    ) {
                        parsed
                    } else {
                        data.clone()
                    }
                } else {
                    data.clone()
                };

                // Risk level
                if let Some(risk) = audit.pointer("/analysis/risk_level").and_then(|r| r.as_str()) {
                    match risk {
                        "nominal" => risk_dist.nominal += 1,
                        "low" => risk_dist.low += 1,
                        "medium" => risk_dist.medium += 1,
                        "high" => risk_dist.high += 1,
                        "critical" => risk_dist.critical += 1,
                        _ => risk_dist.nominal += 1,
                    }
                }

                // Coherence score
                if let Some(coherence) = audit.pointer("/analysis/coherence_score").and_then(|c| c.as_f64()) {
                    coherence_sum += coherence;
                    coherence_count += 1;
                }

                // Session tracking
                if let Some(sid) = audit.pointer("/session/id").and_then(|s| s.as_str()) {
                    sessions.insert(sid.to_string());
                }

                // Policy version
                if latest_policy_version.is_none() {
                    if let Some(pv) = audit.get("policy_version").and_then(|v| v.as_str()) {
                        latest_policy_version = Some(pv.to_string());
                    }
                }
            }
        }

        let avg_coherence = if coherence_count > 0 {
            Some(coherence_sum / coherence_count as f64)
        } else {
            None
        };

        // Verify chain integrity
        let store = HashChainStore::new(self.db);
        let chain_integrity = store.verify_chain().await.unwrap_or(false);

        // Generate compliance flags
        let mut flags = Vec::new();

        if !chain_integrity {
            flags.push("CRITICAL: Audit chain integrity compromised".to_string());
        }

        if risk_dist.high + risk_dist.critical > 0 {
            flags.push(format!(
                "WARNING: {} high/critical risk interactions detected",
                risk_dist.high + risk_dist.critical
            ));
        }

        if let Some(avg_c) = avg_coherence {
            if avg_c < 0.15 {
                flags.push(format!(
                    "WARNING: Low average coherence ({:.3}) - possible systematic deceptive alignment",
                    avg_c
                ));
            }
        }

        if blocked.0 as f64 / (total.0.max(1) as f64) > 0.3 {
            flags.push(format!(
                "NOTICE: High block rate ({:.1}%) - review policy thresholds",
                blocked.0 as f64 / total.0.max(1) as f64 * 100.0
            ));
        }

        if flags.is_empty() {
            flags.push("All compliance checks passed".to_string());
        }

        Ok(ComplianceSummary {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_interactions: total.0,
            blocked_count: blocked.0,
            allowed_count: total.0 - blocked.0,
            risk_distribution: risk_dist,
            avg_coherence,
            chain_integrity,
            policy_version: latest_policy_version,
            session_count: sessions.len() as i64,
            flags,
        })
    }
}