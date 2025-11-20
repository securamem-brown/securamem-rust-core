//! SecuraMem Firewall - Semantic threat detection and OpenAI proxy with audit logging

pub mod engine;
pub mod proxy;

pub use engine::SemanticEngine;
pub use proxy::start_firewall_server;

// Re-export types needed by CLI
pub use securamem_crypto::SecuraMemSigningKey;
pub use securamem_storage::Database;
