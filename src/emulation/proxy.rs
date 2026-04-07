use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::emulation::{
    config::ProxyConfig,
    errors::{EmulationError, Result},
};

/// Proxy manager for residential proxy support
///
/// Handles:
/// - Sticky session management
/// - Rate limiting for home IP
/// - Proxy rotation (if needed)
pub struct ProxyManager {
    config: ProxyConfig,
    current_session_id: Option<String>,
    session_start: Option<Instant>,
    request_count: u64,
    last_request_time: Instant,
}

impl ProxyManager {
    /// Create a new proxy manager
    pub fn new(config: ProxyConfig) -> Self {
        Self {
            config,
            current_session_id: None,
            session_start: None,
            request_count: 0,
            last_request_time: Instant::now(),
        }
    }

    /// Check if proxy is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get proxy URL if configured
    pub fn proxy_url(&self) -> Option<String> {
        if !self.config.enabled {
            return None;
        }

        self.config.provider.as_ref().map(|provider| {
            // In real implementation: construct actual proxy URL
            // For now, return placeholder
            format!("http://{}:8080", provider)
        })
    }

    /// Initialize a sticky session (if enabled)
    pub async fn initialize_sticky_session(&mut self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if self.config.sticky_session.unwrap_or(false) {
            let session_id = format!("session_{}", chrono::Utc::now().timestamp());
            info!("🔗 Initializing sticky proxy session: {}", session_id);
            
            self.current_session_id = Some(session_id);
            self.session_start = Some(Instant::now());
        }

        Ok(())
    }

    /// Get current session ID for sticky session
    pub fn session_id(&self) -> Option<&str> {
        self.current_session_id.as_deref()
    }

    /// Check if rate limit allows request (for home IP)
    ///
    /// Limits to 10 requests per hour when not using proxy
    pub fn check_rate_limit(&mut self) -> Result<()> {
        if self.config.enabled {
            // No rate limit when using proxy
            return Ok(());
        }

        // Home IP: limit to 10 requests per hour
        let elapsed = self.last_request_time.elapsed();
        
        if elapsed < Duration::from_secs(3600) {
            // Within the same hour
            if self.request_count >= 10 {
                return Err(EmulationError::RateLimitError(
                    "Home IP rate limit exceeded (10 requests/hour)".to_string(),
                ));
            }
        } else {
            // New hour, reset counter
            self.request_count = 0;
            self.last_request_time = Instant::now();
        }

        self.request_count += 1;
        debug!(
            "Home IP request count: {}/10 this hour",
            self.request_count
        );

        Ok(())
    }

    /// Record a request (for rate limiting)
    pub fn record_request(&mut self) {
        if !self.config.enabled {
            self.request_count += 1;
        }
    }

    /// Get session duration
    pub fn session_duration(&self) -> Option<Duration> {
        self.session_start.map(|start| start.elapsed())
    }

    /// Check if session should be rotated
    ///
    /// Rotate after 24 hours for sticky sessions
    pub fn should_rotate_session(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        if let Some(start) = self.session_start {
            let duration = start.elapsed();
            // Rotate after 24 hours
            duration > Duration::from_secs(24 * 3600)
        } else {
            false
        }
    }

    /// Rotate to a new sticky session
    pub async fn rotate_session(&mut self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        warn!("🔄 Rotating proxy session");
        
        let new_session_id = format!("session_{}", chrono::Utc::now().timestamp());
        info!("🔗 New sticky proxy session: {}", new_session_id);
        
        self.current_session_id = Some(new_session_id);
        self.session_start = Some(Instant::now());

        Ok(())
    }

    /// Get request count for current period
    pub fn request_count(&self) -> u64 {
        self.request_count
    }

    /// Reset request count
    pub fn reset_request_count(&mut self) {
        self.request_count = 0;
        self.last_request_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_disabled_config() -> ProxyConfig {
        ProxyConfig {
            enabled: false,
            provider: None,
            sticky_session: None,
        }
    }

    fn create_enabled_config() -> ProxyConfig {
        ProxyConfig {
            enabled: true,
            provider: Some("brightdata".to_string()),
            sticky_session: Some(true),
        }
    }

    #[test]
    fn test_proxy_manager_disabled() {
        let config = create_disabled_config();
        let manager = ProxyManager::new(config);

        assert!(!manager.is_enabled());
        assert!(manager.proxy_url().is_none());
    }

    #[test]
    fn test_proxy_manager_enabled() {
        let config = create_enabled_config();
        let manager = ProxyManager::new(config);

        assert!(manager.is_enabled());
        assert!(manager.proxy_url().is_some());
    }

    #[tokio::test]
    async fn test_sticky_session_initialization() {
        let config = create_enabled_config();
        let mut manager = ProxyManager::new(config);

        assert!(manager.session_id().is_none());

        manager.initialize_sticky_session().await.unwrap();

        assert!(manager.session_id().is_some());
        assert!(manager.session_duration().is_some());
    }

    #[test]
    fn test_rate_limiting_home_ip() {
        let config = create_disabled_config();
        let mut manager = ProxyManager::new(config);

        // Should allow first 10 requests
        for i in 0..10 {
            let result = manager.check_rate_limit();
            assert!(result.is_ok(), "Request {} should be allowed", i + 1);
        }

        // 11th request should fail
        let result = manager.check_rate_limit();
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_limiting_with_proxy() {
        let config = create_enabled_config();
        let mut manager = ProxyManager::new(config);

        // Should allow unlimited requests with proxy
        for _ in 0..100 {
            let result = manager.check_rate_limit();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_request_counting() {
        let config = create_disabled_config();
        let mut manager = ProxyManager::new(config);

        assert_eq!(manager.request_count(), 0);

        manager.record_request();
        assert_eq!(manager.request_count(), 1);

        manager.record_request();
        manager.record_request();
        assert_eq!(manager.request_count(), 3);

        manager.reset_request_count();
        assert_eq!(manager.request_count(), 0);
    }

    #[tokio::test]
    async fn test_session_rotation() {
        let config = create_enabled_config();
        let mut manager = ProxyManager::new(config);

        manager.initialize_sticky_session().await.unwrap();
        let first_session = manager.session_id().unwrap().to_string();

        // Initially should not need rotation
        assert!(!manager.should_rotate_session());

        // Rotate session
        manager.rotate_session().await.unwrap();
        let second_session = manager.session_id().unwrap().to_string();

        // Session ID should have changed
        assert_ne!(first_session, second_session);
    }

    #[test]
    fn test_session_duration_tracking() {
        let config = create_enabled_config();
        let mut manager = ProxyManager::new(config);

        assert!(manager.session_duration().is_none());

        manager.session_start = Some(Instant::now());
        
        std::thread::sleep(std::time::Duration::from_millis(100));

        let duration = manager.session_duration().unwrap();
        assert!(duration.as_millis() >= 100);
    }
}
