use std::time::Duration;
use tracing::{error, info, warn};

use crate::emulation::{
    errors::{EmulationError, Result},
    mode_manager::ModeManager,
    session::SessionInitializer,
};

/// Recovery handler for error scenarios
///
/// Implements recovery strategies for different error types:
/// - AuthError/SessionExpired: Reinitialize session
/// - RateLimitError: Exponential backoff
/// - BlockedError: Switch mode and reinitialize
pub struct RecoveryHandler {
    max_retries: usize,
}

impl RecoveryHandler {
    /// Create a new recovery handler
    pub fn new() -> Self {
        Self { max_retries: 3 }
    }

    /// Create with custom max retries
    pub fn with_max_retries(max_retries: usize) -> Self {
        Self { max_retries }
    }

    /// Handle authentication or session expiration error
    ///
    /// Strategy: Reinitialize session through SessionInitializer
    pub async fn handle_auth_error(
        &self,
        session_initializer: &mut SessionInitializer,
    ) -> Result<()> {
        warn!("🔐 Handling authentication error - reinitializing session");

        match session_initializer.initialize_session().await {
            Ok(_session_data) => {
                info!("✅ Session reinitialized successfully");
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to reinitialize session: {}", e);
                Err(e)
            }
        }
    }

    /// Handle rate limit error
    ///
    /// Strategy: Exponential backoff with increasing delays
    pub async fn handle_rate_limit_error(&self, attempt: usize) -> Result<()> {
        if attempt >= self.max_retries {
            return Err(EmulationError::RateLimitError(
                "Max retry attempts exceeded".to_string(),
            ));
        }

        // Exponential backoff: 1s, 2s, 4s, 8s
        let delay = Duration::from_secs(2u64.pow(attempt as u32));
        
        warn!(
            "⏳ Rate limit hit - backing off for {:?} (attempt {}/{})",
            delay,
            attempt + 1,
            self.max_retries
        );

        tokio::time::sleep(delay).await;

        Ok(())
    }

    /// Handle blocked error (403, 429, Cloudflare)
    ///
    /// Strategy:
    /// 1. If Aggressive mode: switch to Hybrid
    /// 2. Reinitialize session
    /// 3. Log alert
    pub async fn handle_blocked_error(
        &self,
        mode_manager: &ModeManager,
        session_initializer: &mut SessionInitializer,
        error_code: u16,
    ) -> Result<()> {
        error!("🚫 Blocked by anti-bot system - HTTP {}", error_code);

        // Use mode manager's built-in handler
        mode_manager
            .handle_blocked_error(session_initializer)
            .await?;

        // Log alert for monitoring
        self.log_alert(&format!("Blocked error: HTTP {}", error_code));

        Ok(())
    }

    /// Handle WebSocket disconnection
    ///
    /// Strategy: Reconnect with exponential backoff
    pub async fn handle_websocket_error<F, Fut>(
        &self,
        reconnect_fn: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        warn!("🔌 WebSocket disconnected - attempting reconnection");

        for attempt in 0..self.max_retries {
            let delay = Duration::from_secs(2u64.pow(attempt as u32));
            
            if attempt > 0 {
                warn!("Reconnection attempt {}/{} after {:?}", attempt + 1, self.max_retries, delay);
                tokio::time::sleep(delay).await;
            }

            match reconnect_fn().await {
                Ok(_) => {
                    info!("✅ WebSocket reconnected successfully");
                    return Ok(());
                }
                Err(e) => {
                    warn!("Reconnection attempt {} failed: {}", attempt + 1, e);
                }
            }
        }

        Err(EmulationError::WebSocketError(
            "Failed to reconnect after max retries".to_string(),
        ))
    }

    /// Log alert for monitoring systems
    fn log_alert(&self, message: &str) {
        error!("🚨 ALERT: {}", message);
        // In real implementation: send to monitoring system, Slack, etc.
    }

    /// Determine recovery strategy for an error
    pub fn get_recovery_strategy(&self, error: &EmulationError) -> RecoveryStrategy {
        match error {
            EmulationError::SessionExpired | EmulationError::AuthError(_) => {
                RecoveryStrategy::ReinitializeSession
            }
            EmulationError::RateLimitError(_) => RecoveryStrategy::ExponentialBackoff,
            EmulationError::BlockedError(_) => RecoveryStrategy::SwitchModeAndReinitialize,
            EmulationError::WebSocketError(_) => RecoveryStrategy::ReconnectWithBackoff,
            EmulationError::SessionInitFailed(_) | EmulationError::BrowserError(_) => {
                RecoveryStrategy::RetryWithBackoff
            }
            _ => RecoveryStrategy::None,
        }
    }
}

impl Default for RecoveryHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Recovery strategy for different error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Reinitialize session through SessionInitializer
    ReinitializeSession,
    /// Apply exponential backoff delays
    ExponentialBackoff,
    /// Switch to Hybrid mode and reinitialize
    SwitchModeAndReinitialize,
    /// Reconnect WebSocket with backoff
    ReconnectWithBackoff,
    /// Retry operation with backoff
    RetryWithBackoff,
    /// No recovery strategy available
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_handler_creation() {
        let handler = RecoveryHandler::new();
        assert_eq!(handler.max_retries, 3);

        let handler = RecoveryHandler::with_max_retries(5);
        assert_eq!(handler.max_retries, 5);
    }

    #[tokio::test]
    async fn test_rate_limit_backoff() {
        let handler = RecoveryHandler::new();

        let start = std::time::Instant::now();
        
        // First attempt should succeed immediately
        let result = handler.handle_rate_limit_error(0).await;
        assert!(result.is_ok());
        
        let elapsed = start.elapsed();
        // Should have waited ~1 second
        assert!(elapsed.as_secs() >= 1);
    }

    #[tokio::test]
    async fn test_rate_limit_max_retries() {
        let handler = RecoveryHandler::with_max_retries(2);

        // Should fail after max retries
        let result = handler.handle_rate_limit_error(2).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_recovery_strategy_determination() {
        let handler = RecoveryHandler::new();

        let error = EmulationError::SessionExpired;
        assert_eq!(
            handler.get_recovery_strategy(&error),
            RecoveryStrategy::ReinitializeSession
        );

        let error = EmulationError::RateLimitError("test".to_string());
        assert_eq!(
            handler.get_recovery_strategy(&error),
            RecoveryStrategy::ExponentialBackoff
        );

        let error = EmulationError::BlockedError("test".to_string());
        assert_eq!(
            handler.get_recovery_strategy(&error),
            RecoveryStrategy::SwitchModeAndReinitialize
        );

        let error = EmulationError::WebSocketError("test".to_string());
        assert_eq!(
            handler.get_recovery_strategy(&error),
            RecoveryStrategy::ReconnectWithBackoff
        );
    }

    #[tokio::test]
    async fn test_websocket_reconnection_success() {
        let handler = RecoveryHandler::new();
        let mut attempt_count = 0;

        let reconnect_fn = || async {
            attempt_count += 1;
            if attempt_count < 2 {
                Err(EmulationError::WebSocketError("test".to_string()))
            } else {
                Ok(())
            }
        };

        let result = handler.handle_websocket_error(reconnect_fn).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_websocket_reconnection_failure() {
        let handler = RecoveryHandler::with_max_retries(2);

        let reconnect_fn = || async {
            Err(EmulationError::WebSocketError("persistent error".to_string()))
        };

        let result = handler.handle_websocket_error(reconnect_fn).await;
        assert!(result.is_err());
    }
}
