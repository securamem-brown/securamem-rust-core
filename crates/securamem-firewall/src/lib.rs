//! SecuraMem Firewall - Semantic threat detection, behavioral analysis, and OpenAI proxy with audit logging

pub mod engine;
pub mod policy;
pub mod proxy;

pub use engine::SemanticEngine;
pub use policy::FirewallPolicy;
pub use proxy::start_firewall_server;

// Re-export types needed by CLI
pub use securamem_crypto::SecuraMemSigningKey;
pub use securamem_storage::Database;
