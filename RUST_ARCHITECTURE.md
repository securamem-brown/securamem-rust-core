# SecuraMem Rust Refactoring Architecture

**Version:** 2.0 (Audit-Only Scope)
**Date:** 2025-01-18
**Purpose:** Complete architectural blueprint for refactoring SecuraMem into a high-performance **AI Black Box Recorder** - an immutable audit ledger for AI system compliance

---

## Executive Summary

This document provides a comprehensive architecture for migrating SecuraMem from a Node.js application (TypeScript) to a Rust-based single-binary executable focused exclusively on **audit trail integrity and compliance logging**.

**Strategic Pivot:** SecuraMem is now a **pure audit & compliance log** - removing all vector memory (L2) capabilities. This is an AI equivalent of an aircraft black box: intercept data, sign it, timestamp it (RFC 3161), store it immutably, and prove integrity cryptographically.

**Key Goals:**
- Single-binary deployment with embedded static assets
- **Immutable hash-chain ledger** (blockchain-style audit log)
- **RFC 3161 timestamping** for legal-grade non-repudiation
- Zero runtime dependencies (air-gapped deployment)
- Memory-safe cryptographic operations (ED25519 + SHA-256)
- Strict localhost binding enforced at compile time
- **Sub-millisecond append latency** for high-throughput logging
- Binary size target: <30MB (vs. current ~200MB node_modules)

---

## 1. Crate Strategy: Rust Workspace Architecture

### 1.1 Workspace Structure (Audit-Only Scope)

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/securamem-cli",       # Binary crate (entry point)
    "crates/securamem-l1",         # L1: Compliance & Audit library
    "crates/securamem-storage",    # L2 renamed: High-performance immutable storage
    "crates/securamem-l3",         # L3: Monitoring & API library
    "crates/securamem-core",       # Shared core primitives
    "crates/securamem-crypto",     # Cryptography primitives (ED25519, SHA-256, RFC 3161)
]
resolver = "2"

[workspace.package]
version = "2.0.0"
edition = "2021"
authors = ["SecuraMem Team"]
license = "PROPRIETARY"
rust-version = "1.75"

[workspace.dependencies]
# === Async Runtime ===
tokio = { version = "1.35", features = ["full", "tracing"] }
tokio-util = { version = "0.7", features = ["io"] }

# === Database (SQLx Only - No rusqlite) ===
sqlx = { version = "0.7", features = [
    "sqlite",
    "runtime-tokio-rustls",
    "migrate",
    "chrono",
    "uuid"
] }

# === Cryptography (ED25519 + SHA-256 only) ===
ed25519-dalek = { version = "2.1", features = ["rand_core", "pkcs8", "pem"] }
signature = "2.2"  # Trait for ed25519-dalek
ring = "0.17"      # SHA-256 hashing (constant-time)
sha2 = "0.10"      # Fallback SHA-256 (pure Rust)
rand_core = { version = "0.6", features = ["getrandom"] }

# === RFC 3161 Timestamping ===
x509-parser = "0.16"    # Parse X.509 certificates in TSP responses
asn1-rs = "0.6"          # ASN.1 DER encoding/decoding
der = "0.7"              # DER encoding (for TimeStampReq)
reqwest = { version = "0.11", features = ["blocking", "rustls-tls"], optional = true }  # TSA HTTP client (optional for air-gap)

# === Web Server & Metrics ===
axum = { version = "0.7", features = ["tracing", "tower-log"] }
hyper = { version = "1.0", features = ["server", "http1", "http2"] }
tower = { version = "0.4", features = ["timeout", "limit"] }
tower-http = { version = "0.5", features = ["trace", "compression-gzip"] }
prometheus = "0.13"

# === CLI ===
clap = { version = "4.4", features = ["derive", "env", "wrap_help"] }
colored = "2.1"

# === Serialization ===
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# === Error Handling & Logging ===
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# === Utilities ===
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
hex = "0.4"
base64 = "0.21"
flate2 = "1.0"

[features]
default = []
tsa-client = ["reqwest"]  # Enable RFC 3161 TSA HTTP client (disable for air-gap)
```

### 1.2 Crate Responsibilities (Refactored for Audit-Only)

#### `securamem-cli` (Binary Crate)
- **Type:** Binary (`src/main.rs`)
- **Purpose:** CLI entry point, command routing, initialization
- **Dependencies:** All service crates (L1, storage, L3), clap
- **Responsibilities:**
  - Parse CLI arguments using clap derive macros
  - Initialize Tokio runtime with optimized settings for DB writes
  - Route commands to appropriate service layers
  - Handle graceful shutdown signals (SIGINT/SIGTERM)
  - Emit final audit receipts and exit codes

#### `securamem-core` (Library Crate)
- **Type:** Library (`src/lib.rs`)
- **Purpose:** Shared primitives and traits
- **Exports:**
  - `SecuraMemError` enum (via thiserror) - comprehensive error taxonomy
  - `Result<T>` type alias
  - Configuration structs (`AuditConfig`, `CryptoConfig`, `StorageConfig`)
  - Service traits (`AuditService`, `StorageService`, `ServiceContainer`)
  - Actor/Identity types (`Actor`, `Principal`)
  - Constants (paths, version strings, schema versions)
  - `LocalhostAddr` type for enforced localhost binding
- **Zero dependencies on other internal crates** (dependency graph root)

#### `securamem-crypto` (Library Crate)
- **Type:** Library
- **Purpose:** Cryptographic primitives for audit integrity
- **Exports:**
  - **`SigningKey`** - ED25519 key pair management
  - **`Signer`** and **`Verifier`** traits (from `signature` crate)
  - **`Receipt`** - Audit receipt builder and validator
  - **`HashChain`** - Blockchain-style linked hash verification
  - **`Rfc3161Client`** - RFC 3161 Time-Stamp Protocol client
  - **`Sha256`** - Hash digest utilities (using `ring`)
- **Dependencies:** `ed25519-dalek`, `ring`, `sha2`, `x509-parser`, `asn1-rs`, `der`, `securamem-core`
- **Key Functions:**
  - `sign_receipt(receipt: &Receipt, key: &SigningKey) -> Signature`
  - `verify_receipt(receipt: &Receipt, signature: &Signature, public_key: &PublicKey) -> bool`
  - `compute_hash_chain_link(prev_hash: &[u8], current_data: &[u8]) -> [u8; 32]`
  - `request_rfc3161_timestamp(data: &[u8], tsa_url: &str) -> Result<TimeStampToken>`

#### `securamem-storage` (Library Crate) — **Formerly `securamem-l2`**
- **Type:** Library
- **Purpose:** High-performance immutable audit log storage
- **Exports:**
  - **`AuditLogStore`** - Main storage interface
  - **`AuditEntry`** - Core audit record structure
  - **`HashChainStore`** - Hash-chained append-only log implementation
  - **`QueryBuilder`** - SQL query builder for audit searches
  - **`StorageMigrations`** - Database schema versioning
- **Dependencies:** `sqlx` (async SQLite), `securamem-core`, `securamem-crypto`
- **Key Features:**
  - Async append-only writes (sub-millisecond latency target)
  - Hash chain integrity verification
  - JSON blob storage with indexed metadata
  - WAL mode for concurrent read/write
  - Prepared statement caching

#### `securamem-l1` (Library Crate)
- **Type:** Library
- **Purpose:** L1 Compliance & Audit Orchestration
- **Exports:**
  - **`AuditOrchestrator`** - Coordinates receipt generation, signing, and storage
  - **`ReceiptService`** - Receipt lifecycle management
  - **`ComplianceChecker`** - GDPR, EU AI Act, NIST RMF validation
  - **`AuditTrailVerifier`** - End-to-end audit chain verification
  - **`PolicyEngine`** - Zero-trust policy enforcement
- **Dependencies:** `securamem-core`, `securamem-crypto`, `securamem-storage`
- **Key Workflows:**
  1. Receive audit event → Generate receipt → Sign → Hash-chain → Persist
  2. RFC 3161 timestamp request (optional, if TSA configured)
  3. Compliance checks (GDPR retention, EU AI Act risk assessment)

#### `securamem-l3` (Library Crate)
- **Type:** Library
- **Purpose:** L3 Monitoring, Metrics & HTTP API
- **Exports:**
  - **`PrometheusExporter`** - Metrics HTTP server (`/metrics`, `/dashboard`)
  - **`TelemetryCollector`** - Audit log statistics aggregator
  - **`HealthChecker`** - Service diagnostics and readiness probes
  - **`ApiServer`** - REST API for audit queries (read-only)
- **Dependencies:** `securamem-core`, `securamem-l1`, `securamem-storage`, `axum`, `prometheus`
- **API Endpoints:**
  - `GET /metrics` - Prometheus metrics
  - `GET /health` - Health check
  - `GET /audit/{receipt_id}` - Retrieve audit receipt
  - `POST /audit/verify` - Verify audit chain integrity
  - `GET /dashboard` - Live HTML dashboard

### 1.3 Dependency Graph (Refactored)

```
securamem-cli (bin)
    ├── securamem-l1 (lib) [Compliance Orchestration]
    │   ├── securamem-crypto [ED25519, SHA-256, RFC 3161]
    │   │   └── securamem-core
    │   ├── securamem-storage [Immutable Audit Log]
    │   │   ├── securamem-crypto
    │   │   │   └── securamem-core
    │   │   └── securamem-core
    │   └── securamem-core
    ├── securamem-storage (lib) [Database Layer]
    │   ├── securamem-crypto
    │   │   └── securamem-core
    │   └── securamem-core
    ├── securamem-l3 (lib) [Monitoring & API]
    │   ├── securamem-l1
    │   ├── securamem-storage
    │   └── securamem-core
    └── securamem-core
```

**Key Principles:**
- **Acyclic:** No circular dependencies (enforced by Cargo)
- **Layered:** L3 depends on L1/storage, but not vice versa
- **Core-first:** All crates depend on `securamem-core`, which has zero internal dependencies
- **Crypto-isolated:** Cryptography is a shared dependency, preventing duplication

---

## 2. Hash-Chain Immutable Ledger Implementation

### 2.1 Concept: Blockchain-Style Audit Log

Each audit entry contains a cryptographic hash of the previous entry, creating a tamper-evident chain. Any modification to a historical entry breaks the chain, making it immediately detectable.

**Chain Structure:**
```
Entry 1: hash(NULL || entry_data_1) = H1
Entry 2: hash(H1 || entry_data_2) = H2
Entry 3: hash(H2 || entry_data_3) = H3
...
Entry N: hash(H(N-1) || entry_data_N) = HN
```

### 2.2 Database Schema (SQLx Migration)

```sql
-- migrations/001_audit_log_schema.sql
-- SecuraMem Immutable Audit Log Schema
-- Version: 2.0 (Audit-Only)

-- === Core Audit Log Table ===
CREATE TABLE IF NOT EXISTS audit_log (
    -- Primary Key (AUTOINCREMENT for sequential ordering)
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,

    -- Receipt Identifier (UUID v4)
    receipt_id TEXT UNIQUE NOT NULL,

    -- Timestamp (ISO 8601 with timezone)
    timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),

    -- Actor/Principal (who performed the action)
    actor_user_id TEXT NOT NULL,
    actor_username TEXT,
    actor_role TEXT,

    -- Operation Context
    operation_type TEXT NOT NULL,  -- e.g., 'cli_command', 'api_call', 'recall', 'index'
    command TEXT,                   -- e.g., 'smem recall', 'smem verify'

    -- Audit Data (JSON blob for flexibility)
    audit_data TEXT NOT NULL,  -- JSON: { parameters, input, output, metadata }

    -- Hash Chain (Immutability Enforcement)
    prev_hash TEXT,            -- SHA-256 of previous entry (NULL for genesis entry)
    entry_hash TEXT NOT NULL,  -- SHA-256 of (prev_hash || entry_data)

    -- Cryptographic Signature (ED25519)
    signature TEXT NOT NULL,         -- Base64-encoded ED25519 signature of entry_hash
    signature_key_id TEXT NOT NULL,  -- Key fingerprint (e.g., 'ed25519:sha256:abc123...')

    -- RFC 3161 Timestamp (Optional, for legal-grade non-repudiation)
    rfc3161_timestamp_token BLOB,   -- DER-encoded TimeStampToken
    rfc3161_tsa_url TEXT,            -- TSA URL used for timestamping
    rfc3161_verified BOOLEAN,        -- Whether TST was verified

    -- Compliance Metadata
    retention_until TEXT,       -- ISO 8601 date (GDPR right to erasure deadline)
    sensitivity_level TEXT,     -- e.g., 'public', 'internal', 'confidential', 'restricted'
    compliance_flags TEXT,      -- JSON: { gdpr_compliant, eu_ai_act_risk_level, nist_rmf_cat }

    -- Indexing for fast queries
    CONSTRAINT unique_receipt UNIQUE (receipt_id),
    CONSTRAINT valid_prev_hash CHECK (prev_hash IS NULL OR length(prev_hash) = 64),
    CONSTRAINT valid_entry_hash CHECK (length(entry_hash) = 64)
);

-- === Indexes for Query Performance ===
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor_user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_operation ON audit_log(operation_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_receipt_id ON audit_log(receipt_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_entry_hash ON audit_log(entry_hash);

-- === Cryptographic Keys Table ===
CREATE TABLE IF NOT EXISTS signing_keys (
    key_id TEXT PRIMARY KEY NOT NULL,
    key_fingerprint TEXT UNIQUE NOT NULL,  -- SHA-256 of public key
    public_key_pem TEXT NOT NULL,
    private_key_encrypted BLOB,            -- Encrypted private key (for backup)
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    rotated_at TEXT,
    status TEXT NOT NULL DEFAULT 'active',  -- 'active', 'rotated', 'revoked'

    CONSTRAINT valid_status CHECK (status IN ('active', 'rotated', 'revoked'))
);

-- === Audit Chain Checkpoints (for fast verification) ===
CREATE TABLE IF NOT EXISTS audit_checkpoints (
    checkpoint_id INTEGER PRIMARY KEY AUTOINCREMENT,
    checkpoint_timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    last_entry_id INTEGER NOT NULL,
    last_entry_hash TEXT NOT NULL,
    entry_count INTEGER NOT NULL,
    checkpoint_signature TEXT NOT NULL,  -- Signature over (last_entry_hash || entry_count)

    FOREIGN KEY (last_entry_id) REFERENCES audit_log(id)
);

-- === Compliance Policy Rules ===
CREATE TABLE IF NOT EXISTS compliance_policies (
    policy_id TEXT PRIMARY KEY NOT NULL,
    policy_name TEXT NOT NULL,
    policy_type TEXT NOT NULL,  -- 'gdpr', 'eu_ai_act', 'nist_rmf', 'custom'
    policy_rules TEXT NOT NULL,  -- JSON policy definition
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'utc'))
);

-- === Telemetry Metrics (for Prometheus) ===
CREATE TABLE IF NOT EXISTS telemetry_metrics (
    metric_id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_labels TEXT  -- JSON: { label_key: label_value }
);

CREATE INDEX IF NOT EXISTS idx_telemetry_timestamp ON telemetry_metrics(metric_timestamp DESC);

-- === Genesis Entry (Chain Initialization) ===
-- Insert the genesis entry to bootstrap the hash chain
INSERT INTO audit_log (
    receipt_id,
    timestamp,
    actor_user_id,
    actor_username,
    actor_role,
    operation_type,
    command,
    audit_data,
    prev_hash,
    entry_hash,
    signature,
    signature_key_id,
    compliance_flags
) VALUES (
    'genesis-00000000-0000-0000-0000-000000000000',
    datetime('now', 'utc'),
    'system',
    'SecuraMem',
    'system',
    'init',
    'smem init',
    '{"message":"SecuraMem audit log initialized","schema_version":"2.0"}',
    NULL,  -- Genesis has no previous hash
    '0000000000000000000000000000000000000000000000000000000000000000',  -- Placeholder, will be replaced
    'genesis_signature_placeholder',
    'ed25519:genesis',
    '{"gdpr_compliant":true,"eu_ai_act_risk_level":"minimal","nist_rmf_cat":"low"}'
) ON CONFLICT(receipt_id) DO NOTHING;
```

### 2.3 Hash Chain Implementation (Rust with sqlx)

```rust
// In crates/securamem-storage/src/hash_chain.rs

use ring::digest::{digest, SHA256};
use sqlx::{SqlitePool, Row};
use securamem_core::{Result, SecuraMemError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
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
        let entry_hash = self.compute_hash_chain_link(prev_hash.as_deref(), &canonical_data)?;

        // Step 4: Insert into database
        sqlx::query(
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
        .map_err(|e| SecuraMemError::Database(e))?;

        // Step 5: Return the created entry
        Ok(AuditEntry {
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
        .map_err(|e| SecuraMemError::Database(e))?;

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

    /// Compute hash chain link: SHA256(prev_hash || current_data)
    fn compute_hash_chain_link(
        &self,
        prev_hash: Option<&str>,
        current_data: &[u8],
    ) -> Result<String> {
        let mut input = Vec::new();

        // Prepend previous hash (or empty if genesis)
        if let Some(prev) = prev_hash {
            input.extend_from_slice(prev.as_bytes());
        }

        // Append current entry data
        input.extend_from_slice(current_data);

        // Compute SHA-256 hash
        let hash_bytes = digest(&SHA256, &input);
        let hash_hex = hex::encode(hash_bytes.as_ref());

        Ok(hash_hex)
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

        use sqlx::Row;
        use futures::StreamExt;

        while let Some(row) = rows.next().await {
            let row = row.map_err(|e| SecuraMemError::Database(e))?;

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

            let computed_hash = self.compute_hash_chain_link(
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
}
```

### 2.4 Usage Example

```rust
// In crates/securamem-l1/src/audit_orchestrator.rs

use securamem_storage::HashChainStore;
use securamem_crypto::{SigningKey, Signer};
use serde_json::json;

pub async fn log_audit_event(
    store: &HashChainStore,
    signing_key: &SigningKey,
    actor: &str,
    operation: &str,
    data: serde_json::Value,
) -> Result<()> {
    // Generate receipt ID
    let receipt_id = format!("r_{}", uuid::Uuid::new_v4());

    // Sign the audit data
    let canonical_data = serde_json::to_vec(&data)?;
    let signature = signing_key.sign(&canonical_data);
    let signature_b64 = base64::encode(signature.as_ref());

    // Append to hash chain
    let entry = store.append_entry(
        receipt_id,
        actor.to_string(),
        operation.to_string(),
        data,
        signature_b64,
        signing_key.key_id(),
    ).await?;

    tracing::info!(
        receipt_id = %entry.receipt_id,
        entry_hash = %entry.entry_hash,
        "Audit event logged"
    );

    Ok(())
}
```

---

## 3. Dependency Mapping: Node.js → Rust (Updated)

### 2.1 Core Dependencies

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **commander** (14.0.0) | CLI framework | **clap** (4.4) with derive macros | Industry standard, compile-time validation, zero-cost |
| **better-sqlite3** (11.10.0) | SQLite sync binding | **rusqlite** (0.30) + **sqlx** (0.7) | `rusqlite` for sqlite-vec extension; `sqlx` for async query pool |
| **chalk** (4.1.2) | Terminal colors | **colored** (2.1) or **owo-colors** (3.5) | `owo-colors` is faster, zero-alloc; `colored` has richer API |
| **chokidar** (3.6.0) | File watching | **notify** (6.1) | Cross-platform, async-ready, production-proven |
| **fs-extra** (11.3.0) | File system utilities | **tokio::fs** + **walkdir** | `tokio::fs` for async I/O; `walkdir` for recursive directory ops |
| **minimatch** (10.0.3) | Glob pattern matching | **globset** (0.4) or **glob** (0.3) | `globset` compiles patterns for O(1) matching |
| **uuid** (13.0.0) | UUID generation | **uuid** (1.6) with `v4` feature | Feature parity, serde-compatible |
| **dotenv** (17.2.1) | Environment config | **dotenvy** (0.15) | Drop-in Rust replacement |
| **ajv** (8.17.1) | JSON Schema validation | **jsonschema** (0.17) | Standards-compliant, fast validation |
| **js-yaml** (4.1.0) | YAML parsing | **serde_yaml** (0.9) | Serde integration, zero-copy deserialization |

### 2.2 Cryptography Stack

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **jose** (5.10.0) | ED25519 signing (JWS/JWT) | **ed25519-dalek** (2.1) + **jsonwebtoken** (9.2) | `ed25519-dalek` for raw signing; `jsonwebtoken` for JWT if needed |
| **crypto** (Node.js built-in) | SHA-256 hashing | **ring** (0.17) or **sha2** (0.10) | **RECOMMENDATION: `ring`** – constant-time, audited, FIPS-friendly |
| N/A | X.509 certificate handling | **rustls** (0.21) or **openssl** (0.10) | **RECOMMENDATION: `rustls`** – pure Rust, no OpenSSL dependency for air-gap |

#### Crypto Choice Matrix

| Use Case | Library | Justification |
|----------|---------|---------------|
| **ED25519 signing/verification** | `ed25519-dalek` | Pure Rust, fast, well-audited, no unsafe code |
| **SHA-256 hashing** | `ring::digest` | Constant-time, BoringSSL-derived, battle-tested |
| **Receipt signatures** | `ed25519-dalek::Signer` | Trait-based, zero-copy, 64-byte signatures |
| **Key generation** | `ed25519-dalek::SigningKey::generate` | Cryptographically secure RNG via `rand_core` |
| **X.509 (future)** | `rustls` + `rcgen` | Air-gap friendly, no OpenSSL, pure Rust TLS |

**Decision: Avoid `openssl` crate**
- Requires OpenSSL system library (breaks air-gap single-binary goal)
- Use `ring` for hashing, `ed25519-dalek` for signing, `rustls` for TLS

### 2.3 Web Server & API

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **express/fastify** | REST API server | **axum** (0.7) | **RECOMMENDATION: Axum** – Tokio-native, composable middleware, fastest routing |
| **http** (Node.js) | HTTP server | **hyper** (1.0) | Axum is built on hyper; use directly for custom protocols |
| N/A | Prometheus metrics | **prometheus** (0.13) + **axum-prometheus** | Official Prometheus client, Axum middleware |

#### Web Framework Comparison

| Framework | Pros | Cons | Verdict |
|-----------|------|------|---------|
| **Axum** | Tokio ecosystem, type-safe extractors, compile-time routing, tower middleware | Newer (less mature than Actix) | ✅ **Recommended** |
| **Actix-web** | Mature, battle-tested, actor model | Actor overhead, moving away from actors | ❌ Unnecessary complexity |
| **Rocket** | Elegant API, compile-time validation | Requires nightly Rust (until 0.6), slower | ❌ Nightly dependency |

**Decision: Axum**
- Reason 1: Seamless Tokio integration (same runtime as SQLite async, file I/O)
- Reason 2: Tower middleware (tracing, compression, rate-limiting)
- Reason 3: Compile-time safety (extractors validate at build time)
- Reason 4: Fastest routing performance (trie-based, zero-cost)

### 2.4 Vector Search & Embeddings

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **@xenova/transformers** (2.17.2) | ONNX model inference | **tract-onnx** (0.20) or **ort** (1.16) | `tract` is pure Rust; `ort` wraps ONNX Runtime C++ |
| **onnxruntime-node** (1.22.0) | ONNX Runtime bindings | **ort** (1.16) | Official ONNX Runtime bindings |
| **sqlite-vec** (0.1.7-alpha.2) | Vector SQLite extension | **rusqlite** with manual DLL loading | Load `vec0.dll` via `rusqlite::Connection::load_extension` |
| **sqlite-vss** (0.1.2) | FAISS-backed vectors | **rusqlite** + manual FAISS binding | Consider **`faiss-rs`** or inline FAISS C++ via FFI |

#### Embedding Inference Choice

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| **tract-onnx** | Pure Rust, no C++ deps, compiles to binary | Slower than ONNX Runtime | ✅ **Recommended for air-gap** |
| **ort (ONNX Runtime)** | Fastest inference, NVIDIA/Microsoft-backed | Requires bundling ONNX Runtime DLL | Use if performance > binary size |

**Decision: `tract-onnx`**
- Reason: Air-gap requirement mandates zero external DLL dependencies
- Performance: Acceptable (384D model inference ~5-10ms on CPU)
- Future: Can swap to `ort` via feature flag if needed

### 2.5 Tree-sitter (Code Parsing)

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **tree-sitter** (0.25.0) | Parser framework | **tree-sitter** (0.20) | Official Rust implementation |
| **tree-sitter-javascript** | JS/TS grammar | **tree-sitter-javascript** (0.20) | Rust bindings |
| **tree-sitter-python** | Python grammar | **tree-sitter-python** (0.20) | Rust bindings |
| **tree-sitter-typescript** | TypeScript grammar | **tree-sitter-typescript** (0.20) | Rust bindings |

**Implementation:** Embed grammars as compiled `.so` or compile into binary via `include_bytes!`

### 2.6 Compression & Serialization

| Node.js Package | Purpose | Rust Equivalent | Rationale |
|----------------|---------|-----------------|-----------|
| **fflate** (0.8.2) | Gzip/Deflate | **flate2** (1.0) | Pure Rust, battle-tested |
| N/A | MessagePack (future) | **rmp-serde** (1.1) | 50% smaller than JSON, faster |

### 2.7 Complete Dependency Table

```toml
# Cargo.toml dependencies (consolidated)
[workspace.dependencies]
# CLI & Config
clap = { version = "4.4", features = ["derive", "env"] }
dotenvy = "0.15"
colored = "2.1"

# Async Runtime
tokio = { version = "1.35", features = ["full", "tracing"] }
tokio-util = { version = "0.7", features = ["io"] }

# Database
rusqlite = { version = "0.30", features = ["bundled", "blob", "functions"] }
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "migrate"] }

# Cryptography
ed25519-dalek = { version = "2.1", features = ["rand_core"] }
ring = "0.17"
sha2 = "0.10"
rand_core = { version = "0.6", features = ["getrandom"] }

# Web Server & Metrics
axum = { version = "0.7", features = ["tracing", "tower-log"] }
hyper = { version = "1.0", features = ["server", "http1", "http2"] }
tower = { version = "0.4", features = ["timeout", "limit"] }
tower-http = { version = "0.5", features = ["trace", "compression-gzip"] }
prometheus = "0.13"

# Embeddings & Vector Search
tract-onnx = "0.20"
ndarray = "0.15"

# Tree-sitter Code Parsing
tree-sitter = "0.20"
tree-sitter-javascript = "0.20"
tree-sitter-python = "0.20"
tree-sitter-typescript = "0.20"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Error Handling & Logging
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
globset = "0.4"
walkdir = "2.4"
notify = "6.1"
flate2 = "1.0"
```

---

## 3. Async Runtime: Tokio Configuration Strategy

### 3.1 Runtime Architecture

**Single Tokio Runtime** with partitioned thread pools:

```rust
// In securamem-cli/src/main.rs
use tokio::runtime::Builder;

fn main() -> Result<()> {
    // Create multi-threaded runtime with custom configuration
    let runtime = Builder::new_multi_thread()
        .worker_threads(4)  // 4 worker threads for I/O and API requests
        .thread_name("smem-worker")
        .enable_all()  // Enable I/O and time
        .build()?;

    runtime.block_on(async {
        // Spawn background vector indexing task
        let indexing_handle = tokio::spawn(async {
            indexing_loop().await
        });

        // Spawn API server on separate task
        let api_handle = tokio::spawn(async {
            start_api_server("127.0.0.1:9091").await
        });

        // Run CLI command (foreground)
        let result = run_cli_command().await;

        // Graceful shutdown
        indexing_handle.abort();
        api_handle.abort();

        result
    })
}
```

### 3.2 Concurrency Model

#### Background Tasks
- **Vector Indexing:** Long-running task for code indexing
- **API Server:** Axum server bound to `127.0.0.1`
- **Metrics Collector:** Periodic telemetry aggregation (every 5s)

#### Foreground Tasks
- **CLI Commands:** Async I/O for file reads, DB queries
- **HTTP Handlers:** Axum request handlers (one task per request)

### 3.3 Thread Pool Configuration

```rust
// Recommended configuration for 4-core machine
Builder::new_multi_thread()
    .worker_threads(num_cpus::get().min(8))  // Cap at 8 threads
    .max_blocking_threads(4)  // For blocking SQLite operations
    .thread_stack_size(2 * 1024 * 1024)  // 2MB stack (default is 2MB)
    .enable_all()
    .build()?
```

### 3.4 Async Database Integration

**Challenge:** SQLite is synchronous, but we need async API.

**Solution:** Use `sqlx::SqlitePool` with connection pooling:

```rust
use sqlx::sqlite::{SqlitePool, SqliteConnectOptions};

let pool = SqlitePool::connect_with(
    SqliteConnectOptions::new()
        .filename(".securamem/memory.db")
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)  // WAL for concurrency
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)  // Balance safety/speed
).await?;
```

**For sqlite-vec extension (rusqlite):**
Use `tokio::task::spawn_blocking` to move blocking operations off async threads:

```rust
use rusqlite::Connection;

let conn = Connection::open(".securamem/memory.db")?;
conn.load_extension_no_entrypoint(".securamem/sqlite-vec/vec0.dll")?;

// Wrap in spawn_blocking for async context
tokio::task::spawn_blocking(move || {
    let results = conn.query_row(
        "SELECT * FROM vec0_search(...)",
        [],
        |row| Ok(...)
    )?;
    Ok(results)
}).await??;
```

### 3.5 Graceful Shutdown

```rust
use tokio::signal;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, cleaning up...");
}

// In API server
axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .with_graceful_shutdown(shutdown_signal())
    .await?;
```

---

## 4. Error Handling: Global `SecuraMemError` Enum

### 4.1 Error Architecture

**Strategy:** Use `thiserror` for structured error types, `anyhow` for ad-hoc errors in CLI.

```rust
// In securamem-core/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecuraMemError {
    // Database Errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Database migration failed: {0}")]
    Migration(String),

    #[error("Vector backend unavailable: {backend}")]
    VectorBackendUnavailable { backend: String },

    // Cryptography Errors
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Key not found: {key_id}")]
    KeyNotFound { key_id: String },

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Receipt hash mismatch: expected {expected}, got {actual}")]
    ReceiptHashMismatch { expected: String, actual: String },

    #[error("Journal chain broken at entry {entry_id}")]
    JournalChainBroken { entry_id: String },

    // Network Errors
    #[error("HTTP server error: {0}")]
    HttpServer(#[from] hyper::Error),

    #[error("Attempted to bind to non-localhost address: {addr}")]
    NonLocalhostBind { addr: String },

    #[error("Port {port} already in use")]
    PortInUse { port: u16 },

    // I/O Errors
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid UTF-8 in file: {path}")]
    InvalidUtf8 { path: String },

    // Parsing Errors
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Tree-sitter parsing failed for {language}: {message}")]
    TreeSitterParse { language: String, message: String },

    // Vector/Embedding Errors
    #[error("Embedding model not loaded: {model}")]
    EmbeddingModelNotLoaded { model: String },

    #[error("ONNX inference failed: {0}")]
    OnnxInference(String),

    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    VectorDimensionMismatch { expected: usize, actual: usize },

    // Policy/Compliance Errors
    #[error("Policy violation: {policy} - {reason}")]
    PolicyViolation { policy: String, reason: String },

    #[error("Compliance check failed: {framework}")]
    ComplianceCheckFailed { framework: String },

    #[error("Approval required for operation: {operation}")]
    ApprovalRequired { operation: String },

    // Configuration Errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid configuration key: {key}")]
    InvalidConfigKey { key: String },

    // Generic Errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not implemented: {feature}")]
    NotImplemented { feature: String },
}

pub type Result<T> = std::result::Result<T, SecuraMemError>;
```

### 4.2 Error Conversion Chain

```rust
// Automatic conversions via #[from]
sqlx::Error → SecuraMemError::Database
rusqlite::Error → SecuraMemError::Sqlite
std::io::Error → SecuraMemError::Io
serde_json::Error → SecuraMemError::Json

// Manual context wrapping
use thiserror::Error;

impl SecuraMemError {
    pub fn context(self, msg: &str) -> Self {
        Self::Internal(format!("{}: {:?}", msg, self))
    }
}

// Usage:
let result = load_key("private.pem")
    .map_err(|e| e.context("Failed to load server signing key"))?;
```

### 4.3 Audit Logging Integration

Every error should emit a tracing event:

```rust
impl SecuraMemError {
    pub fn log(&self) {
        match self {
            Self::SignatureVerificationFailed(msg) => {
                tracing::error!(
                    error.kind = "signature_verification",
                    error.message = %msg,
                    "Cryptographic verification failed"
                );
            }
            Self::PolicyViolation { policy, reason } => {
                tracing::warn!(
                    error.kind = "policy_violation",
                    policy = %policy,
                    reason = %reason,
                    "Policy violation detected"
                );
            }
            _ => {
                tracing::error!(
                    error.kind = "generic",
                    error = %self,
                    "Error occurred"
                );
            }
        }
    }
}
```

---

## 5. Air-Gap Enforcement: Strict Localhost Binding

### 5.1 Compile-Time Safety

**Goal:** Prevent any code from binding to `0.0.0.0` or external interfaces.

#### Type-Safe Address Binding

```rust
// In securamem-core/src/network.rs
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;

/// A socket address that is guaranteed to be localhost-only
#[derive(Debug, Clone, Copy)]
pub struct LocalhostAddr {
    port: u16,
}

impl LocalhostAddr {
    /// Create a new localhost address with the given port
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Convert to a SocketAddr (always 127.0.0.1)
    pub fn to_socket_addr(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.port)
    }
}

/// Attempt to create a LocalhostAddr from a SocketAddr, returning an error if not localhost
impl TryFrom<SocketAddr> for LocalhostAddr {
    type Error = SecuraMemError;

    fn try_from(addr: SocketAddr) -> Result<Self, Self::Error> {
        if addr.ip().is_loopback() {
            Ok(LocalhostAddr { port: addr.port() })
        } else {
            Err(SecuraMemError::NonLocalhostBind {
                addr: addr.to_string(),
            })
        }
    }
}

/// Parse a string like "9091" or "127.0.0.1:9091", rejecting external IPs
impl std::str::FromStr for LocalhostAddr {
    type Err = SecuraMemError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // If just a port number
        if let Ok(port) = s.parse::<u16>() {
            return Ok(LocalhostAddr::new(port));
        }

        // If a socket address
        if let Ok(addr) = s.parse::<SocketAddr>() {
            return addr.try_into();
        }

        Err(SecuraMemError::Config(format!(
            "Invalid localhost address: {}",
            s
        )))
    }
}
```

### 5.2 API Server Enforcement

```rust
// In securamem-l3/src/prometheus.rs
use axum::Router;
use securamem_core::LocalhostAddr;

pub async fn start_prometheus_server(
    addr: LocalhostAddr,
    collector: TelemetryCollector,
) -> Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .layer(Extension(collector));

    // Compile-time guarantee: addr.to_socket_addr() is always 127.0.0.1
    let listener = tokio::net::TcpListener::bind(addr.to_socket_addr()).await?;

    tracing::info!("Prometheus exporter listening on {}", addr.to_socket_addr());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
```

### 5.3 Additional Safeguards

#### Network Egress Blocking (at compile-time)

```rust
// In securamem-core/src/lib.rs
#[cfg(not(feature = "allow-network-egress"))]
compile_error!(
    "SecuraMem is configured for air-gapped deployment. \
     Network egress is disabled. If you need to enable it for testing, \
     compile with --features allow-network-egress"
);
```

#### Runtime Assertion

```rust
// In securamem-cli/src/main.rs
fn main() -> Result<()> {
    // Assert at startup
    assert!(
        !cfg!(feature = "allow-network-egress"),
        "Network egress must be disabled for production builds"
    );

    // ... rest of main
}
```

### 5.4 Static Analysis Integration

Use `cargo-deny` to enforce no network dependencies:

```toml
# deny.toml
[bans]
deny = [
    { name = "reqwest" },      # HTTP client
    { name = "hyper-tls" },    # TLS (use rustls instead)
    { name = "curl" },         # cURL bindings
    { name = "ureq" },         # Lightweight HTTP
]
```

---

## 6. CLI Architecture: Implementation with Clap

### 6.1 Command Structure

```rust
// In securamem-cli/src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "smem")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "SecuraMem - Secure, persistent memory for AI coding assistants")]
#[command(long_about = None)]
struct Cli {
    /// Print plan and side-effects (no hidden work)
    #[arg(long, global = true)]
    trace: bool,

    /// Simulate without side effects
    #[arg(long, global = true)]
    dry_run: bool,

    /// Emit machine-readable receipts
    #[arg(long, global = true)]
    json: bool,

    /// Explain what and why before running
    #[arg(long, global = true)]
    explain: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Store unlimited memories locally
    Remember {
        /// Content to remember
        content: String,

        /// Context for the memory
        #[arg(short, long, default_value = "general")]
        context: String,

        /// Type of memory
        #[arg(short = 't', long, default_value = "general")]
        memory_type: String,
    },

    /// Search unlimited local memories
    Recall {
        /// Search query
        query: String,

        /// Maximum results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Show unlimited local-only status
    Status,

    /// Initialize SecuraMem in current project
    Init {
        /// Force reinitialize if already exists
        #[arg(long)]
        force: bool,
    },

    /// Index code files into SecuraMem
    IndexCode {
        /// Root directory to index
        #[arg(long)]
        path: Option<String>,

        /// Max lines per chunk
        #[arg(long, default_value = "200")]
        max_chunk: usize,

        /// Include patterns (supports ** and *)
        #[arg(long)]
        include: Vec<String>,

        /// Exclude patterns
        #[arg(long)]
        exclude: Vec<String>,

        /// Use symbol-aware chunking
        #[arg(long)]
        symbols: bool,

        /// Skip unchanged files (digest-based)
        #[arg(long)]
        diff: bool,
    },

    /// Cryptographically verify audit trail integrity
    Verify {
        /// Verify specific receipt by ID or path
        #[arg(long)]
        receipt: Option<String>,

        /// Verify all receipts
        #[arg(long)]
        all: bool,

        /// Verify entire audit chain
        #[arg(long)]
        chain: bool,

        /// Show detailed verification information
        #[arg(long)]
        verbose: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        output: OutputFormat,

        /// Verify DB journal hash chain
        #[arg(long)]
        db_journal: bool,
    },

    /// Start Prometheus metrics exporter
    Prometheus {
        /// Port for Prometheus exporter
        #[arg(long, default_value = "9091")]
        port: u16,
    },

    /// Rotate cryptographic signing keys
    KeyRotate {
        /// Reason for key rotation
        #[arg(long)]
        reason: Option<String>,

        /// Create backup of current key
        #[arg(long, default_value = "true")]
        backup: bool,

        /// Force rotation even if current key is new
        #[arg(long)]
        force: bool,
    },

    // ... (remaining 50+ commands)
}

#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Json,
    Text,
}
```

### 6.2 Command Dispatch

```rust
// In securamem-cli/src/main.rs
use clap::Parser;
use securamem_l1::ReceiptService;
use securamem_l2::MemoryEngine;
use securamem_l3::PrometheusExporter;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    // Initialize services
    let service_container = ServiceContainer::new(std::env::current_dir()?).await?;

    // Dispatch command
    match cli.command {
        Commands::Remember { content, context, memory_type } => {
            commands::remember::execute(
                &service_container,
                content,
                context,
                memory_type,
            ).await?;
        }

        Commands::Recall { query, limit } => {
            commands::recall::execute(
                &service_container,
                query,
                limit,
            ).await?;
        }

        Commands::Verify { receipt, all, chain, verbose, output, db_journal } => {
            commands::verify::execute(
                &service_container,
                receipt,
                all,
                chain,
                verbose,
                output,
                db_journal,
            ).await?;
        }

        Commands::Prometheus { port } => {
            let addr = LocalhostAddr::new(port);
            commands::prometheus::start_server(
                &service_container,
                addr,
            ).await?;
        }

        // ... other commands
    }

    Ok(())
}
```

### 6.3 Service Container Pattern

```rust
// In securamem-core/src/container.rs
use std::sync::Arc;
use securamem_db::SqliteDatabase;
use securamem_l1::{ReceiptService, JournalService, ComplianceService};
use securamem_l2::{MemoryEngine, VectorEmbeddings, HybridSearchEngine};
use securamem_l3::{TelemetryCollector, PrometheusExporter};

pub struct ServiceContainer {
    project_path: PathBuf,
    db: Arc<SqliteDatabase>,
    receipt_service: Arc<ReceiptService>,
    memory_engine: Arc<MemoryEngine>,
    telemetry: Arc<TelemetryCollector>,
}

impl ServiceContainer {
    pub async fn new(project_path: PathBuf) -> Result<Self> {
        // Initialize database
        let db_path = project_path.join(".securamem").join("memory.db");
        let db = Arc::new(SqliteDatabase::open(db_path).await?);

        // Initialize L1 services
        let receipt_service = Arc::new(ReceiptService::new(db.clone())?);
        let journal_service = Arc::new(JournalService::new(db.clone())?);
        let compliance_service = Arc::new(ComplianceService::new(db.clone())?);

        // Initialize L2 services
        let embeddings = Arc::new(VectorEmbeddings::load_model().await?);
        let search_engine = Arc::new(HybridSearchEngine::new(db.clone(), embeddings.clone())?);
        let memory_engine = Arc::new(MemoryEngine::new(db.clone(), search_engine)?);

        // Initialize L3 services
        let telemetry = Arc::new(TelemetryCollector::new(db.clone()));

        Ok(Self {
            project_path,
            db,
            receipt_service,
            memory_engine,
            telemetry,
        })
    }

    pub fn receipt_service(&self) -> &ReceiptService {
        &self.receipt_service
    }

    pub fn memory_engine(&self) -> &MemoryEngine {
        &self.memory_engine
    }

    pub fn telemetry(&self) -> &TelemetryCollector {
        &self.telemetry
    }
}
```

---

## 7. Performance Targets

### 7.1 Benchmarks (Node.js → Rust)

| Operation | Node.js (Current) | Rust (Target) | Improvement |
|-----------|-------------------|---------------|-------------|
| **Vector search (384D, 10K vectors)** | 50-100ms | <10ms | 5-10x |
| **Receipt generation + signing** | 5-8ms | <1ms | 5-8x |
| **SQLite write (1KB memory)** | 2-3ms | <0.5ms | 4-6x |
| **Code indexing (10K LOC)** | 15-20s | <5s | 3-4x |
| **Journal hash chain verification (1K entries)** | 500ms | <50ms | 10x |
| **Binary startup (cold)** | 300-500ms | <50ms | 6-10x |
| **Memory footprint (idle)** | 80-120MB | <20MB | 4-6x |
| **Binary size** | 200MB (node_modules) | <50MB | 4x |

### 7.2 Optimization Strategies

#### Database
- Use WAL mode for concurrent reads/writes
- Prepared statement caching (already done in Node.js, preserve in Rust)
- Batch inserts with transactions

#### Vector Search
- Use `tract` with graph optimization
- Precompute embeddings at index time
- Cache query embeddings (TTL: 1 hour)

#### API Server
- Tower middleware for compression (gzip)
- Connection pooling (already in Tokio)
- Zero-copy serialization (use `bytes::Bytes`)

---

## 8. Migration Strategy

### 8.1 Phased Approach

#### Phase 1: Core Infrastructure (Weeks 1-2)
- [ ] Set up Cargo workspace
- [ ] Implement `securamem-core` (error types, config)
- [ ] Implement `securamem-crypto` (ED25519, SHA-256)
- [ ] Implement `securamem-db` (SQLite, migrations)
- [ ] Write integration tests

#### Phase 2: L1 Compliance Layer (Weeks 3-4)
- [ ] Port `ReceiptService` to Rust
- [ ] Port `JournalService` (hash chain)
- [ ] Port `ComplianceService` (GDPR, EU AI Act checks)
- [ ] Verify parity with Node.js receipts (cross-verify signatures)

#### Phase 3: L2 Memory Layer (Weeks 5-6)
- [ ] Port `VectorEmbeddings` (ONNX model loading)
- [ ] Port `HybridSearchEngine` (BM25 + vector)
- [ ] Port `IndexingService` (tree-sitter integration)
- [ ] Benchmark vector search (<10ms target)

#### Phase 4: L3 Monitoring Layer (Week 7)
- [ ] Port `PrometheusExporter` (Axum server)
- [ ] Port `TelemetryCollector`
- [ ] Implement `/metrics`, `/health`, `/dashboard` endpoints

#### Phase 5: CLI & Commands (Week 8)
- [ ] Implement clap CLI structure
- [ ] Port all 50+ commands
- [ ] End-to-end testing

#### Phase 6: Optimization & Packaging (Week 9-10)
- [ ] Profile with `cargo flamegraph`
- [ ] Optimize hot paths
- [ ] Create release builds with LTO and stripping
- [ ] Cross-compile for Windows/Linux/macOS
- [ ] Documentation and deployment guides

### 8.2 Testing Strategy

#### Unit Tests
- Every public function in `securamem-l1`, `securamem-l2`, `securamem-l3`
- Property-based testing for crypto (use `proptest`)

#### Integration Tests
- Cross-verify receipts with Node.js implementation
- Test entire CLI workflows (e.g., `init → index-code → search-code → verify`)

#### Performance Tests
- Criterion.rs benchmarks for vector search, crypto ops
- Compare with Node.js baseline

---

## 9. Build Configuration

### 9.1 Release Profile

```toml
# Cargo.toml (root)
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

### 9.2 Cross-Compilation Targets

```bash
# Linux (x86_64)
cargo build --release --target x86_64-unknown-linux-musl

# macOS (Apple Silicon)
cargo build --release --target aarch64-apple-darwin

# Windows (x86_64)
cargo build --release --target x86_64-pc-windows-msvc
```

### 9.3 Binary Size Optimization

- Use `cargo-bloat` to identify large dependencies
- Use `wasm-opt` techniques (even for native binaries)
- Embed assets with `include_bytes!` and compress with gzip

Target: **<50MB** single binary (vs. current 200MB with node_modules)

---

## 10. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| **Crypto implementation bugs** | Use audited libraries (`ring`, `ed25519-dalek`); cross-verify with Node.js |
| **Performance regressions** | Benchmark continuously; compare with Node.js baseline |
| **Async/await complexity** | Use `#[tokio::test]` for all async code; lint with Clippy |
| **SQLite extension loading (sqlite-vec)** | Test on all platforms; fallback to pure Rust vector index if needed |
| **Tree-sitter grammar compatibility** | Pin grammar versions; test against Node.js parse trees |
| **Binary compatibility (cross-platform)** | CI/CD matrix testing (Linux, macOS, Windows) |

---

## 11. Success Criteria

- [ ] All 50+ CLI commands ported and functional
- [ ] Receipts cross-verify with Node.js implementation (ED25519 signatures)
- [ ] Vector search <10ms (vs. current 50-100ms)
- [ ] Single binary <50MB (vs. current 200MB)
- [ ] Zero runtime dependencies (air-gapped deployment verified)
- [ ] 100% localhost binding enforcement (compile-time + runtime checks)
- [ ] Full test coverage (>80% line coverage)
- [ ] Documentation complete (architecture, API, deployment)

---

## Appendix A: Key File Mappings

| Node.js File | Rust Crate | Rust File |
|--------------|-----------|-----------|
| `src/index.ts` | `securamem-cli` | `src/main.rs` |
| `src/services/ReceiptService.ts` | `securamem-l1` | `src/receipt.rs` |
| `src/utils/cryptoHelpers.ts` | `securamem-crypto` | `src/ed25519.rs` |
| `src/engine/VectorEmbeddings.ts` | `securamem-l2` | `src/embeddings.rs` |
| `src/engine/HybridSearchEngine.ts` | `securamem-l2` | `src/search.rs` |
| `src/server/PrometheusExporter.ts` | `securamem-l3` | `src/prometheus.rs` |
| `src/database/MemoryDatabase.ts` | `securamem-db` | `src/sqlite.rs` |

---

## Appendix B: Configuration File (Rust)

```toml
# smem.config.toml
[server]
prometheus_port = 9091
bind_address = "127.0.0.1"  # Enforced, cannot be changed to 0.0.0.0

[database]
path = ".securamem/memory.db"
journal_mode = "wal"
cache_size = 10000  # Pages (10MB)

[vector]
backend = "sqlite-vec"  # Options: sqlite-vec, faiss, local
dimensions = 384
model = "e5-small-v2"
model_path = ".securamem/models/e5-small-v2.onnx"

[compliance]
frameworks = ["gdpr", "eu_ai_act", "nist_rmf"]
retention_days = 2555  # 7 years

[logging]
level = "info"
format = "json"
output = ".securamem/logs/securamem.log"
```

---

## Appendix C: Example Rust Receipt Structure

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub receipt_id: String,
    pub timestamp: DateTime<Utc>,
    pub schema_version: String,
    pub actor: Actor,
    pub context: ReceiptContext,
    pub output: ReceiptOutput,
    pub meta: ReceiptMeta,
    #[serde(skip_serializing)]
    pub server_signature: String,
    #[serde(skip_serializing)]
    pub server_key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Actor {
    pub user_id: String,
    pub username: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiptContext {
    pub operation_type: String,
    pub command: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiptOutput {
    pub success: bool,
    pub result_summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiptMeta {
    pub requires_approval: bool,
    pub approved: bool,
}
```

---

**End of Document**

This architecture document provides a complete blueprint for refactoring SecuraMem to Rust. Implementation should follow the phased approach, with continuous benchmarking against the Node.js baseline to ensure performance targets are met.
