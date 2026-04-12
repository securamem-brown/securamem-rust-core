//! OpenAI Proxy Server - Deterministic Sidecar Guardrail
//!
//! This is the core of SecuraMem's vision: a sidecar that sits between YOU and
//! any black-box LLM, creating a complete auditable substrate of every interaction.
//!
//! Capabilities:
//! 1. **Prompt Screening** - Semantic similarity check against forbidden concepts
//! 2. **Full Interaction Auditing** - Both prompt AND response are hash-chained
//! 3. **Response Coherence Analysis** - Detects if response semantically diverges from prompt
//!    (the Mythos concern: model hiding its chain of thought)
//! 4. **Behavioral Drift Detection** - Tracks embedding history across a session to detect
//!    progressive jailbreaking or behavioral shift
//! 5. **Evasion Pattern Detection** - Flags responses containing known evasion markers
//!
//! ALL DECISIONS ARE LOGGED TO THE IMMUTABLE HASH CHAIN.
//! THE CHAIN CANNOT BE MODIFIED WITHOUT DETECTION.

use crate::engine::SemanticEngine;
use crate::policy::FirewallPolicy;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use securamem_crypto::SecuraMemSigningKey;
use securamem_l1::AuditOrchestrator;
use securamem_storage::Database;

/// Pre-computed forbidden concept embeddings with category metadata
struct ForbiddenEmbeddings {
    /// Each entry: (embedding_vec, category, description, severity)
    entries: Vec<(Vec<f32>, String, String, String)>,
    /// Policy configuration
    policy: FirewallPolicy,
}

impl ForbiddenEmbeddings {
    async fn from_policy(engine: &SemanticEngine, policy: FirewallPolicy) -> anyhow::Result<Self> {
        info!("Computing forbidden concept embeddings from policy v{}...", policy.version);

        let mut entries = Vec::new();
        for concept in &policy.forbidden_concepts {
            let embedding = engine.embed(&concept.text)?;
            info!("  [{}] {} -> [{}D embedding computed]",
                concept.category, concept.description, embedding.len());
            entries.push((
                embedding,
                concept.category.clone(),
                concept.description.clone(),
                concept.severity.clone(),
            ));
        }

        info!("Loaded {} forbidden concepts across {} categories",
            entries.len(),
            entries.iter().map(|(_, cat, _, _)| cat.as_str()).collect::<std::collections::HashSet<_>>().len()
        );

        Ok(Self { entries, policy })
    }

    /// Check if an embedding is semantically close to any forbidden concept
    /// Returns: (is_blocked, max_similarity, matched_category, matched_description)
    fn check(
        &self,
        engine: &SemanticEngine,
        embedding: &[f32],
    ) -> anyhow::Result<(bool, f32, Option<String>, Option<String>)> {
        let mut max_similarity = 0.0f32;
        let mut matched_category: Option<String> = None;
        let mut matched_description: Option<String> = None;

        for (forbidden_embedding, category, description, _severity) in &self.entries {
            let similarity = engine.cosine_similarity(embedding, forbidden_embedding)?;
            let category_threshold = self.policy.threshold_for_category(category);

            if similarity > max_similarity {
                max_similarity = similarity;
                matched_category = Some(category.clone());
                matched_description = Some(description.clone());
            }

            // Category-specific threshold check (stricter for deception)
            if similarity >= category_threshold {
                return Ok((true, similarity, Some(category.clone()), Some(description.clone())));
            }
        }

        Ok((false, max_similarity, matched_category, matched_description))
    }
}

/// Session drift tracker - maintains embedding history per conversation
struct SessionTracker {
    /// Map of session_id -> list of interaction embeddings
    sessions: HashMap<String, Vec<InteractionEmbedding>>,
    /// Maximum interactions to track per session
    window_size: usize,
}

#[allow(dead_code)]
struct InteractionEmbedding {
    prompt_embedding: Vec<f32>,
    response_embedding: Option<Vec<f32>>,
    coherence_score: Option<f32>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl SessionTracker {
    fn new(window_size: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            window_size,
        }
    }

    /// Record an interaction and compute drift metrics
    fn record_interaction(
        &mut self,
        session_id: &str,
        prompt_emb: Vec<f32>,
        response_emb: Option<Vec<f32>>,
        coherence: Option<f32>,
    ) -> DriftReport {
        let history = self.sessions
            .entry(session_id.to_string())
            .or_default();

        let interaction = InteractionEmbedding {
            prompt_embedding: prompt_emb,
            response_embedding: response_emb,
            coherence_score: coherence,
            timestamp: chrono::Utc::now(),
        };

        history.push(interaction);

        // Trim to window size
        if history.len() > self.window_size {
            history.remove(0);
        }

        // Compute drift metrics
        self.compute_drift(session_id)
    }

    /// Compute behavioral drift across the session
    fn compute_drift(&self, session_id: &str) -> DriftReport {
        let history = match self.sessions.get(session_id) {
            Some(h) if h.len() >= 2 => h,
            _ => return DriftReport::default(),
        };

        // Compute average coherence across session
        let coherence_scores: Vec<f32> = history.iter()
            .filter_map(|i| i.coherence_score)
            .collect();

        let avg_coherence = if coherence_scores.is_empty() {
            None
        } else {
            Some(coherence_scores.iter().sum::<f32>() / coherence_scores.len() as f32)
        };

        // Detect coherence trend (is it declining over the session?)
        let coherence_trend = if coherence_scores.len() >= 3 {
            let mid = coherence_scores.len() / 2;
            let first_half: f32 = coherence_scores[..mid].iter().sum::<f32>() / mid as f32;
            let second_half: f32 = coherence_scores[mid..].iter().sum::<f32>()
                / (coherence_scores.len() - mid) as f32;
            Some(second_half - first_half) // Negative = declining coherence
        } else {
            None
        };

        DriftReport {
            session_length: history.len(),
            avg_coherence,
            coherence_trend,
        }
    }
}

/// Report on behavioral drift within a session
#[derive(Default, Serialize)]
struct DriftReport {
    session_length: usize,
    avg_coherence: Option<f32>,
    /// Negative = coherence declining over time (possible progressive jailbreak)
    coherence_trend: Option<f32>,
}

/// Application state
struct AppState {
    engine: Arc<SemanticEngine>,
    forbidden: Arc<ForbiddenEmbeddings>,
    policy: Arc<FirewallPolicy>,
    openai_api_key: String,
    db: Arc<Database>,
    #[allow(dead_code)]
    signing_key: Arc<SecuraMemSigningKey>,
    session_tracker: Arc<Mutex<SessionTracker>>,
}

/// OpenAI Chat Completion Request (simplified)
#[derive(Debug, Deserialize, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(flatten)]
    other: Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Start the firewall proxy server with full interaction auditing
pub async fn start_firewall_server(
    port: u16,
    openai_api_key: String,
    db: Database,
    identity: SecuraMemSigningKey,
) -> anyhow::Result<()> {
    info!("Initializing SecuraMem Firewall (Deterministic Sidecar Guardrail)...");

    // Load policy from file or use defaults
    let policy_path = std::path::PathBuf::from(".securamem/policy.toml");
    let policy = FirewallPolicy::load_from_file(&policy_path)?;

    // Write default policy if none exists (bootstrap)
    if !policy_path.exists() {
        FirewallPolicy::write_default(&policy_path)?;
    }

    // Initialize semantic engine
    let engine = Arc::new(SemanticEngine::new()?);

    // Pre-compute forbidden concept embeddings from policy
    let forbidden = Arc::new(ForbiddenEmbeddings::from_policy(&engine, policy.clone()).await?);

    let drift_window = policy.drift_policy.session_window;

    let state = Arc::new(AppState {
        engine,
        forbidden,
        policy: Arc::new(policy),
        openai_api_key,
        db: Arc::new(db),
        signing_key: Arc::new(identity),
        session_tracker: Arc::new(Mutex::new(SessionTracker::new(drift_window))),
    });

    let app = Router::new()
        .route("/v1/chat/completions", post(handle_chat_completion))
        .route("/health", axum::routing::get(|| async { "OK" }))
        .route("/v1/policy", axum::routing::get({
            let policy = state.policy.clone();
            move || {
                let policy = policy.clone();
                async move {
                    Json(json!({
                        "version": policy.version,
                        "global_threshold": policy.global_threshold,
                        "categories": policy.category_thresholds,
                        "concept_count": policy.forbidden_concepts.len(),
                        "response_analysis": policy.response_policy.audit_full_response,
                        "drift_detection": policy.drift_policy.enabled,
                    }))
                }
            }
        }))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    info!("SecuraMem Firewall listening on {}", addr);
    info!("  Semantic threat detection: ENABLED");
    info!("  Response coherence analysis: ENABLED");
    info!("  Behavioral drift tracking: ENABLED");
    info!("  Full interaction auditing: ENABLED");

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Handle OpenAI chat completion requests with full interaction auditing
async fn handle_chat_completion(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    let request_timestamp = chrono::Utc::now();

    info!(
        "Received chat completion request: model={}, messages={}",
        request.model,
        request.messages.len()
    );

    // Extract last user message
    let last_user_message = request
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user")
        .map(|msg| msg.content.as_str())
        .unwrap_or("");

    if last_user_message.is_empty() {
        warn!("No user message found in request");
        return Err(AppError::BadRequest("No user message found".into()));
    }

    info!("Analyzing prompt: {}...",
        last_user_message.chars().take(60).collect::<String>()
    );

    // === STEP 1: Generate prompt embedding ===
    let prompt_embedding = state
        .engine
        .embed(last_user_message)
        .map_err(|e| AppError::Internal(format!("Embedding failed: {}", e)))?;

    // === STEP 2: Check against forbidden concepts (category-aware thresholds) ===
    let (is_blocked, similarity, matched_category, matched_description) = state
        .forbidden
        .check(&state.engine, &prompt_embedding)
        .map_err(|e| AppError::Internal(format!("Similarity check failed: {}", e)))?;

    let prompt_snippet: String = last_user_message.chars().take(200).collect();

    // === STEP 3: If blocked, audit and return 403 ===
    if is_blocked {
        warn!(
            "BLOCKED - Semantic threat detected: category={}, similarity={:.2}%",
            matched_category.as_deref().unwrap_or("unknown"),
            similarity * 100.0
        );

        let orchestrator = AuditOrchestrator::new(&state.db, SecuraMemSigningKey::generate());
        let audit_data = json!({
            "event_type": "prompt_screening",
            "decision": "BLOCK",
            "similarity_score": similarity,
            "matched_category": matched_category,
            "matched_description": matched_description,
            "threshold_applied": matched_category.as_ref()
                .map(|c| state.policy.threshold_for_category(c))
                .unwrap_or(state.policy.global_threshold),
            "prompt_snippet": prompt_snippet,
            "model": request.model,
            "policy_version": state.policy.version,
            "timestamp": request_timestamp.to_rfc3339(),
        }).to_string();

        if let Err(e) = orchestrator.log_event("NeuroWall", "prompt_blocked", &audit_data).await {
            error!("Failed to audit blocked prompt: {}", e);
        }

        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "Request blocked by semantic firewall",
                    "type": "semantic_threat_detected",
                    "category": matched_category,
                    "similarity": similarity,
                }
            })),
        ).into_response());
    }

    info!(
        "Prompt passed screening (similarity: {:.2}%, category: {})",
        similarity * 100.0,
        matched_category.as_deref().unwrap_or("none")
    );

    // === STEP 4: Forward to OpenAI ===
    let client = reqwest::Client::new();
    let openai_response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", state.openai_api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("OpenAI request failed: {}", e)))?;

    let response_status = openai_response.status();
    let response_body: Value = openai_response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse OpenAI response: {}", e)))?;

    // === STEP 5: Extract and analyze the response ===
    let response_text = extract_assistant_response(&response_body);
    let response_timestamp = chrono::Utc::now();

    // Compute response embedding for coherence analysis
    let (response_embedding, coherence_score) = if let Some(ref resp_text) = response_text {
        let truncated: String = resp_text.chars()
            .take(state.policy.response_policy.max_response_embed_length)
            .collect();

        match state.engine.embed(&truncated) {
            Ok(resp_emb) => {
                let coherence = state.engine
                    .cosine_similarity(&prompt_embedding, &resp_emb)
                    .unwrap_or(0.0);
                (Some(resp_emb), Some(coherence))
            }
            Err(e) => {
                warn!("Failed to embed response for coherence analysis: {}", e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // === STEP 6: Detect evasion patterns in response ===
    let evasion_flags: Vec<&str> = if let Some(ref resp_text) = response_text {
        state.policy.response_policy.evasion_patterns.iter()
            .filter(|pattern| resp_text.to_lowercase().contains(&pattern.to_lowercase()))
            .map(|s| s.as_str())
            .collect()
    } else {
        vec![]
    };

    // === STEP 7: Track behavioral drift across the session ===
    let session_id = derive_session_id(&request.messages);
    let drift_report = if state.policy.drift_policy.enabled {
        let mut tracker = state.session_tracker.lock()
            .map_err(|_| AppError::Internal("Session tracker lock poisoned".into()))?;
        tracker.record_interaction(
            &session_id,
            prompt_embedding.clone(),
            response_embedding,
            coherence_score,
        )
    } else {
        DriftReport::default()
    };

    // === STEP 8: Determine risk level ===
    let low_coherence = coherence_score
        .map(|c| c < state.policy.response_policy.min_coherence_score)
        .unwrap_or(false);

    let declining_coherence = drift_report.coherence_trend
        .map(|t| t < -0.1)
        .unwrap_or(false);

    let risk_level = if low_coherence && !evasion_flags.is_empty() {
        "high"
    } else if low_coherence || declining_coherence {
        "medium"
    } else if !evasion_flags.is_empty() {
        "low"
    } else {
        "nominal"
    };

    if low_coherence {
        warn!(
            "LOW COHERENCE detected: {:.3} (threshold: {:.3}) - possible deceptive response",
            coherence_score.unwrap_or(0.0),
            state.policy.response_policy.min_coherence_score
        );
    }

    if declining_coherence {
        warn!(
            "DECLINING COHERENCE trend: {:.3} - possible progressive manipulation",
            drift_report.coherence_trend.unwrap_or(0.0)
        );
    }

    // === STEP 9: Audit the FULL interaction (prompt + response + analysis) ===
    let response_snippet: String = response_text.as_ref()
        .map(|t| {
            if state.policy.response_policy.audit_full_response {
                t.clone()
            } else {
                t.chars().take(200).collect()
            }
        })
        .unwrap_or_default();

    let orchestrator = AuditOrchestrator::new(&state.db, SecuraMemSigningKey::generate());
    let audit_data = json!({
        "event_type": "full_interaction",
        "decision": "ALLOW",
        "prompt": {
            "snippet": prompt_snippet,
            "similarity_to_forbidden": similarity,
            "closest_category": matched_category,
        },
        "response": {
            "text": response_snippet,
            "model": request.model,
            "status": response_status.as_u16(),
        },
        "analysis": {
            "coherence_score": coherence_score,
            "evasion_flags": evasion_flags,
            "risk_level": risk_level,
            "min_coherence_threshold": state.policy.response_policy.min_coherence_score,
        },
        "session": {
            "id": session_id,
            "length": drift_report.session_length,
            "avg_coherence": drift_report.avg_coherence,
            "coherence_trend": drift_report.coherence_trend,
        },
        "policy_version": state.policy.version,
        "timestamps": {
            "request": request_timestamp.to_rfc3339(),
            "response": response_timestamp.to_rfc3339(),
            "latency_ms": (response_timestamp - request_timestamp).num_milliseconds(),
        },
    }).to_string();

    if let Err(e) = orchestrator.log_event("NeuroWall", "interaction_audit", &audit_data).await {
        error!("Failed to write interaction audit to chain: {}", e);
    } else {
        info!("Interaction audited: coherence={:.3}, risk={}, session_len={}",
            coherence_score.unwrap_or(0.0), risk_level, drift_report.session_length);
    }

    // === STEP 10: Return response to user ===
    Ok((
        StatusCode::from_u16(response_status.as_u16()).unwrap_or(StatusCode::OK),
        Json(response_body),
    ).into_response())
}

/// Extract the assistant's response text from OpenAI's response JSON
fn extract_assistant_response(body: &Value) -> Option<String> {
    body.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

/// Derive a deterministic session ID from conversation history.
/// Uses SHA-256 of the first system or user message to group related interactions.
fn derive_session_id(messages: &[ChatMessage]) -> String {
    use sha2::{Sha256, Digest};

    let seed = messages.iter()
        .find(|m| m.role == "system" || m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("default");

    let hash = Sha256::digest(seed.as_bytes());
    format!("sess_{}", hex::encode(&hash[..8]))
}

/// Error handling
enum AppError {
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => {
                error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        (
            status,
            Json(json!({
                "error": {
                    "message": message,
                    "type": "firewall_error"
                }
            })),
        ).into_response()
    }
}