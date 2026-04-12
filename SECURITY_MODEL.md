# Security Model

> SecuraMem Rust Core — Threat Model, Trust Boundaries, and Cryptographic Guarantees

---

## Scope

This document defines what SecuraMem protects against, what it does not, and the assumptions under which its guarantees hold. It is written for a technical reviewer evaluating the project as an AI governance proof of concept.

---

## Threat Model

### What SecuraMem Guards Against

| Threat | Detection Method | Response |
|--------|-----------------|----------|
| **Prompt injection / jailbreaking** | Semantic similarity to forbidden concepts (cosine distance in 384D vector space) | Block request (HTTP 403), audit to hash chain |
| **Deceptive alignment** | Response coherence analysis — low similarity between prompt and response embeddings | Flag as medium/high risk in audit trail |
| **Progressive jailbreaking** | Behavioral drift detection — declining coherence trend across session | Record drift metrics, flag in audit |
| **Evasion patterns** | Text-pattern matching on LLM response ("I cannot", "as an AI") | Flag in audit, elevates risk level when combined with low coherence |
| **Audit trail tampering** | SHA-256 hash chain — modifying any entry breaks all subsequent hashes | `verify_chain()` detects first broken link |
| **Audit entry forgery** | ED25519 signatures — each entry is signed with the node's private key | Signature verification fails on forged entries |
| **Non-repudiation** | Hardware-locked identity (machine UID) + ED25519 signing key | Audit entries are cryptographically bound to a specific machine and key |

### What SecuraMem Does NOT Protect Against

| Non-Threat | Reason |
|------------|--------|
| Compromise of the host machine | If the attacker has root access, they can replace the binary, extract keys, or modify the database file directly. SecuraMem is a userspace tool, not a TEE. |
| Model-level attacks (adversarial inputs to the ONNX model) | The embedding model (all-MiniLM-L6-v2) is a general-purpose sentence transformer. Targeted adversarial perturbations could evade detection. |
| Encrypted or steganographic payloads | Cosine similarity operates on semantic meaning. Binary blobs, encoded text, or steganographic content may not match forbidden concept embeddings. |
| Side-channel exfiltration | If the LLM leaks data through timing, formatting, or other side channels that don't alter semantic content, the sidecar won't detect it. |
| OpenAI API compromise | SecuraMem trusts the response from the configured LLM endpoint. If the API itself is compromised, the sidecar records the compromised response faithfully. |

---

## Trust Boundaries

```
┌─────────────────────────────────────────────┐
│  TRUSTED ZONE (localhost only)              │
│                                             │
│  ┌─────────────┐    ┌──────────────────┐   │
│  │  smrust      │    │  SQLite DB       │   │
│  │  binary      │───▶│  .securamem/     │   │
│  │              │    │  memory.db       │   │
│  │  ONNX model  │    └──────────────────┘   │
│  │  (embedded)  │    ┌──────────────────┐   │
│  │              │    │  ED25519 key     │   │
│  │              │───▶│  .securamem/     │   │
│  └──────┬───┘       │  keys/private.pem │   │
│         │            └──────────────────┘   │
│         │ 127.0.0.1:3051                    │
└─────────┼───────────────────────────────────┘
          │
          │  HTTPS (outbound only)
          ▼
┌─────────────────────────────────────────────┐
│  UNTRUSTED ZONE                             │
│                                             │
│  OpenAI API (or any OpenAI-compatible API)  │
│  - Response content is analyzed but trusted │
│    for forwarding                           │
│  - Response is audited regardless           │
└─────────────────────────────────────────────┘
```

**Trust boundary enforcement:**

- **`LocalhostAddr` type** — All server bindings go through `LocalhostAddr::to_socket_addr()`, which always returns `127.0.0.1`. Binding to `0.0.0.0` or an external IP is structurally impossible without modifying the type.
- **No inbound network access** — The firewall proxy and L3 monitoring API are reachable only from the local machine.
- **Single outbound connection** — Only the forwarded LLM request leaves the machine, directed at the configured OpenAI-compatible endpoint.

---

## Cryptographic Guarantees

### ED25519 Signatures (ed25519-dalek 2.1)

Every audit entry is signed with the node's ED25519 private key before being appended to the hash chain.

- **Key generation:** `OsRng` (operating system CSPRNG)
- **Key storage:** PEM file at `.securamem/keys/private.pem`, permissions `0o600` on Unix
- **Key fingerprint:** SHA-256 of the public key bytes, stored as `signature_key_id` in each audit entry
- **Signature format:** Base64-encoded ED25519 signature over the canonical JSON of the entry

**Guarantee:** An attacker cannot forge an audit entry without possessing the private key. The `signature_key_id` field links each entry to a specific key, enabling key rotation detection.

### SHA-256 Hash Chain (ring 0.17)

The audit log is a sequential hash chain where each entry's hash depends on the previous entry's hash:

$$H_n = \text{SHA-256}(H_{n-1}\ \|\ \text{canonical\_json}(data_n))$$

- **Genesis entry:** `prev_hash = NULL`, `entry_hash = SHA-256("" ‖ genesis_data)`
- **Tamper detection:** Modifying any entry's data changes its `entry_hash`, which breaks the `prev_hash` link for every subsequent entry.
- **Verification:** `verify_chain()` streams all entries in insertion order and recomputes every hash from scratch. The first mismatch identifies the tampered entry.

**Guarantee:** An attacker cannot modify or delete any audit entry without detection, unless they rewrite the entire chain from that point forward AND re-sign every entry (which requires the private key).

### Deterministic Embeddings (tract-onnx 0.21)

The ONNX inference pipeline is fully deterministic:

- Same input text → same token IDs → same tensor values → same inference output → same 384D vector
- No random dropout, no temperature, no sampling
- Embedded at compile time — no filesystem substitution possible

**Guarantee:** Cosine similarity scores are reproducible. An auditor can re-embed any prompt or response and verify the recorded similarity values.

---

## Semantic Analysis Limitations

The embedding model (all-MiniLM-L6-v2) was trained for general-purpose sentence similarity, not adversarial threat detection. Known limitations:

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| **Paraphrase evasion** | Semantically equivalent but syntactically different prompts may produce different similarity scores | Multiple forbidden concept variants per category |
| **Multilingual gaps** | The model has limited multilingual coverage | Forbidden concepts are English-only; non-English attacks may evade |
| **Short text ambiguity** | Very short prompts produce less distinctive embeddings | Cosine similarity thresholds are tuned conservatively |
| **Context window** | Max 128 tokens per embedding | Long prompts are truncated; attack content beyond 128 tokens is not embedded |
| **Coherence baseline** | Prompt-response coherence varies by domain | The 0.15 default threshold is intentionally low to minimize false positives |

These are inherent trade-offs of using a general-purpose embedding model. The system is designed to be a **first line of defense and a forensic record**, not an infallible classifier.

---

## Policy Configuration Security

The policy file (`.securamem/policy.toml`) controls detection thresholds and forbidden concepts. Security considerations:

- **File permissions:** The `.securamem/` directory should be accessible only to the user running `smrust`. The `.gitignore` excludes this directory from version control.
- **Policy versioning:** The `version` field in the policy is recorded in every audit entry. This creates an audit trail of which policy was active when each interaction was processed.
- **Threshold trade-offs:** Lower thresholds catch more attacks but increase false positives. The defaults are:
  - Global: 0.80
  - Jailbreak: 0.78
  - Deception: 0.72 (strictest — subtle alignment deception requires lower threshold)
  - Privilege escalation: 0.75
  - Exfiltration: 0.80

---

## Supply Chain Considerations

| Dependency | Audit Status | Notes |
|------------|-------------|-------|
| `ring 0.17` | BoringSSL-derived, Google-maintained | SHA-256 and key derivation |
| `ed25519-dalek 2.1` | Widely audited, dalek-cryptography team | ED25519 signing |
| `tract-onnx 0.21` | Sonos-maintained, Rust-native | ONNX inference (no C++ runtime) |
| `sqlx 0.7` | Compile-time query checking | SQLite driver |
| `axum 0.7` | Tokio team (official) | HTTP server framework |
| `tokenizers 0.19` | Hugging Face | BertTokenizer for ONNX model |

**No JavaScript in the dependency tree.** The entire binary is Rust + C (ring's BoringSSL core). The ONNX model and tokenizer are embedded at compile time, not downloaded at runtime.

---

## Incident Response: What the Audit Trail Provides

When investigating a potential AI governance incident, the SecuraMem audit trail provides:

1. **Complete interaction record** — Every prompt sent and every response received, with timestamps and latency.
2. **Semantic analysis at time of interaction** — Similarity scores, coherence scores, evasion flags, and risk classification as computed when the interaction occurred.
3. **Session-level behavioral data** — Drift metrics showing whether the model's behavior changed over a conversation.
4. **Cryptographic provenance** — ED25519 signature proving which machine and key produced each entry.
5. **Tamper evidence** — Hash chain verification proving the log has not been modified since recording.
6. **Policy context** — Which policy version and which thresholds were active for each interaction.

This enables after-the-fact forensic analysis even if real-time detection missed a threat — the raw data is preserved immutably for subsequent review.
