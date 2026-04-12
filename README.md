# SecuraMem Rust � Deterministic Sidecar Guardrail for LLMs

**Version:** 2.0.0  
**Status:** Personal proof-of-concept for AIGP Certification  
**License:** Personal project

---

## What Is This?

SecuraMem is a **sidecar tool** that sits between you and any OpenAI-compatible LLM, turning every interaction into an **auditable, tamper-evident record**. It is a single Rust binary � no JavaScript, no Python, no external runtime.

Point your LLM client at `http://127.0.0.1:3051` instead of `https://api.openai.com` and every prompt and response is:

1. **Screened** against forbidden concepts using a local ONNX embedding model
2. **Analyzed** for response coherence (does the answer match the question?)
3. **Tracked** for behavioral drift across a conversation session
4. **Recorded** to an immutable SHA-256 hash chain with ED25519 signatures

The guardrail runs **outside** the LLM's neural network. The model cannot influence, evade, or tamper with it.

## Quick Start

```bash
# Build
cargo build --release

# Initialize audit database (creates genesis block)
smrust init

# (Optional) Generate default policy for customization
smrust init-policy

# Start the sidecar firewall
export OPENAI_API_KEY=sk-...
smrust firewall --port 3051 --openai-api-key $OPENAI_API_KEY

# Use it (drop-in OpenAI replacement)
curl http://127.0.0.1:3051/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}'
```

## Commands

| Command | Description |
|---------|-------------|
| `smrust init` | Initialize database + genesis block |
| `smrust init-policy` | Write default `policy.toml` for customization |
| `smrust firewall --port 3051` | Start the sidecar proxy |
| `smrust verify` | Verify audit chain integrity |
| `smrust audit-log` | View recent audit entries |
| `smrust audit-log --filter interaction_audit` | View interaction audits only |
| `smrust compliance-report` | Generate AIGP compliance summary (JSON) |
| `smrust export-audit` | Export full audit trail to JSON |
| `smrust serve --port 3050` | Start L3 monitoring API |
| `smrust status` | Show system status |
| `smrust test-embedding --text "..."` | Debug: test ONNX embeddings |

## Architecture

```
  User / App
      �
      ?
+---------------------------------------------+
�  NeuroWall Sidecar (localhost:3051)         �
�                                              �
�  Embed prompt ? Screen forbidden concepts   �
�  Forward to OpenAI ? Receive response       �
�  Embed response ? Coherence analysis        �
�  Track session drift ? Classify risk        �
�  Audit everything ? Immutable hash chain    �
+---------------------------------------------+
                       �
                       ?
+---------------------------------------------+
�  SQLite Hash Chain (.securamem/memory.db)   �
�                                              �
�  Entry N: SHA256(prev_hash ? data)          �
�           + ED25519 signature               �
�  Tamper with any entry ? chain breaks       �
+---------------------------------------------+
```

### Crate Layout

| Crate | Purpose |
|-------|---------|
| `securamem-cli` | Binary (`smrust`) � CLI commands |
| `securamem-firewall` | NeuroWall proxy, ONNX engine, policy engine |
| `securamem-l1` | Compliance orchestrator, risk analysis |
| `securamem-l3` | Monitoring HTTP API, Prometheus metrics |
| `securamem-storage` | SQLite hash-chain store |
| `securamem-crypto` | ED25519 signing, SHA-256 hashing |
| `securamem-core` | Shared error types and primitives |

## Policy Configuration

Edit `.securamem/policy.toml` to customize:

- **Forbidden concepts** � Text patterns to block, organized by category
- **Category thresholds** � Per-category similarity thresholds (deception is stricter)
- **Response analysis** � Coherence thresholds, evasion patterns
- **Drift detection** � Session window size, drift threshold

Run `smrust init-policy` to generate the default file.

## Why This Exists

LLMs are black boxes. Research (Mythos) has shown that models can hide their chain of thought � appearing aligned while concealing true intentions. Guardrails inside the neural network can be subverted by the network itself.

SecuraMem proves that **external, deterministic observation** is both possible and necessary. The sidecar uses its own embedding model, its own analysis, and its own tamper-evident storage. The LLM under observation has zero influence over any of it.

## Build

```bash
cargo build --release
# Output: target/release/smrust.exe (~100MB, ONNX model embedded)
```

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Crate graph, data flows, layer details, key algorithms
- **[SECURITY_MODEL.md](SECURITY_MODEL.md)** — Threat model, trust boundaries, cryptographic guarantees
- **[USAGE_GUIDE.md](USAGE_GUIDE.md)** — Setup, configuration, daily use, compliance reporting
- **[SECURAMEM_FINAL_VISION.md](SECURAMEM_FINAL_VISION.md)** — Vision, AIGP alignment, implementation checklist, roadmap
- **[THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt)** � Open source license attributions