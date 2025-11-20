//! L3 Monitoring Layer - API Server and Prometheus Metrics

use axum::{
    extract::State,
    routing::get,
    Router, Json,
};
use prometheus::{Encoder, TextEncoder, Counter, Gauge};
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde_json::json;

use securamem_storage::Database;
use securamem_core::{Result, SecuraMemError};

// --- Metrics Definitions (Thread-safe) ---
struct Metrics {
    http_requests: Counter,
    audit_count: Gauge,
}

impl Metrics {
    fn new() -> Result<Self> {
        use prometheus::{register_counter, register_gauge};

        let http_requests = register_counter!(
            "smem_http_requests_total",
            "Total HTTP requests received"
        ).map_err(|e| SecuraMemError::Internal(e.to_string()))?;

        let audit_count = register_gauge!(
            "smem_audit_entries_count",
            "Current number of audit entries"
        ).map_err(|e| SecuraMemError::Internal(e.to_string()))?;

        Ok(Self {
            http_requests,
            audit_count,
        })
    }
}

// --- App State ---
#[derive(Clone)]
pub struct AppState {
    db: Arc<Database>,
    metrics: Arc<Metrics>,
}

// --- Handlers ---

/// GET /health - Simple Liveness Probe
async fn health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.metrics.http_requests.inc();
    Json(json!({
        "status": "ok",
        "version": "2.0.0",
        "mode": "audit-only"
    }))
}

/// GET /metrics - Prometheus Scraping Endpoint
async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// GET /audit/stats - Read directly from Storage
async fn audit_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.metrics.http_requests.inc();

    // Query total count
    let count: i64 = securamem_storage::sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&state.db.pool)
        .await
        .unwrap_or(0);

    // Update the Gauge
    state.metrics.audit_count.set(count as f64);

    // Get latest hash
    let last_hash: Option<String> = securamem_storage::sqlx::query_scalar(
        "SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1"
    )
        .fetch_optional(&state.db.pool)
        .await
        .unwrap_or(None);

    Json(json!({
        "total_entries": count,
        "latest_hash": last_hash.unwrap_or_else(|| "none".to_string()),
        "integrity_status": "unchecked"
    }))
}

// --- The Server Factory ---
pub async fn start_server(db: Database, port: u16) -> Result<()> {
    // Initialize metrics
    let metrics = Arc::new(Metrics::new()?);

    let state = AppState {
        db: Arc::new(db),
        metrics,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/audit/stats", get(audit_stats))
        .with_state(state);

    // STRICT AIR-GAP ENFORCEMENT: Bind to 127.0.0.1 only
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("L3 Control Plane listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await
        .map_err(SecuraMemError::Io)?;

    axum::serve(listener, app).await
        .map_err(SecuraMemError::Io)?;

    Ok(())
}
