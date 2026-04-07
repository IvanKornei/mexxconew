use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Emulation status data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationStatus {
    pub trading_mode: String,
    pub session_valid: bool,
    pub session_created_at: Option<String>,
    pub session_expires_at: Option<String>,
    pub background_actions_count: u64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub last_activity: Option<String>,
}

impl Default for EmulationStatus {
    fn default() -> Self {
        Self {
            trading_mode: "Hybrid".to_string(),
            session_valid: false,
            session_created_at: None,
            session_expires_at: None,
            background_actions_count: 0,
            latency_p50_ms: 0.0,
            latency_p95_ms: 0.0,
            latency_p99_ms: 0.0,
            last_activity: None,
        }
    }
}

/// Shared emulation status state
pub type EmulationStatusState = Arc<RwLock<EmulationStatus>>;

/// Get emulation status endpoint
pub async fn get_emulation_status(
    State(status): State<EmulationStatusState>,
) -> impl IntoResponse {
    let status = status.read().await;
    Json(status.clone())
}

/// Update emulation status (internal use)
pub async fn update_emulation_status(
    state: &EmulationStatusState,
    update: EmulationStatus,
) {
    let mut status = state.write().await;
    *status = update;
}
