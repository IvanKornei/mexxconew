/// Trading control API
/// 
/// REST API for controlling the trading engine

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

// Временные заглушки для компиляции
// TODO: Заменить на реальные импорты из crate::trading::

#[derive(Debug, Clone)]
pub struct TradingEngine;

#[derive(Debug, Clone, Default)]
pub struct TradingStats {
    pub total_signals: u64,
    pub signals_executed: u64,
    pub signals_skipped: u64,
    pub total_profit: f64,
    pub total_loss: f64,
    pub win_rate: f64,
    pub last_signal_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    DryRun,
    Paper,
    Live,
}

#[derive(Debug, Clone)]
pub struct ArbitragePosition {
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub entry_spread: f64,
    pub position_size: f64,
    pub entry_time: chrono::DateTime<chrono::Utc>,
}

impl TradingEngine {
    pub async fn is_running(&self) -> bool {
        false
    }
    
    pub async fn get_stats(&self) -> TradingStats {
        TradingStats::default()
    }
    
    pub async fn get_current_position(&self) -> Option<ArbitragePosition> {
        None
    }
    
    pub async fn start(&self) {
        // No-op
    }
    
    pub async fn stop(&self) {
        // No-op
    }
    
    pub fn set_execution_mode(&mut self, _mode: ExecutionMode) {
        // No-op
    }
    
    pub fn set_auto_trading(&mut self, _enabled: bool) {
        // No-op
    }
}

/// Shared state for trading control
pub struct TradingControlState {
    pub engine: Arc<RwLock<TradingEngine>>,
    pub position_manager: Arc<RwLock<crate::core::PositionManager>>,
}

/// Trading status response
#[derive(Debug, Serialize)]
pub struct TradingStatus {
    pub is_running: bool,
    pub mode: String,
    pub auto_trading_enabled: bool,
    pub current_position: Option<PositionInfo>,
    pub stats: TradingStatsResponse,
}

/// Position information
#[derive(Debug, Serialize)]
pub struct PositionInfo {
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub entry_spread: f64,
    pub position_size: f64,
    pub duration_minutes: i64,
}

/// Trading statistics response
#[derive(Debug, Serialize)]
pub struct TradingStatsResponse {
    pub total_signals: u64,
    pub signals_executed: u64,
    pub signals_skipped: u64,
    pub total_profit: f64,
    pub total_loss: f64,
    pub win_rate: f64,
    pub last_signal_time: Option<String>,
}

/// Engine configuration request
#[derive(Debug, Deserialize)]
pub struct EngineConfigRequest {
    pub mode: String, // "dry_run", "paper", "live"
    pub auto_trading_enabled: bool,
    pub min_spread_open: f64,
    pub spread_close: f64,
    pub max_position_size: f64,
    pub capital_per_trade: f64,
}

/// Create trading control router
pub fn create_trading_router(state: Arc<TradingControlState>) -> Router {
    Router::new()
        .route("/status", get(get_trading_status))
        .route("/start", post(start_trading))
        .route("/stop", post(stop_trading))
        .route("/config", get(get_trading_config).post(update_config))
        .route("/stats", get(get_stats))
        .with_state(state)
}

/// Get trading status
async fn get_trading_status(
    State(state): State<Arc<TradingControlState>>,
) -> Result<Json<TradingStatus>, StatusCode> {
    let engine: tokio::sync::RwLockReadGuard<'_, TradingEngine> = state.engine.read().await;
    let position_manager = state.position_manager.read().await;
    
    let is_running = engine.is_running().await;
    let stats: TradingStats = engine.get_stats().await;
    let position: Option<ArbitragePosition> = engine.get_current_position().await;
    
    let position_info = position.map(|p: ArbitragePosition| {
        let now = Utc::now();
        // Calculate duration in seconds manually using timestamp()
        let duration_seconds = (now.timestamp() - p.entry_time.timestamp()) as i64;
        PositionInfo {
            buy_exchange: p.buy_exchange,
            sell_exchange: p.sell_exchange,
            entry_spread: p.entry_spread,
            position_size: p.position_size,
            duration_minutes: duration_seconds / 60,
        }
    });
    
    let execution_mode = position_manager.get_execution_mode().await;
    let is_enabled = position_manager.is_trading_enabled().await;
    
    let mode_str = match execution_mode {
        crate::core::TradingMode::Emulation => "Emulation",
        crate::core::TradingMode::Live => "Live",
    };
    
    Ok(Json(TradingStatus {
        is_running,
        mode: mode_str.to_string(),
        auto_trading_enabled: is_enabled,
        current_position: position_info,
        stats: TradingStatsResponse {
            total_signals: stats.total_signals,
            signals_executed: stats.signals_executed,
            signals_skipped: stats.signals_skipped,
            total_profit: stats.total_profit,
            total_loss: stats.total_loss,
            win_rate: stats.win_rate,
            last_signal_time: stats.last_signal_time.map(|t| t.to_rfc3339()),
        },
    }))
}

/// Start trading engine
async fn start_trading(
    State(state): State<Arc<TradingControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("🚀 Starting trading engine via API");
    
    let engine: tokio::sync::RwLockReadGuard<'_, TradingEngine> = state.engine.read().await;
    
    if engine.is_running().await {
        return Err(StatusCode::CONFLICT);
    }
    
    engine.start().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Trading engine started"
    })))
}

/// Stop trading engine
async fn stop_trading(
    State(state): State<Arc<TradingControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("⏹️ Stopping trading engine via API");
    
    let engine: tokio::sync::RwLockReadGuard<'_, TradingEngine> = state.engine.read().await;
    
    if !engine.is_running().await {
        return Err(StatusCode::NOT_FOUND);
    }
    
    engine.stop().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Trading engine stopped"
    })))
}

/// Update configuration
async fn update_config(
    State(state): State<Arc<TradingControlState>>,
    Json(config): Json<EngineConfigRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("💾 Updating trading config: {:?}", config);
    
    let mut engine: tokio::sync::RwLockWriteGuard<'_, TradingEngine> = state.engine.write().await;
    
    // Parse execution mode
    let mode = match config.mode.as_str() {
        "dry_run" => ExecutionMode::DryRun,
        "paper" => ExecutionMode::Paper,
        "live" => ExecutionMode::Live,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    engine.set_execution_mode(mode);
    engine.set_auto_trading(config.auto_trading_enabled);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Configuration updated"
    })))
}

/// Get trading configuration
async fn get_trading_config(
    State(state): State<Arc<TradingControlState>>,
) -> Result<Json<TradingConfigResponse>, StatusCode> {
    let _engine = state.engine.read().await;
    let position_manager = state.position_manager.read().await;
    
    let execution_mode = position_manager.get_execution_mode().await;
    let is_enabled = position_manager.is_trading_enabled().await;
    
    let mode_str = match execution_mode {
        crate::core::TradingMode::Emulation => "emulation",
        crate::core::TradingMode::Live => "live",
    };
    
    Ok(Json(TradingConfigResponse {
        mode: mode_str.to_string(),
        auto_trading_enabled: is_enabled,
        min_spread_percent: 0.3,
        position_size_percent: 10.0,
        leverage: 200,
        max_positions: 2,
    }))
}

#[derive(Debug, Serialize)]
struct TradingConfigResponse {
    mode: String,
    auto_trading_enabled: bool,
    min_spread_percent: f64,
    position_size_percent: f64,
    leverage: u32,
    max_positions: usize,
}


/// Get statistics
async fn get_stats(
    State(state): State<Arc<TradingControlState>>,
) -> Result<Json<TradingStatsResponse>, StatusCode> {
    let engine: tokio::sync::RwLockReadGuard<'_, TradingEngine> = state.engine.read().await;
    let stats: TradingStats = engine.get_stats().await;
    
    Ok(Json(TradingStatsResponse {
        total_signals: stats.total_signals,
        signals_executed: stats.signals_executed,
        signals_skipped: stats.signals_skipped,
        total_profit: stats.total_profit,
        total_loss: stats.total_loss,
        win_rate: stats.win_rate,
        last_signal_time: stats.last_signal_time.map(|t| t.to_rfc3339()),
    }))
}