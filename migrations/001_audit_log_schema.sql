-- migrations/001_audit_log_schema.sql

-- The Immutable Audit Log Table
-- Note: PRAGMA statements are set in Rust via SqliteConnectOptions
CREATE TABLE IF NOT EXISTS audit_log (
    -- Primary Key (Auto-incrementing Sequence)
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,

    -- Unique Receipt ID (UUID v4)
    receipt_id TEXT UNIQUE NOT NULL,

    -- ISO 8601 Timestamp (UTC)
    timestamp TEXT NOT NULL DEFAULT (datetime('now', 'utc')),

    -- Actor Attribution
    actor_user_id TEXT NOT NULL,
    actor_role TEXT,

    -- Operation Context
    operation_type TEXT NOT NULL,  -- e.g. 'cli', 'api'
    command TEXT,

    -- The Payload (JSON)
    audit_data TEXT NOT NULL,

    -- The Hash Chain
    prev_hash TEXT,            -- Nullable only for Genesis
    entry_hash TEXT NOT NULL,  -- SHA-256(prev_hash + data)

    -- The Cryptographic Proof
    signature TEXT NOT NULL,         -- ED25519 Signature
    signature_key_id TEXT NOT NULL,  -- Key Fingerprint

    -- Compliance Metadata
    retention_until TEXT,
    compliance_flags TEXT,

    -- Integrity Constraints
    CONSTRAINT entry_hash_len CHECK (length(entry_hash) = 64),
    CONSTRAINT prev_hash_len CHECK (prev_hash IS NULL OR length(prev_hash) = 64)
);

-- 3. Indexes for fast Lookups
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_receipt ON audit_log(receipt_id);
CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_log(actor_user_id);

-- 4. The Genesis Entry (Bootstraps the Hash Chain)
-- This ensures the first real log always has a 'prev_hash' to point to.
INSERT OR IGNORE INTO audit_log (
    receipt_id,
    timestamp,
    actor_user_id,
    actor_role,
    operation_type,
    command,
    audit_data,
    prev_hash,
    entry_hash,
    signature,
    signature_key_id
) VALUES (
    '00000000-0000-0000-0000-000000000000',
    '1970-01-01T00:00:00Z',
    'system',
    'root',
    'genesis',
    'init',
    '{"event":"SecuraMem Log Initialization"}',
    NULL,
    '0000000000000000000000000000000000000000000000000000000000000000',
    'GENESIS_SIGNATURE_PLACEHOLDER',
    'system:genesis'
);
