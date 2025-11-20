# SecuraMem Rust Refactoring Summary (Audit-Only Scope)

**Version:** 2.0 - AI Black Box Recorder
**Date:** 2025-01-18
**Strategic Pivot:** Removed all vector/AI memory capabilities - **Pure audit & compliance ledger**

---

## Executive Summary

SecuraMem is now a **high-performance AI Black Box Recorder** - an immutable audit ledger with cryptographic integrity guarantees. This document summarizes the refactored Cargo workspace and database schema for the audit-only scope.

**What Changed:**
- ❌ **Removed:** All vector search (sqlite-vec, rusqlite, tract-onnx, tree-sitter)
- ❌ **Removed:** AI inference (ONNX, transformers, embeddings)
- ✅ **Added:** RFC 3161 timestamping for legal-grade non-repudiation
- ✅ **Added:** Hash-chain immutable ledger (blockchain-style audit log)
- ✅ **Simplified:** SQLx-only database layer (no rusqlite)

---

## 1. Refactored Cargo Workspace

### 1.1 Workspace Structure

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/securamem-cli",       # Binary crate (entry point)
    "crates/securamem-l1",         # L1: Compliance & Audit
    "crates/securamem-storage",    # Storage: Immutable hash-chain ledger (renamed from L2)
    "crates/securamem-l3",         # L3: Monitoring & API
    "crates/securamem-core",       # Core: Shared primitives
    "crates/securamem-crypto",     # Crypto: ED25519, SHA-256, RFC 3161
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

# === Database (SQLx ONLY - No rusqlite) ===
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
ring = "0.17"      # SHA-256 hashing (constant-time, BoringSSL-derived)
sha2 = "0.10"      # Fallback SHA-256 (pure Rust)
rand_core = { version = "0.6", features = ["getrandom"] }

# === RFC 3161 Timestamping ===
x509-parser = "0.16"    # Parse X.509 certificates in TSP responses
asn1-rs = "0.6"          # ASN.1 DER encoding/decoding
der = "0.7"              # DER encoding (for TimeStampReq)
reqwest = { version = "0.11", features = ["blocking", "rustls-tls"], optional = true }  # TSA HTTP client (air-gap optional)

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
futures = "0.3"               # For async stream processing

[features]
default = []
tsa-client = ["reqwest"]      # Enable RFC 3161 TSA HTTP client (disable for air-gap)
```

### 1.2 Crate Roles

| Crate | Role | Key Exports |
|-------|------|-------------|
| **securamem-cli** | Binary entry point | CLI command dispatcher, Tokio runtime setup |
| **securamem-core** | Shared primitives | `SecuraMemError`, `Result<T>`, config structs, `LocalhostAddr` |
| **securamem-crypto** | Cryptography | `SigningKey`, `Receipt`, `HashChain`, `Rfc3161Client`, SHA-256 utils |
| **securamem-storage** | Hash-chain ledger | `HashChainStore`, `AuditEntry`, `QueryBuilder`, SQLx migrations |
| **securamem-l1** | Compliance orchestration | `AuditOrchestrator`, `ReceiptService`, `ComplianceChecker`, `PolicyEngine` |
| **securamem-l3** | Monitoring & API | `PrometheusExporter`, `TelemetryCollector`, `ApiServer`, health checks |

### 1.3 Dependency Graph

```
securamem-cli (bin)
    ├── securamem-l1 (Compliance Orchestration)
    │   ├── securamem-crypto (ED25519, SHA-256, RFC 3161)
    │   │   └── securamem-core
    │   ├── securamem-storage (Hash-Chain Ledger)
    │   │   ├── securamem-crypto
    │   │   │   └── securamem-core
    │   │   └── securamem-core
    │   └── securamem-core
    ├── securamem-storage
    │   ├── securamem-crypto
    │   │   └── securamem-core
    │   └── securamem-core
    ├── securamem-l3 (Monitoring & API)
    │   ├── securamem-l1
    │   ├── securamem-storage
    │   └── securamem-core
    └── securamem-core
```

**Principles:**
- ✅ Acyclic (enforced by Cargo)
- ✅ Layered (L3 depends on L1/storage, not vice versa)
- ✅ Core-first (all crates depend on core, core depends on nothing internal)

---

## 2. Database Schema (SQLx Migration)

### 2.1 Core Audit Log Table

```sql
-- migrations/001_audit_log_schema.sql
-- SecuraMem Immutable Audit Log Schema v2.0

CREATE TABLE IF NOT EXISTS audit_log (
    -- Primary Key
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,

    -- Receipt Identifier (UUID v4)
    receipt_id TEXT UNIQUE NOT NULL,

    -- Timestamp (ISO 8601 UTC)
    timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),

    -- Actor/Principal
    actor_user_id TEXT NOT NULL,
    actor_username TEXT,
    actor_role TEXT,

    -- Operation Context
    operation_type TEXT NOT NULL,  -- 'cli_command', 'api_call', 'recall', etc.
    command TEXT,                   -- 'smem verify', 'smem export-audit', etc.

    -- Audit Data (JSON blob for flexibility)
    audit_data TEXT NOT NULL,  -- JSON: { parameters, input, output, metadata }

    -- Hash Chain (Immutability Enforcement)
    prev_hash TEXT,            -- SHA-256 of previous entry (NULL for genesis)
    entry_hash TEXT NOT NULL,  -- SHA-256 of (prev_hash || entry_data)

    -- Cryptographic Signature (ED25519)
    signature TEXT NOT NULL,         -- Base64-encoded ED25519 signature
    signature_key_id TEXT NOT NULL,  -- Key fingerprint

    -- RFC 3161 Timestamp (Optional)
    rfc3161_timestamp_token BLOB,   -- DER-encoded TimeStampToken
    rfc3161_tsa_url TEXT,            -- TSA URL
    rfc3161_verified BOOLEAN,        -- Whether TST was verified

    -- Compliance Metadata
    retention_until TEXT,       -- ISO 8601 date (GDPR retention)
    sensitivity_level TEXT,     -- 'public', 'internal', 'confidential', 'restricted'
    compliance_flags TEXT,      -- JSON: { gdpr_compliant, eu_ai_act_risk_level }

    -- Constraints
    CONSTRAINT unique_receipt UNIQUE (receipt_id),
    CONSTRAINT valid_prev_hash CHECK (prev_hash IS NULL OR length(prev_hash) = 64),
    CONSTRAINT valid_entry_hash CHECK (length(entry_hash) = 64)
);

-- Indexes for Query Performance
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp DESC);
CREATE INDEX idx_audit_log_actor ON audit_log(actor_user_id, timestamp DESC);
CREATE INDEX idx_audit_log_operation ON audit_log(operation_type, timestamp DESC);
CREATE INDEX idx_audit_log_receipt_id ON audit_log(receipt_id);
CREATE INDEX idx_audit_log_entry_hash ON audit_log(entry_hash);
```

### 2.2 Supporting Tables

```sql
-- Cryptographic Keys Table
CREATE TABLE signing_keys (
    key_id TEXT PRIMARY KEY NOT NULL,
    key_fingerprint TEXT UNIQUE NOT NULL,  -- SHA-256 of public key
    public_key_pem TEXT NOT NULL,
    private_key_encrypted BLOB,            -- Encrypted private key (backup)
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    rotated_at TEXT,
    status TEXT NOT NULL DEFAULT 'active',  -- 'active', 'rotated', 'revoked'

    CONSTRAINT valid_status CHECK (status IN ('active', 'rotated', 'revoked'))
);

-- Audit Chain Checkpoints (for fast verification)
CREATE TABLE audit_checkpoints (
    checkpoint_id INTEGER PRIMARY KEY AUTOINCREMENT,
    checkpoint_timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    last_entry_id INTEGER NOT NULL,
    last_entry_hash TEXT NOT NULL,
    entry_count INTEGER NOT NULL,
    checkpoint_signature TEXT NOT NULL,  -- Signature over (last_entry_hash || entry_count)

    FOREIGN KEY (last_entry_id) REFERENCES audit_log(id)
);

-- Compliance Policy Rules
CREATE TABLE compliance_policies (
    policy_id TEXT PRIMARY KEY NOT NULL,
    policy_name TEXT NOT NULL,
    policy_type TEXT NOT NULL,  -- 'gdpr', 'eu_ai_act', 'nist_rmf', 'custom'
    policy_rules TEXT NOT NULL,  -- JSON policy definition
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'utc'))
);

-- Telemetry Metrics (for Prometheus)
CREATE TABLE telemetry_metrics (
    metric_id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_labels TEXT  -- JSON: { label_key: label_value }
);

CREATE INDEX idx_telemetry_timestamp ON telemetry_metrics(metric_timestamp DESC);
```

---

## 3. Hash-Chain Implementation (Rust)

### 3.1 Core Logic

```rust
// In crates/securamem-storage/src/hash_chain.rs

use ring::digest::{digest, SHA256};
use sqlx::{SqlitePool, Row};
use securamem_core::{Result, SecuraMemError};

pub struct HashChainStore {
    pool: SqlitePool,
}

impl HashChainStore {
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
        // 1. Get previous entry hash
        let prev_hash = self.get_last_entry_hash().await?;

        // 2. Compute canonical representation
        let canonical_data = serde_json::json!({
            "receipt_id": receipt_id,
            "actor_user_id": actor_user_id,
            "operation_type": operation_type,
            "audit_data": audit_data,
        });
        let canonical_bytes = serde_json::to_vec(&canonical_data)?;

        // 3. Compute entry hash = SHA256(prev_hash || canonical_data)
        let entry_hash = self.compute_hash_chain_link(prev_hash.as_deref(), &canonical_bytes)?;

        // 4. Insert into database
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
        .await?;

        Ok(AuditEntry { receipt_id, entry_hash, prev_hash, /* ... */ })
    }

    /// Compute hash chain link: SHA256(prev_hash || current_data)
    fn compute_hash_chain_link(
        &self,
        prev_hash: Option<&str>,
        current_data: &[u8],
    ) -> Result<String> {
        let mut input = Vec::new();

        if let Some(prev) = prev_hash {
            input.extend_from_slice(prev.as_bytes());
        }

        input.extend_from_slice(current_data);

        let hash_bytes = digest(&SHA256, &input);
        Ok(hex::encode(hash_bytes.as_ref()))
    }

    /// Verify entire hash chain integrity
    pub async fn verify_chain(&self) -> Result<bool> {
        let mut expected_prev_hash: Option<String> = None;
        let mut rows = sqlx::query("SELECT * FROM audit_log ORDER BY id ASC")
            .fetch(&self.pool);

        while let Some(row) = rows.next().await {
            let row = row?;
            let prev_hash: Option<String> = row.get("prev_hash");
            let entry_hash: String = row.get("entry_hash");

            // Verify prev_hash matches expected
            if prev_hash != expected_prev_hash {
                return Ok(false);  // Chain broken
            }

            // Recompute and verify entry_hash
            // (omitted for brevity - see full implementation in RUST_ARCHITECTURE.md)

            expected_prev_hash = Some(entry_hash);
        }

        Ok(true)
    }
}
```

### 3.2 Usage Example

```rust
// In crates/securamem-l1/src/audit_orchestrator.rs

pub async fn log_audit_event(
    store: &HashChainStore,
    signing_key: &SigningKey,
    actor: &str,
    operation: &str,
    data: serde_json::Value,
) -> Result<()> {
    let receipt_id = format!("r_{}", uuid::Uuid::new_v4());

    // Sign the audit data
    let canonical_data = serde_json::to_vec(&data)?;
    let signature = signing_key.sign(&canonical_data);

    // Append to hash chain
    store.append_entry(
        receipt_id,
        actor.to_string(),
        operation.to_string(),
        data,
        base64::encode(signature.as_ref()),
        signing_key.key_id(),
    ).await?;

    Ok(())
}
```

---

## 4. Removed Dependencies

### 4.1 Vector/AI Components (All Removed)

| Removed Package | Reason |
|----------------|--------|
| ❌ `@xenova/transformers` | No vector search needed |
| ❌ `onnxruntime-node` | No AI inference needed |
| ❌ `sqlite-vec` | No vector index needed |
| ❌ `sqlite-vss` | No FAISS backend needed |
| ❌ `tract-onnx` | No ONNX models needed |
| ❌ `ndarray` | No linear algebra needed |
| ❌ `tree-sitter` (all grammars) | No code parsing needed |
| ❌ `rusqlite` | Replaced by sqlx-only |
| ❌ `chokidar` | File watching not needed for audit |
| ❌ `minimatch` | Glob matching not needed for audit |

### 4.2 Binary Size Impact

| Metric | Before (with AI/Vector) | After (Audit-Only) | Reduction |
|--------|------------------------|-------------------|-----------|
| **node_modules** | ~200MB | N/A (Rust) | - |
| **Binary size** | ~50MB target | **~20-30MB** | 40-60% |
| **Dependencies** | ~150MB | **~50MB** | 67% |
| **Runtime memory** | 80-120MB | **<30MB** | 70% |

---

## 5. RFC 3161 Time-Stamp Protocol

### 5.1 Overview

**RFC 3161** provides legal-grade non-repudiation via trusted timestamps from a Time-Stamp Authority (TSA).

**Workflow:**
1. Hash the audit entry (SHA-256)
2. Create TimeStampReq (ASN.1 DER encoded)
3. Submit to TSA via HTTP POST (if `tsa-client` feature enabled)
4. Receive TimeStampResp containing TimeStampToken
5. Verify TSA signature using X.509 certificate chain
6. Store TimeStampToken in `audit_log.rfc3161_timestamp_token` (BLOB)

### 5.2 Implementation (Rust)

```rust
// In crates/securamem-crypto/src/rfc3161.rs

use der::{Encode, Decode};
use x509_parser::prelude::*;
use asn1_rs::*;

pub struct Rfc3161Client {
    tsa_url: String,
}

impl Rfc3161Client {
    #[cfg(feature = "tsa-client")]
    pub fn request_timestamp(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 1. Hash the data
        let digest = ring::digest::digest(&ring::digest::SHA256, data);

        // 2. Create TimeStampReq
        let tsr_der = self.create_timestamp_request(digest.as_ref())?;

        // 3. Submit to TSA
        let response = reqwest::blocking::Client::new()
            .post(&self.tsa_url)
            .header("Content-Type", "application/timestamp-query")
            .body(tsr_der)
            .send()?;

        // 4. Parse TimeStampToken
        let tst_der = response.bytes()?.to_vec();
        self.verify_timestamp_response(&tst_der)?;

        Ok(tst_der)
    }

    fn create_timestamp_request(&self, digest: &[u8]) -> Result<Vec<u8>> {
        // ASN.1 DER encoding of TimeStampReq
        // (implementation using `der` crate)
        todo!("Encode TimeStampReq")
    }

    fn verify_timestamp_response(&self, tst_der: &[u8]) -> Result<()> {
        // Parse and verify TimeStampToken using x509-parser
        // (implementation using `x509-parser` crate)
        todo!("Verify TSA signature")
    }
}
```

### 5.3 Air-Gap Mode

When compiled without `--features tsa-client`:
- ❌ `reqwest` is excluded (zero network dependencies)
- ✅ Offline RFC 3161 verification still works (parse existing TSTs)
- ✅ Genesis timestamps can be embedded manually

---

## 6. Performance Targets (Audit-Only)

| Operation | Node.js (Current) | Rust (Target) | Improvement |
|-----------|-------------------|---------------|-------------|
| **Receipt generation + signing** | 5-8ms | **<0.5ms** | 10-16x |
| **SQLite write (1KB entry)** | 2-3ms | **<0.3ms** | 6-10x |
| **Hash chain verification (1K entries)** | 500ms | **<20ms** | 25x |
| **Binary startup (cold)** | 300-500ms | **<30ms** | 10-17x |
| **Memory footprint (idle)** | 80-120MB | **<20MB** | 4-6x |
| **Binary size** | 200MB (node_modules) | **<30MB** | 7x |

---

## 7. Migration Checklist

- [ ] Set up Cargo workspace with 6 crates
- [ ] Implement `securamem-core` (error types, config)
- [ ] Implement `securamem-crypto` (ED25519, SHA-256, RFC 3161)
- [ ] Implement `securamem-storage` (hash-chain SQLx implementation)
- [ ] Implement `securamem-l1` (audit orchestrator, compliance checker)
- [ ] Implement `securamem-l3` (Prometheus exporter, API server)
- [ ] Implement `securamem-cli` (clap CLI, command dispatcher)
- [ ] Write SQLx migrations (`migrations/001_audit_log_schema.sql`)
- [ ] Cross-verify receipts with Node.js implementation
- [ ] Benchmark hash-chain verification (<20ms target)
- [ ] Create release builds (LTO, stripped)
- [ ] Documentation (API docs, deployment guide)

---

## 8. Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| **SQLx-only (no rusqlite)** | No sqlite-vec needed; async-native; compile-time query validation |
| **Remove all vector/AI deps** | Pure audit focus; 67% dependency reduction; simpler codebase |
| **Add RFC 3161 timestamping** | Legal-grade non-repudiation; court-admissible evidence |
| **Hash-chain ledger** | Blockchain-style immutability; tamper-evident; verifiable offline |
| **ring for SHA-256** | Constant-time; FIPS 140-2; BoringSSL-derived; audited |
| **ed25519-dalek for signing** | Pure Rust; trait-based; well-audited; no unsafe code |
| **Axum for API** | Tokio-native; fastest routing; tower middleware; type-safe |
| **Feature-gated TSA client** | Air-gap mode: no network; Online mode: RFC 3161 support |

---

## 9. Success Criteria

- ✅ All CLI commands ported and functional (audit-focused subset)
- ✅ Receipts cross-verify with Node.js implementation (ED25519 signatures)
- ✅ Hash-chain verification <20ms (vs. current 500ms)
- ✅ Single binary <30MB (vs. current 200MB)
- ✅ Zero runtime dependencies (air-gapped deployment verified)
- ✅ 100% localhost binding enforcement (compile-time + runtime checks)
- ✅ RFC 3161 timestamping integrated (feature-gated for air-gap)
- ✅ Full test coverage (>80% line coverage)
- ✅ Documentation complete (architecture, API, deployment)

---

**End of Refactored Summary**

For full implementation details, see the complete [RUST_ARCHITECTURE.md](RUST_ARCHITECTURE.md) document.
