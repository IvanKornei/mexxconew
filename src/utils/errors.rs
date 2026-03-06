use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Failed to connect to {exchange}: {reason}")]
    ConnectFailed { exchange: String, reason: String },
    
    #[error("Connection lost to {exchange}")]
    Disconnected { exchange: String },
    
    #[error("Reconnection failed after {attempts} attempts")]
    ReconnectFailed { attempts: u32 },
    
    #[error("Operation '{operation}' timed out after {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),
    
    #[error("Invalid API credentials")]
    InvalidCredentials,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Exchange error: {0}")]
    ExchangeError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, Error>;
