use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::emulation::{
    browser::BrowserActions,
    config::EmulationConfig,
    persistence::SessionData,
};

/// Shared state for browser control
pub struct BrowserControlState {
    pub config: EmulationConfig,
    pub browser: Arc<RwLock<Option<BrowserActions>>>,
    pub session: Arc<RwLock<Option<SessionData>>>,
    pub start_time: Arc<RwLock<Option<std::time::Instant>>>,
}

/// Browser status response
#[derive(Debug, Serialize)]
pub struct BrowserStatus {
    pub is_running: bool,
    pub is_logged_in: bool,
    pub current_url: String,
    pub session_duration_seconds: u64,
    pub last_activity: String,
    pub mode: String,
    pub stealth_enabled: bool,
    pub human_behavior_enabled: bool,
}

/// Browser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub mode: String, // "visible" or "headless"
    pub window_width: u32,
    pub window_height: u32,
    pub stealth_mode: bool,
    pub human_behavior: bool,
    pub typing_delay_min: u64,
    pub typing_delay_max: u64,
    pub click_delay_min: u64,
    pub click_delay_max: u64,
    pub auto_activity_enabled: bool,
    pub auto_activity_interval_minutes: u64,
    pub screenshot_on_error: bool,
    pub timezone: String,
    pub language: String,
}

/// Trading action request
#[derive(Debug, Deserialize)]
pub struct TradingAction {
    #[serde(rename = "type")]
    pub action_type: String, // "market", "limit", "cancel"
    pub side: Option<String>, // "BUY" or "SELL"
    pub price: Option<f64>,
    pub quantity: Option<f64>,
    pub order_id: Option<String>,
}

/// Position information
#[derive(Debug, Serialize)]
pub struct Position {
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub pnl: f64,
    pub leverage: u32,
}

/// Create browser control router
pub fn create_browser_router(state: Arc<BrowserControlState>) -> Router {
    Router::new()
        .route("/status", get(get_browser_status))
        .route("/start", post(start_browser))
        .route("/stop", post(stop_browser))
        .route("/screenshot", post(take_screenshot))
        .route("/simulate-activity", post(simulate_activity))
        .route("/config", get(get_config).post(save_config))
        .route("/trade", post(execute_trade))
        .route("/balance", get(get_balance))
        .route("/positions", get(get_positions))
        .with_state(state)
}

/// Get browser status
async fn get_browser_status(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<BrowserStatus>, StatusCode> {
    let browser_lock = state.browser.read().await;
    let start_time_lock = state.start_time.read().await;
    
    let is_running = browser_lock.is_some();
    
    let session_duration = if let Some(start) = *start_time_lock {
        start.elapsed().as_secs()
    } else {
        0
    };
    
    Ok(Json(BrowserStatus {
        is_running,
        is_logged_in: is_running, // TODO: Check actual login status
        current_url: if is_running {
            "https://futures.mexc.com/exchange/BTC_USDT".to_string()
        } else {
            "".to_string()
        },
        session_duration_seconds: session_duration,
        last_activity: chrono::Utc::now().to_rfc3339(),
        mode: state.config.browser_profile.to_string(),
        stealth_enabled: true,
        human_behavior_enabled: state.config.jitter.enabled_in_hybrid,
    }))
}

/// Start browser
async fn start_browser(
    State(state): State<Arc<BrowserControlState>>,
    Json(config): Json<BrowserConfig>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("🚀 Starting browser with config: {:?}", config);
    
    // Check if already running
    {
        let browser_lock = state.browser.read().await;
        if browser_lock.is_some() {
            return Err(StatusCode::CONFLICT);
        }
    }
    
    // Get session
    let session_lock = state.session.read().await;
    let session = session_lock.as_ref()
        .ok_or_else(|| {
            error!("No session available");
            StatusCode::BAD_REQUEST
        })?;
    
    // Create browser
    let mut browser = BrowserActions::new(state.config.clone());
    
    // Initialize browser
    browser.initialize(&state.config, session).await
        .map_err(|e| {
            error!("Failed to initialize browser: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Store browser
    {
        let mut browser_lock = state.browser.write().await;
        *browser_lock = Some(browser);
    }
    
    // Store start time
    {
        let mut start_time_lock = state.start_time.write().await;
        *start_time_lock = Some(std::time::Instant::now());
    }
    
    info!("✅ Browser started successfully");
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Browser started successfully"
    })))
}

/// Stop browser
async fn stop_browser(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("⏹️ Stopping browser");
    
    let mut browser_lock = state.browser.write().await;
    
    if let Some(mut browser) = browser_lock.take() {
        browser.close().await
            .map_err(|e| {
                error!("Failed to close browser: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        // Clear start time
        let mut start_time_lock = state.start_time.write().await;
        *start_time_lock = None;
        
        info!("✅ Browser stopped");
        
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Browser stopped successfully"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Take screenshot
async fn take_screenshot(
    State(_state): State<Arc<BrowserControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Implement screenshot functionality
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// Simulate human activity
async fn simulate_activity(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut browser_lock = state.browser.write().await;
    
    if let Some(browser) = browser_lock.as_mut() {
        browser.simulate_human_activity().await
            .map_err(|e| {
                error!("Failed to simulate activity: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        Ok(Json(serde_json::json!({
            "success": true
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get configuration
async fn get_config(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<BrowserConfig>, StatusCode> {
    let config = BrowserConfig {
        mode: "headless".to_string(),
        window_width: 1920,
        window_height: 1080,
        stealth_mode: true,
        human_behavior: state.config.jitter.enabled_in_hybrid,
        typing_delay_min: state.config.behavior.typing_delay_range_ms[0],
        typing_delay_max: state.config.behavior.typing_delay_range_ms[1],
        click_delay_min: state.config.jitter.min_delay_ms,
        click_delay_max: state.config.jitter.max_delay_ms,
        auto_activity_enabled: true,
        auto_activity_interval_minutes: state.config.background_actions_interval_secs() / 60,
        screenshot_on_error: true,
        timezone: "Europe/Moscow".to_string(),
        language: "ru-RU".to_string(),
    };
    
    Ok(Json(config))
}

/// Save configuration
async fn save_config(
    State(_state): State<Arc<BrowserControlState>>,
    Json(config): Json<BrowserConfig>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("💾 Saving config: {:?}", config);
    
    // TODO: Persist config to file
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Configuration saved"
    })))
}

/// Execute trade
async fn execute_trade(
    State(state): State<Arc<BrowserControlState>>,
    Json(action): Json<TradingAction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("📊 Executing trade: {:?}", action);
    
    let mut browser_lock = state.browser.write().await;
    
    if let Some(browser) = browser_lock.as_mut() {
        match action.action_type.as_str() {
            "market" => {
                let side = action.side.as_deref().unwrap_or("BUY");
                let quantity = action.quantity.unwrap_or(0.001);
                
                let order_id = browser.place_market_order(side, quantity).await
                    .map_err(|e| {
                        error!("Failed to place market order: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                
                Ok(Json(serde_json::json!({
                    "success": true,
                    "order_id": order_id
                })))
            }
            "limit" => {
                let side = action.side.as_deref().unwrap_or("BUY");
                let price = action.price.unwrap_or(50000.0);
                let quantity = action.quantity.unwrap_or(0.001);
                
                let order_id = browser.place_limit_order(side, price, quantity).await
                    .map_err(|e| {
                        error!("Failed to place limit order: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                
                Ok(Json(serde_json::json!({
                    "success": true,
                    "order_id": order_id
                })))
            }
            "cancel" => {
                let order_id = action.order_id.as_deref().unwrap_or("");
                
                browser.cancel_order(order_id).await
                    .map_err(|e| {
                        error!("Failed to cancel order: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                
                Ok(Json(serde_json::json!({
                    "success": true
                })))
            }
            _ => Err(StatusCode::BAD_REQUEST),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get balance
async fn get_balance(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut browser_lock = state.browser.write().await;
    
    if let Some(browser) = browser_lock.as_mut() {
        let balance = browser.get_balance().await
            .map_err(|e| {
                error!("Failed to get balance: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        Ok(Json(serde_json::json!({
            "balance": balance
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get positions
async fn get_positions(
    State(state): State<Arc<BrowserControlState>>,
) -> Result<Json<Vec<Position>>, StatusCode> {
    let mut browser_lock = state.browser.write().await;
    
    if let Some(browser) = browser_lock.as_mut() {
        let positions = browser.get_open_positions().await
            .map_err(|e| {
                error!("Failed to get positions: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        
        let positions: Vec<Position> = positions
            .into_iter()
            .map(|p| Position {
                symbol: p.symbol,
                side: p.side,
                size: p.size,
                entry_price: p.entry_price,
                pnl: p.pnl,
                leverage: 200, // TODO: Get actual leverage
            })
            .collect();
        
        Ok(Json(positions))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
