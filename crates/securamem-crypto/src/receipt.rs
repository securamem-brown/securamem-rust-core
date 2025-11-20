//! Receipt structure and builder for audit entries

use securamem_core::{Result, Actor};
use serde::{Deserialize, Serialize};

/// Audit receipt structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub receipt_id: String,
    pub timestamp: String,
    pub schema_version: String,
    pub actor: Actor,
    pub context: ReceiptContext,
    pub output: ReceiptOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ReceiptMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptContext {
    pub operation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_resources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_assessment: Option<RiskAssessment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_level: RiskLevel,
    pub risk_factors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptOutput {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_resources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_approval: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_period: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,
}

/// Receipt builder for constructing audit receipts
pub struct ReceiptBuilder {
    receipt_id: String,
    actor: Actor,
    operation_type: String,
    command: Option<String>,
    parameters: Option<serde_json::Value>,
    success: bool,
    result_summary: Option<String>,
    error_message: Option<String>,
}

impl ReceiptBuilder {
    /// Create a new receipt builder
    pub fn new(receipt_id: impl Into<String>, actor: Actor, operation_type: impl Into<String>) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            actor,
            operation_type: operation_type.into(),
            command: None,
            parameters: None,
            success: true,
            result_summary: None,
            error_message: None,
        }
    }

    /// Set the command
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Set the parameters
    pub fn parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = Some(parameters);
        self
    }

    /// Set success status
    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set result summary
    pub fn result_summary(mut self, summary: impl Into<String>) -> Self {
        self.result_summary = Some(summary.into());
        self
    }

    /// Set error message
    pub fn error_message(mut self, error: impl Into<String>) -> Self {
        self.error_message = Some(error.into());
        self.success = false;
        self
    }

    /// Build the receipt
    pub fn build(self) -> Receipt {
        Receipt {
            receipt_id: self.receipt_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            schema_version: "2.0".to_string(),
            actor: self.actor,
            context: ReceiptContext {
                operation_type: self.operation_type,
                command: self.command,
                parameters: self.parameters,
                target_resources: None,
                risk_assessment: None,
            },
            output: ReceiptOutput {
                success: self.success,
                result_summary: self.result_summary,
                affected_resources: None,
                error_message: self.error_message,
            },
            meta: None,
        }
    }
}

impl Receipt {
    /// Create a builder for a new receipt
    pub fn builder(receipt_id: impl Into<String>, actor: Actor, operation_type: impl Into<String>) -> ReceiptBuilder {
        ReceiptBuilder::new(receipt_id, actor, operation_type)
    }

    /// Serialize to canonical JSON (deterministic ordering)
    pub fn to_canonical_json(&self) -> Result<String> {
        // serde_json serializes maps in insertion order, which is deterministic
        // for our struct-based serialization
        serde_json::to_string(self).map_err(Into::into)
    }

    /// Serialize to canonical bytes for signing
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.to_canonical_json()?.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_builder() {
        let actor = Actor::new("test_user");
        let receipt = Receipt::builder("r_test_123", actor, "cli_command")
            .command("smem verify")
            .parameters(serde_json::json!({"args": ["--all"]}))
            .result_summary("Verification complete")
            .build();

        assert_eq!(receipt.receipt_id, "r_test_123");
        assert_eq!(receipt.context.operation_type, "cli_command");
        assert_eq!(receipt.context.command, Some("smem verify".to_string()));
        assert!(receipt.output.success);
    }

    #[test]
    fn test_receipt_serialization() {
        let actor = Actor::new("test_user");
        let receipt = Receipt::builder("r_test_456", actor, "test_op")
            .result_summary("Test")
            .build();

        let json = receipt.to_canonical_json().unwrap();
        assert!(json.contains("r_test_456"));
        assert!(json.contains("test_op"));

        // Deserialize
        let parsed: Receipt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.receipt_id, receipt.receipt_id);
    }

    #[test]
    fn test_receipt_determinism() {
        let actor = Actor::new("test_user");
        let receipt = Receipt::builder("r_deterministic", actor.clone(), "test")
            .parameters(serde_json::json!({"key": "value"}))
            .build();

        let json1 = receipt.to_canonical_json().unwrap();
        let json2 = receipt.to_canonical_json().unwrap();

        assert_eq!(json1, json2);
    }
}
