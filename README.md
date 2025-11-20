# SecuraMem - Rust-Powered AI Audit & Security Platform

**Version:** 2.0.0
**Status:** Production Ready ✅
**License:** Proprietary (Acquisition Ready)

## Overview

SecuraMem is an enterprise-grade AI security platform providing cryptographic audit trails and semantic threat detection for AI systems.

### Key Features

- **NeuroWall Semantic Firewall** - ONNX-powered jailbreak detection (384D embeddings)
- **Cryptographic Audit Trail** - Blockchain-style immutable ledger (SHA-256 + Ed25519)
- **Hardware Node-Locking** - Ed25519 JWT licensing with machine fingerprinting
- **Zero External Dependencies** - Single 100MB binary with embedded AI model
- **Compliance Ready** - SOC 2, HIPAA, GDPR, AI Executive Order aligned

## Quick Start

```bash
# Initialize database and identity
./smem init

# Check system status
./smem status

# Start semantic firewall
export OPENAI_API_KEY=sk-...
./smem firewall --port 3051

# Verify audit chain integrity
./smem verify
```

## Architecture

### 4-Layer Design

| Layer | Component | Purpose |
|-------|-----------|---------|
| **L1** | Compliance Engine | Ed25519 signatures, SHA-256 chaining, immutable ledger |
| **L2** | Data Storage | SQLx async, SQLite WAL, foreign key enforcement |
| **L3** | Monitoring | Prometheus metrics, Axum HTTP server, health checks |
| **L5** | NeuroWall | ONNX inference, 384D embeddings, threat detection |

## Build from Source

```bash
# Clean build
cargo clean

# Release build (hardened profile)
cargo build --release

# Output: target/release/smem.exe (100 MB)
```

### Build Profile

```toml
[profile.release]
opt-level = "z"          # Size optimization
lto = "fat"              # Link-time optimization
strip = true             # Strip debug symbols
panic = "abort"          # No unwinding
overflow-checks = true   # Runtime safety
```

## Technical Specifications

| Property | Value |
|----------|-------|
| Binary Size | 100 MB |
| Embedded Model | all-MiniLM-L6-v2 (90 MB ONNX) |
| Embedding Dimensions | 384 |
| Runtime Dependencies | ZERO (fully static) |
| SIMD Optimizations | AVX2/FMA |

## Commands

```bash
smem init              # Initialize database and identity
smem status            # Show system status
smem verify            # Verify audit chain integrity
smem machine-id        # Show hardware fingerprint
smem firewall          # Start semantic firewall proxy
smem test-embedding    # Test embedding generation
smem serve             # Start L3 API server
smem log               # Log test event
```

## Documentation

- [Golden Binary Build](GOLDEN_BINARY_BUILD.md) - Build process and hardening
- [Verification Report](GOLDEN_BINARY_VERIFICATION.md) - Acceptance tests
- [Post-Build Summary](POST_BUILD_SUMMARY.md) - Executive summary
- [Defense Kit README](DEFENSE_KIT_README.txt) - Customer guide
- [Phase 5 Details](PHASE5_NEUROWALL_COMPLETE.md) - NeuroWall implementation

## Binary Verification

```bash
certutil -hashfile smem.exe SHA256
# Expected: f3fd2701a1bf8daff84b3d3faf5bf738a78fb6f4d1e2a9466dadcf9455728ab1
```

## License

**Proprietary Software** - All Rights Reserved

For licensing: sales@securamem.com

---

**Build Date:** 2025-11-19
**Status:** PRODUCTION READY ✅
