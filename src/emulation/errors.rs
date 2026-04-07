use thiserror::Error;

/// Emulation system error types
#[derive(Debug, Error)]
pub enum EmulationError {
    /// Session initialization failed
    #[error("Session initialization failed: {0}")]
    SessionInitFailed(String),

    /// Browser automation error
    #[error("Browser automation error: {0}")]
    BrowserError(String),

    /// WebSocket connection error
    #[error("WebSocket connection error: {0}")]
    WebSocketError(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Session expired
    #[error("Session expired")]
    SessionExpired,

    /// Session error
    #[error("Session error: {0}")]
    SessionError(String),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    HttpError(String),

    /// Blocked by anti-bot system
    #[error("Blocked by anti-bot system: {0}")]
    BlockedError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Encryption/Decryption error
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    /// Browser cookie extraction error
    #[error("Browser cookie error: {0}")]
    BrowserCookieError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

/// Result type alias for emulation operations
pub type Result<T> = std::result::Result<T, EmulationError>;

impl EmulationError {
    /// Returns true if this error is recoverable with retry
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            EmulationError::WebSocketError(_)
                | EmulationError::RateLimitError(_)
                | EmulationError::SessionInitFailed(_)
        )
    }

    /// Returns true if this error requires session reinitialization
    pub fn requires_session_reinit(&self) -> bool {
        matches!(
            self,
            EmulationError::SessionExpired
                | EmulationError::AuthError(_)
                | EmulationError::BlockedError(_)
        )
    }

    /// Returns true if this error indicates a block/ban
    pub fn is_blocked(&self) -> bool {
        matches!(self, EmulationError::BlockedError(_))
    }
}
