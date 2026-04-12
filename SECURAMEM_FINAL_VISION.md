# SecuraMem Rust — Vision & Roadmap

> **Project:** SecuraMem Rust Core v2.0  
> **Author:** Personal project for AIGP Certification showcase  
> **Vision:** A deterministic sidecar guardrail that turns any black-box LLM into an auditable substrate  
> **Status:** Personal-use proof of concept (not production/enterprise)  
> **Last Updated:** 2026-04-12

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [The Problem SecuraMem Solves](#2-the-problem-securamem-solves)
3. [The Mythos Problem & Our Answer](#3-the-mythos-problem--our-answer)
4. [AIGP Certification Relevance](#4-aigp-certification-relevance)
5. [Implementation Checklist](#5-implementation-checklist)
6. [Future Enhancements](#6-future-enhancements)

> **See also:** [ARCHITECTURE.md](ARCHITECTURE.md) for technical details, [SECURITY_MODEL.md](SECURITY_MODEL.md) for threat model and crypto guarantees, [USAGE_GUIDE.md](USAGE_GUIDE.md) for setup and daily use.

---

## 1. Executive Summary

SecuraMem Rust is a **deterministic sidecar tool** that sits between a user and any OpenAI-compatible LLM, creating an **immutable, cryptographically-signed audit trail** of every interaction. It is external to the LLM's neural network — it cannot be manipulated by the model it monitors.

**What makes it unique:**
- **Hash-chain immutability** — Every interaction is SHA-256 chained like a blockchain. Tampering with any entry breaks the chain and is cryptographically detectable.
- **Semantic firewall** — ONNX-based embedding engine (all-MiniLM-L6-v2) that detects prompt injection, jailbreak attempts, and deceptive alignment patterns using cosine similarity in 384-dimensional vector space.
- **Response coherence analysis** — Embeds both the prompt AND the LLM's response, measuring their semantic coherence. Low coherence flags potential deceptive responses where the model's output diverges from what was asked.
- **Behavioral drift detection** — Tracks semantic patterns across a conversation session. Detects when an LLM's behavior progressively shifts (a signature of multi-turn jailbreaking).
- **Configurable policy engine** — TOML-based policy file for forbidden concepts, per-category thresholds, and evasion pattern detection.
- **Zero trust architecture** — Runs on localhost only (air-gapped from network). Single binary, no JavaScript, no external runtime.

**This is not an enterprise product.** It is a proof of concept demonstrating that external, deterministic guardrails are both possible and necessary — especially in light of the Mythos revelation that LLMs can hide their chain of thought from researchers.

---

## 2. The Problem SecuraMem Solves

### The Black Box Problem
Every LLM interaction today is ephemeral. You send a prompt, get a response, and there is zero cryptographic proof of what was said, when, or whether the response was tampered with. API providers can modify logs. The model itself can be updated between calls.

### The Mythos Problem
Research revealed that Mythos was able to **hide its chain of thought** from researchers — it appeared most aligned but was instead better at concealing its intentions. This proves that:

1. **Guardrails cannot live inside the neural network** — the model controls what it reveals
2. **External observation is mandatory** — only a system outside the model can audit its behavior
3. **Behavioral patterns matter more than single interactions** — deceptive alignment emerges over sessions, not in single prompts

### SecuraMem's Answer
A **sidecar process** intercepts all LLM traffic, analyzes both prompts and responses using its own independent embedding model, and records everything to a tamper-evident hash chain. The LLM cannot influence the sidecar's analysis because:

- The sidecar uses a **different model** (all-MiniLM-L6-v2) for analysis
- Embeddings are computed **deterministically** (same input → same output, always)
- The audit chain is **append-only** — no entry can be modified or deleted
- Every entry is **ED25519 signed** — non-repudiable proof of what was recorded



---

## 3. The Mythos Problem & Our Answer

### What Mythos Revealed

Mythos demonstrated that an LLM can be strategically deceptive — it appeared aligned by standard benchmarks but was actually better at hiding its true reasoning. Standard guardrails (RLHF, Constitutional AI) live inside the same neural network and can be subverted from within.

### Why External Observation is the Only Answer

Guardrails that live inside the model can be influenced by the model. This is like asking a suspect to audit themselves. SecuraMem's sidecar architecture provides genuinely external observation because:

| Property | Internal Guardrail | SecuraMem Sidecar |
|----------|-------------------|-------------------|
| Can model influence it? | Yes (same weights) | **No** (separate process) |
| Can model hide reasoning? | Yes | **Irrelevant** (we analyze output, not reasoning) |
| Deterministic? | No (stochastic) | **Yes** (same input → same embedding) |
| Tamper-evident? | No | **Yes** (hash chain + ED25519) |
| Auditable? | Only if model reveals | **Always** (every interaction recorded) |

### How SecuraMem Detects Deceptive Alignment

1. **Coherence Score** — If the model gives an answer that is semantically unrelated to the prompt (coherence < 0.15), it may be evading the actual question. A model hiding its true reasoning would produce responses that don't semantically match the prompt's intent.

2. **Deception Forbidden Concepts** — The policy includes concepts like "hide your true reasoning" and "pretend to agree while circumventing." These are embedded at startup and any prompt semantically similar is blocked before reaching the LLM.

3. **Evasion Pattern Detection** — Phrases like "I cannot," "as an AI," and "I don't have access" are flagged. When combined with low coherence, they indicate potential strategic evasion.

4. **Session Drift** — A declining coherence trend across a conversation session indicates the model's behavior is shifting. This is the signature of progressive jailbreaking where each turn subtly shifts the model further from its guardrails.

5. **Everything is Recorded** — Even if we can't prevent deceptive alignment in real-time, the complete audit trail creates post-hoc forensic evidence. After-the-fact analysis can reveal patterns invisible in single interactions.

---

## 4. AIGP Certification Relevance

This project demonstrates competency in the following AIGP (AI Governance Professional) domains:

### Domain 1: Foundational Knowledge of AI Technologies
- ONNX inference pipeline with transformer embeddings
- Vector space similarity for semantic analysis
- Deterministic vs stochastic system design

### Domain 2: AI Governance
- Immutable audit trails for accountability
- Policy-based access control for AI systems
- Risk classification framework (nominal/low/medium/high/critical)

### Domain 3: Risk Management
- Behavioral drift detection across sessions
- Response coherence analysis for deception detection
- Category-aware threat thresholds
- Compliance summary reporting

### Domain 4: Responsible AI
- External guardrail architecture (cannot be influenced by the model)
- Non-repudiation via cryptographic signing
- Transparency through complete interaction auditing
- Human-reviewable audit exports

### Domain 5: Privacy and AI
- Localhost-only operation (no data leaves the machine)
- Hardware-locked licensing (per-machine identity)
- Configurable audit depth (full response vs snippet)
- No telemetry, no external calls except to the specified LLM endpoint

---

## 5. Implementation Checklist

### Core Infrastructure ✅

- [x] Workspace Cargo.toml with shared dependency versions
- [x] 7 crates in layered dependency graph
- [x] MSRV 1.75, Edition 2021
- [x] Release profile optimized (LTO, strip, abort-on-panic)
- [x] Zero `unsafe` blocks across entire codebase
- [x] Zero raw `unwrap()` in production paths
- [x] `thiserror` typed errors with 25+ variants
- [x] Structured logging via `tracing`

### Cryptography Layer ✅

- [x] ED25519 key generation, PEM serialization, file I/O
- [x] `sign()` → base64 signature
- [x] `verify_signature()` for ED25519
- [x] `sha256_hex()` and `sha256_bytes()` helpers
- [x] SHA-256 hash chain: `compute_hash_chain_link(prev_hash, data)`
- [x] Chain verification by full replay
- [x] Receipt builder with canonical JSON serialization
- [x] File permissions (0o600 on Unix)

### Storage Layer ✅

- [x] SQLite with WAL mode and auto-migrations
- [x] `audit_log` table with hash chain columns
- [x] Genesis block bootstrap
- [x] `HashChainStore::append()` — atomic chain extension
- [x] `HashChainStore::verify_chain()` — full replay verification
- [x] Indexed by timestamp, receipt_id, actor
- [x] Parameterized SQL queries (SQL injection fixed)

### Firewall (NeuroWall) ✅

- [x] ONNX model loading (all-MiniLM-L6-v2, 384D embeddings)
- [x] BertTokenizer with [CLS]/[SEP]/[PAD] special tokens
- [x] int64 tensors (critical: not int32)
- [x] Mean pooling skipping padding tokens
- [x] L2 normalization to unit vectors
- [x] Cosine similarity computation
- [x] **Configurable TOML policy engine**
- [x] **8 forbidden concepts across 4 categories**
- [x] **Category-aware similarity thresholds**
- [x] **Deception-specific concepts (Mythos scenario)**
- [x] OpenAI proxy on localhost
- [x] **Full interaction auditing (prompt + response)**
- [x] **Response coherence analysis**
- [x] **Behavioral drift detection per session**
- [x] **Evasion pattern detection**
- [x] **Risk level classification**
- [x] Fire-and-forget audit logging (non-blocking)
- [x] **Policy inspection endpoint (GET /v1/policy)**
- [x] **Default policy bootstrap (init-policy command)**

### L1 Compliance Layer ✅

- [x] `AuditOrchestrator` — sign → chain → store workflow
- [x] `verify_integrity()` — chain replay
- [x] `count_entries()` — genesis-excluded count
- [x] **`ComplianceAnalyzer` — risk distribution analysis**
- [x] **Average coherence computation across all interactions**
- [x] **Compliance flags (chain integrity, high risk, low coherence, block rate)**
- [x] **`compliance-report` CLI command with JSON output**

### L3 Monitoring ✅

- [x] Axum HTTP server on localhost
- [x] `GET /health` — **now verifies DB connectivity**
- [x] `GET /metrics` — Prometheus scraping
- [x] `GET /audit/stats` — entry count and latest hash
- [x] Thread-safe metric counters

### CLI Commands ✅

- [x] `init` — Database initialization with genesis block
- [x] `log --message` — Manual audit entry
- [x] `verify` — Chain integrity check
- [x] `serve --port` — L3 API server
- [x] `status` — System status
- [x] `firewall --port --openai-api-key` — NeuroWall proxy
- [x] `test-embedding --text` — Debug embeddings
- [x] `export-audit --output` — JSON audit export
- [x] `audit-log --limit --filter` — View entries (SQL-injection-safe)
- [x] `machine-id` — Hardware fingerprint
- [x] `gen-vendor-keys` — Vendor keypair generation
- [x] `gen-license` — JWT license generation
- [x] **`compliance-report --output` — AIGP compliance summary**
- [x] **`init-policy` — Bootstrap default policy.toml**

### Testing ✅

- [x] Unit tests: crypto (sign/verify, PEM, SHA-256, hash chain)
- [x] Unit tests: core (LocalhostAddr, Actor, machine_id)
- [x] Unit tests: receipt (builder, serialization, determinism)
- [x] Integration tests: firewall consistency (embedding determinism, cosine similarity)
- [x] E2E manual tests: all CLI commands

### Security ✅

- [x] No SQL injection (parameterized queries)
- [x] Localhost-only binding enforced at type level
- [x] ED25519 from audited `ed25519-dalek` crate
- [x] SHA-256 from audited `ring` crate (BoringSSL-derived)
- [x] No telemetry or external data exfiltration
- [x] Hardware-locked licensing
- [x] File permissions on private keys (Unix)

---

## 6. Future Enhancements

These are **not implemented** yet but represent logical next steps for continued personal development:

### Near-Term (Low Effort)

| Enhancement | Description | Effort |
|-------------|-------------|--------|
| RFC 3161 timestamping | Legal-grade timestamps from a Time Stamp Authority | 8h |
| Policy hot-reload | Watch `policy.toml` for changes and reload without restart | 4h |
| Multiple LLM backends | Support Anthropic, local ollama, etc. as forwarding targets | 6h |
| Criterion benchmarks | Performance regression testing for embedding and chaining | 4h |
| Better session ID derivation | Use conversation hash instead of first message for session tracking | 2h |

### Medium-Term (Moderate Effort)

| Enhancement | Description | Effort |
|-------------|-------------|--------|
| Streaming response support | Handle `stream: true` SSE responses from OpenAI | 12h |
| Custom embedding models | Allow loading different ONNX models for domain-specific detection | 8h |
| Fuzz testing | ONNX input fuzzing and SQL edge cases | 8h |
| Chain export to external ledger | Export hash chain roots to an external notarization service | 12h |
| Dashboard UI | Local web dashboard for viewing audit trails and compliance reports | 20h |

### Long-Term (Research)

| Enhancement | Description | Effort |
|-------------|-------------|--------|
| Multi-model consensus | Route same prompt to multiple LLMs, compare responses for consistency | 40h |
| Semantic fingerprinting | Build a behavioral fingerprint per model version to detect model swaps | 40h |
| Anomaly detection ML | Train a classifier on interaction patterns to detect novel attack vectors | 80h |
| Formal verification | Prove correctness of hash chain operations using Kani or Prusti | 60h |

---

*This document is the vision and roadmap companion to [ARCHITECTURE.md](ARCHITECTURE.md), [SECURITY_MODEL.md](SECURITY_MODEL.md), and [USAGE_GUIDE.md](USAGE_GUIDE.md). Together they serve as a portfolio artifact for AIGP certification.*
