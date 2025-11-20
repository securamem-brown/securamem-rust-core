//! SecuraMem Core - Shared primitives and error types
//!
//! This crate is the dependency root - it has zero internal dependencies.
//! All other crates depend on this one.

use thiserror::Error;

/// Global error type for SecuraMem
#[derive(Error, Debug)]
pub enum SecuraMemError {
    // === Database Errors ===
    #[error("Database error: {0}")]
    Database(String),

    #[error("Database migration failed: {0}")]
    Migration(String),

    #[error("Transaction failed: {0}")]
    Transaction(String),

    // === Cryptography Errors ===
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Key not found: {key_id}")]
    KeyNotFound { key_id: String },

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Invalid signature format: {0}")]
    InvalidSignature(String),

    // === Hash Chain Errors ===
    #[error("Hash chain broken at entry {entry_id}: {reason}")]
    HashChainBroken { entry_id: i64, reason: String },

    #[error("Receipt hash mismatch: expected {expected}, got {actual}")]
    ReceiptHashMismatch { expected: String, actual: String },

    #[error("Invalid previous hash: {0}")]
    InvalidPreviousHash(String),

    #[error("Integrity violation: {0}")]
    IntegrityViolation(String),

    // === Network Errors ===
    #[error("HTTP server error: {0}")]
    HttpServer(String),

    #[error("Attempted to bind to non-localhost address: {addr}")]
    NonLocalhostBind { addr: String },

    #[error("Port {port} already in use")]
    PortInUse { port: u16 },

    // === I/O Errors ===
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid UTF-8 in file: {path}")]
    InvalidUtf8 { path: String },

    // === Parsing Errors ===
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    // === RFC 3161 Errors ===
    #[error("RFC 3161 timestamp request failed: {0}")]
    Rfc3161RequestFailed(String),

    #[error("RFC 3161 timestamp verification failed: {0}")]
    Rfc3161VerificationFailed(String),

    #[error("TSA certificate invalid: {0}")]
    TsaCertificateInvalid(String),

    // === Compliance Errors ===
    #[error("Policy violation: {policy} - {reason}")]
    PolicyViolation { policy: String, reason: String },

    #[error("Compliance check failed: {framework}")]
    ComplianceCheckFailed { framework: String },

    #[error("Approval required for operation: {operation}")]
    ApprovalRequired { operation: String },

    #[error("Retention policy violation: {0}")]
    RetentionPolicyViolation(String),

    // === Configuration Errors ===
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid configuration key: {key}")]
    InvalidConfigKey { key: String },

    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    // === License Errors ===
    #[error("License error: {0}")]
    LicenseError(String),

    #[error("License expired at {expired_at}")]
    LicenseExpired { expired_at: String },

    #[error("No valid license found")]
    LicenseNotFound,

    // === Generic Errors ===
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not implemented: {feature}")]
    NotImplemented { feature: String },

    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

/// Convenience Result type using SecuraMemError
pub type Result<T> = std::result::Result<T, SecuraMemError>;

/// Localhost-only socket address for air-gap enforcement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalhostAddr {
    port: u16,
}

impl LocalhostAddr {
    /// Create a new localhost address with the given port
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Get the port number
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Convert to a SocketAddr (always 127.0.0.1)
    pub fn to_socket_addr(&self) -> std::net::SocketAddr {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.port)
    }
}

impl std::fmt::Display for LocalhostAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "127.0.0.1:{}", self.port)
    }
}

impl std::str::FromStr for LocalhostAddr {
    type Err = SecuraMemError;

    fn from_str(s: &str) -> Result<Self> {
        // If just a port number
        if let Ok(port) = s.parse::<u16>() {
            return Ok(LocalhostAddr::new(port));
        }

        // If a socket address
        if let Ok(addr) = s.parse::<std::net::SocketAddr>() {
            if addr.ip().is_loopback() {
                return Ok(LocalhostAddr::new(addr.port()));
            } else {
                return Err(SecuraMemError::NonLocalhostBind {
                    addr: addr.to_string(),
                });
            }
        }

        Err(SecuraMemError::Config(format!(
            "Invalid localhost address: {}. Must be a port number (e.g., '9091') or 127.0.0.1:port",
            s
        )))
    }
}

/// Actor/Principal information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Actor {
    pub user_id: String,
    pub username: Option<String>,
    pub role: Option<String>,
    pub authentication_method: Option<String>,
}

impl Actor {
    /// Create a new actor
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            username: None,
            role: None,
            authentication_method: None,
        }
    }

    /// Create actor from OS user
    pub fn from_os_user() -> Self {
        let username = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            user_id: format!("os_user_{}", username),
            username: Some(username),
            role: Some("user".to_string()),
            authentication_method: Some("os_user".to_string()),
        }
    }
}

/// Configuration for audit system
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditConfig {
    pub database_path: std::path::PathBuf,
    pub retention_days: u32,
    pub enable_rfc3161: bool,
    pub tsa_url: Option<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            database_path: std::path::PathBuf::from(".securamem/audit.db"),
            retention_days: 2555, // 7 years (GDPR default)
            enable_rfc3161: false,
            tsa_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_addr_from_port() {
        let addr: LocalhostAddr = "9091".parse().unwrap();
        assert_eq!(addr.port(), 9091);
        assert_eq!(addr.to_string(), "127.0.0.1:9091");
    }

    #[test]
    fn test_localhost_addr_from_socket() {
        let addr: LocalhostAddr = "127.0.0.1:8080".parse().unwrap();
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_localhost_addr_rejects_external() {
        let result: Result<LocalhostAddr> = "0.0.0.0:9091".parse();
        assert!(result.is_err());
        assert!(matches!(result, Err(SecuraMemError::NonLocalhostBind { .. })));
    }

    #[test]
    fn test_actor_from_os_user() {
        let actor = Actor::from_os_user();
        assert!(actor.user_id.starts_with("os_user_"));
        assert_eq!(actor.role, Some("user".to_string()));
    }
}

// === License & Identity Modules ===
pub mod license;
pub mod identity;
