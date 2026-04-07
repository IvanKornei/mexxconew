use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::emulation::{
    browser_cookies::BrowserCookieExtractor,
    config::EmulationConfig,
    persistence::SessionData,
    session::SessionInitializer,
};

/// Shared state for cookie management
pub struct CookieManagementState {
    pub config: EmulationConfig,
    pub current_session: Arc<RwLock<Option<SessionData>>>,
}

/// Cookie information for display
#[derive(Debug, Serialize)]
pub struct CookieInfo {
    pub name: String,
    pub domain: String,
    pub value_preview: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires: Option<String>,
}

/// Session status response
#[derive(Debug, Serialize)]
pub struct SessionStatus {
    pub is_valid: bool,
    pub cookies_count: usize,
    pub expires_at: Option<String>,
    pub time_until_expiry_minutes: Option<i64>,
    pub last_refresh: Option<String>,
    pub source: String,
}

/// Cookie extraction result
#[derive(Debug, Serialize)]
pub struct CookieExtractionResult {
    pub success: bool,
    pub cookies_count: usize,
    pub browser: Option<String>,
    pub message: String,
}

/// Manual cookie input request
#[derive(Debug, Deserialize)]
pub struct ManualCookieRequest {
    pub cookie_string: String,
}

/// Session test result
#[derive(Debug, Serialize)]
pub struct SessionTestResult {
    pub valid: bool,
    pub message: String,
}

/// Create cookie management router
pub fn create_cookie_router(state: Arc<CookieManagementState>) -> Router {
    Router::new()
        .route("/session/status", get(get_session_status))
        .route("/cookies/extract-browser", post(extract_from_browser))
        .route("/cookies/manual", post(submit_manual_cookies))
        .route("/cookies/list", get(get_cookie_list))
        .route("/session/clear", delete(clear_session))
        .route("/session/test", post(test_session))
        .with_state(state)
}

/// Get current session status
async fn get_session_status(
    State(state): State<Arc<CookieManagementState>>,
) -> Result<Json<SessionStatus>, StatusCode> {
    let session_lock = state.current_session.read().await;
    
    if let Some(session) = session_lock.as_ref() {
        let now = chrono::Utc::now();
        let time_until_expiry = session.expires_at.signed_duration_since(now);
        
        Ok(Json(SessionStatus {
            is_valid: session.is_valid(),
            cookies_count: session.cookies.len(),
            expires_at: Some(session.expires_at.to_rfc3339()),
            time_until_expiry_minutes: Some(time_until_expiry.num_minutes()),
            last_refresh: Some(session.created_at.to_rfc3339()),
            source: "cached".to_string(),
        }))
    } else {
        Ok(Json(SessionStatus {
            is_valid: false,
            cookies_count: 0,
            expires_at: None,
            time_until_expiry_minutes: None,
            last_refresh: None,
            source: "none".to_string(),
        }))
    }
}

/// Extract cookies from browser
async fn extract_from_browser(
    State(state): State<Arc<CookieManagementState>>,
) -> Result<Json<CookieExtractionResult>, StatusCode> {
    info!("🍪 Extracting cookies from browser");
    
    // Try to auto-detect browser and extract cookies
    match BrowserCookieExtractor::auto_detect() {
        Ok(extractor) => {
            match extractor.extract_mexc_cookies() {
                Ok(cookies) => {
                    if cookies.is_empty() {
                        return Ok(Json(CookieExtractionResult {
                            success: false,
                            cookies_count: 0,
                            browser: None,
                            message: "No MEXC cookies found. Please log in to MEXC first.".to_string(),
                        }));
                    }
                    
                    // Create session initializer
                    let mut initializer = SessionInitializer::new(state.config.clone())
                        .map_err(|e| {
                            error!("Failed to create session initializer: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                    
                    // Initialize session from browser cookies
                    match initializer.initialize_from_browser_cookies().await {
                        Ok(session_data) => {
                            let cookies_count = session_data.cookies.len();
                            
                            // Update current session
                            let mut session_lock = state.current_session.write().await;
                            *session_lock = Some(session_data);
                            
                            info!("✅ Successfully extracted {} cookies", cookies_count);
                            
                            Ok(Json(CookieExtractionResult {
                                success: true,
                                cookies_count,
                                browser: Some("Auto-detected".to_string()),
                                message: format!("Successfully extracted {} cookies", cookies_count),
                            }))
                        }
                        Err(e) => {
                            error!("Failed to initialize session: {}", e);
                            Ok(Json(CookieExtractionResult {
                                success: false,
                                cookies_count: 0,
                                browser: None,
                                message: format!("Failed to initialize session: {}", e),
                            }))
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to extract cookies: {}", e);
                    Ok(Json(CookieExtractionResult {
                        success: false,
                        cookies_count: 0,
                        browser: None,
                        message: format!("Failed to extract cookies: {}", e),
                    }))
                }
            }
        }
        Err(e) => {
            error!("Failed to detect browser: {}", e);
            Ok(Json(CookieExtractionResult {
                success: false,
                cookies_count: 0,
                browser: None,
                message: format!("No supported browser found: {}", e),
            }))
        }
    }
}

/// Submit manual cookies
async fn submit_manual_cookies(
    State(state): State<Arc<CookieManagementState>>,
    Json(request): Json<ManualCookieRequest>,
) -> Result<Json<CookieExtractionResult>, StatusCode> {
    info!("📝 Submitting manual cookies");
    
    if request.cookie_string.trim().is_empty() {
        return Ok(Json(CookieExtractionResult {
            success: false,
            cookies_count: 0,
            browser: None,
            message: "Cookie string is empty".to_string(),
        }));
    }
    
    // Create session initializer
    let mut initializer = SessionInitializer::new(state.config.clone())
        .map_err(|e| {
            error!("Failed to create session initializer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Initialize session from manual cookies
    match initializer.initialize_from_manual_cookies(&request.cookie_string).await {
        Ok(session_data) => {
            let cookies_count = session_data.cookies.len();
            
            // Update current session
            let mut session_lock = state.current_session.write().await;
            *session_lock = Some(session_data);
            
            info!("✅ Successfully added {} cookies", cookies_count);
            
            Ok(Json(CookieExtractionResult {
                success: true,
                cookies_count,
                browser: Some("Manual".to_string()),
                message: format!("Successfully added {} cookies", cookies_count),
            }))
        }
        Err(e) => {
            error!("Failed to initialize session: {}", e);
            Ok(Json(CookieExtractionResult {
                success: false,
                cookies_count: 0,
                browser: None,
                message: format!("Failed to initialize session: {}", e),
            }))
        }
    }
}

/// Get list of cookies
async fn get_cookie_list(
    State(state): State<Arc<CookieManagementState>>,
) -> Result<Json<Vec<CookieInfo>>, StatusCode> {
    let session_lock = state.current_session.read().await;
    
    if let Some(session) = session_lock.as_ref() {
        let cookie_list: Vec<CookieInfo> = session
            .cookies
            .iter()
            .map(|cookie| CookieInfo {
                name: cookie.name.clone(),
                domain: cookie.domain.clone(),
                value_preview: if cookie.value.len() > 40 {
                    cookie.value[..40].to_string()
                } else {
                    cookie.value.clone()
                },
                secure: cookie.secure,
                http_only: cookie.http_only,
                expires: cookie.expires.map(|dt| dt.to_rfc3339()),
            })
            .collect();
        
        Ok(Json(cookie_list))
    } else {
        Ok(Json(vec![]))
    }
}

/// Clear current session
async fn clear_session(
    State(state): State<Arc<CookieManagementState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("🗑️ Clearing session");
    
    let mut session_lock = state.current_session.write().await;
    *session_lock = None;
    
    Ok(Json(serde_json::json!({ "success": true })))
}

/// Test session validity
async fn test_session(
    State(state): State<Arc<CookieManagementState>>,
) -> Result<Json<SessionTestResult>, StatusCode> {
    let session_lock = state.current_session.read().await;
    
    if let Some(session) = session_lock.as_ref() {
        if session.is_valid() {
            Ok(Json(SessionTestResult {
                valid: true,
                message: "Session is valid and active".to_string(),
            }))
        } else {
            Ok(Json(SessionTestResult {
                valid: false,
                message: "Session has expired".to_string(),
            }))
        }
    } else {
        Ok(Json(SessionTestResult {
            valid: false,
            message: "No active session".to_string(),
        }))
    }
}
