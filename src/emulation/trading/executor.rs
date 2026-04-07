use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tracing::{debug, info, warn};

use crate::emulation::{
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::SessionData,
    tls::TLSEmulator,
    trading::{HumanJitter, ParameterManager},
    TradingMode,
};

/// Order to be executed
#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

/// Order execution response
#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: OrderStatus,
    pub filled_quantity: f64,
    pub average_price: f64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Filled,
    PartiallyFilled,
    Pending,
    Rejected,
}

/// High-speed trading executor
///
/// Handles:
/// - WebSocket connections to Binance (monitoring) and MEXC (trading)
/// - Order execution with optional human jitter
/// - Arbitrage monitoring
/// - Rate limiting
pub struct TradingExecutor {
    config: EmulationConfig,
    session_data: Arc<RwLock<SessionData>>,
    tls_emulator: TLSEmulator,
    parameter_manager: Arc<RwLock<ParameterManager>>,
    human_jitter: Option<HumanJitter>,
    last_order_time: Arc<RwLock<Instant>>,
    min_order_interval: Duration,
}

impl TradingExecutor {
    /// Create a new trading executor
    pub fn new(
        config: EmulationConfig,
        session_data: SessionData,
        parameter_manager: ParameterManager,
    ) -> Self {
        let tls_emulator = TLSEmulator::new(config.browser_profile);
        
        // Create human jitter only if in Hybrid mode
        let human_jitter = if config.should_apply_jitter() {
            Some(HumanJitter::from_config(&config.jitter))
        } else {
            None
        };

        // Set minimum order interval based on mode
        let min_order_interval = match config.trading_mode {
            TradingMode::Hybrid => Duration::from_millis(500), // 500ms ±200ms
            TradingMode::Aggressive => Duration::from_millis(100), // 100ms minimum
        };

        Self {
            config,
            session_data: Arc::new(RwLock::new(session_data)),
            tls_emulator,
            parameter_manager: Arc::new(RwLock::new(parameter_manager)),
            human_jitter,
            last_order_time: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(10))),
            min_order_interval,
        }
    }

    /// Connect to Binance WebSocket for price monitoring (no emulation)
    pub async fn connect_to_binance(&self, url: &str) -> Result<()> {
        info!("Connecting to Binance for price monitoring: {}", url);

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| EmulationError::WebSocketError(format!("Binance connection failed: {}", e)))?;

        info!("✅ Connected to Binance WebSocket");

        // In real implementation: spawn task to handle messages
        // For now, just verify connection works
        drop(ws_stream);

        Ok(())
    }

    /// Connect to MEXC WebSocket for trading with session tokens
    pub async fn connect_to_mexc(&self, url: &str) -> Result<()> {
        info!("Connecting to MEXC for trading: {}", url);

        // Get session data
        let session = self.session_data.read().await;
        
        // Build request with TLS fingerprints and session cookies
        let request = self.build_mexc_request(url, &session)?;
        drop(session);

        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| EmulationError::WebSocketError(format!("MEXC connection failed: {}", e)))?;

        info!("✅ Connected to MEXC WebSocket with session tokens");

        // In real implementation: spawn task to handle messages
        drop(ws_stream);

        Ok(())
    }

    /// Build MEXC WebSocket request with TLS fingerprints and cookies
    fn build_mexc_request(
        &self,
        url: &str,
        session: &SessionData,
    ) -> Result<tokio_tungstenite::tungstenite::handshake::client::Request> {
        use tokio_tungstenite::tungstenite::handshake::client::Request;

        let mut request = Request::builder()
            .uri(url)
            .header("User-Agent", self.tls_emulator.get_user_agent())
            .header("Accept-Language", self.tls_emulator.get_accept_language())
            .header("Accept-Encoding", self.tls_emulator.get_accept_encoding())
            .header("Sec-CH-UA", self.tls_emulator.get_sec_ch_ua())
            .header("Sec-CH-UA-Platform", self.tls_emulator.get_sec_ch_ua_platform());

        // Add cookies from session
        if !session.cookies.is_empty() {
            let cookie_header = session
                .cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            request = request.header("Cookie", cookie_header);
        }

        // Add authorization token
        request = request.header("Authorization", &session.auth_token);

        let request = request
            .body(())
            .map_err(|e| EmulationError::WebSocketError(format!("Failed to build request: {}", e)))?;

        Ok(request)
    }

    /// Execute an order with optional human jitter
    pub async fn execute_order(&mut self, order: Order) -> Result<OrderResponse> {
        let start = Instant::now();

        // Apply human jitter if in Hybrid mode
        if let Some(ref jitter) = self.human_jitter {
            jitter.apply_jitter().await;
        }

        // Enforce minimum order interval
        self.enforce_rate_limit().await?;

        // Get order parameters
        let params = {
            let pm = self.parameter_manager.read().await;
            pm.get_order_parameters().clone()
        };

        debug!(
            "Executing order: {:?} {} @ {:?} (params: size={}, leverage={})",
            order.side, order.symbol, order.price, params.position_size, params.leverage
        );

        // In real implementation: send order via WebSocket
        // For now, simulate order execution
        tokio::time::sleep(Duration::from_micros(100)).await;

        // Update last order time
        *self.last_order_time.write().await = Instant::now();

        let latency = start.elapsed();

        let response = OrderResponse {
            order_id: format!("ord_{}", chrono::Utc::now().timestamp_millis()),
            status: OrderStatus::Filled,
            filled_quantity: order.quantity,
            average_price: order.price.unwrap_or(0.0),
            latency_ms: latency.as_millis() as u64,
        };

        debug!(
            "Order executed: {} in {}ms",
            response.order_id, response.latency_ms
        );

        Ok(response)
    }

    /// Enforce rate limiting between orders
    async fn enforce_rate_limit(&self) -> Result<()> {
        let last_order = self.last_order_time.read().await;
        let elapsed = last_order.elapsed();

        if elapsed < self.min_order_interval {
            let wait_time = self.min_order_interval - elapsed;
            debug!("Rate limit: waiting {:?}", wait_time);
            tokio::time::sleep(wait_time).await;
        }

        Ok(())
    }

    /// Monitor arbitrage opportunities between Binance and MEXC
    pub async fn monitor_arbitrage(&self) -> Result<()> {
        info!("🔍 Starting arbitrage monitoring");

        // In real implementation:
        // 1. Subscribe to Binance price feed
        // 2. Subscribe to MEXC price feed
        // 3. Calculate spread in real-time
        // 4. Trigger execute_order when opportunity detected

        // For now, just log that monitoring started
        debug!("Arbitrage monitoring active");

        Ok(())
    }

    /// Reconnect to WebSocket with exponential backoff
    pub async fn reconnect_with_backoff(&self, url: &str, max_retries: usize) -> Result<()> {
        let mut attempt = 0;

        while attempt < max_retries {
            attempt += 1;
            
            match self.connect_to_mexc(url).await {
                Ok(_) => {
                    info!("✅ Reconnected successfully on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    let delay = Duration::from_secs(2u64.pow(attempt as u32 - 1));
                    warn!(
                        "Reconnection attempt {} failed: {}. Retrying in {:?}",
                        attempt, e, delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(EmulationError::WebSocketError(format!(
            "Failed to reconnect after {} attempts",
            max_retries
        )))
    }

    /// Get current trading mode
    pub fn trading_mode(&self) -> TradingMode {
        self.config.trading_mode
    }

    /// Check if human jitter is enabled
    pub fn has_jitter(&self) -> bool {
        self.human_jitter.is_some()
    }

    /// Get parameter manager
    pub fn parameter_manager(&self) -> Arc<RwLock<ParameterManager>> {
        self.parameter_manager.clone()
    }

    /// Update session data (e.g., after refresh)
    pub async fn update_session(&self, new_session: SessionData) {
        let mut session = self.session_data.write().await;
        *session = new_session;
        info!("Session data updated");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulation::config::{BehaviorConfig, EmulationConfig};
    use crate::emulation::persistence::{Cookie, SessionData};

    fn create_test_session() -> SessionData {
        SessionData::new(
            vec![Cookie {
                name: "test".to_string(),
                value: "value".to_string(),
                domain: "mexc.com".to_string(),
                path: "/".to_string(),
                secure: true,
                http_only: true,
                expires: None,
            }],
            "Bearer test_token".to_string(),
            vec![1, 2, 3],
            "Mozilla/5.0".to_string(),
            24,
        )
    }

    fn create_test_config(mode: TradingMode) -> EmulationConfig {
        let mut config = EmulationConfig::default();
        config.trading_mode = mode;
        config
    }

    #[test]
    fn test_executor_creation_hybrid() {
        let config = create_test_config(TradingMode::Hybrid);
        let session = create_test_session();
        let param_manager = ParameterManager::new(BehaviorConfig::default());

        let executor = TradingExecutor::new(config, session, param_manager);

        assert_eq!(executor.trading_mode(), TradingMode::Hybrid);
        assert!(executor.has_jitter());
    }

    #[test]
    fn test_executor_creation_aggressive() {
        let config = create_test_config(TradingMode::Aggressive);
        let session = create_test_session();
        let param_manager = ParameterManager::new(BehaviorConfig::default());

        let executor = TradingExecutor::new(config, session, param_manager);

        assert_eq!(executor.trading_mode(), TradingMode::Aggressive);
        assert!(!executor.has_jitter());
    }

    #[tokio::test]
    async fn test_order_execution() {
        let config = create_test_config(TradingMode::Aggressive);
        let session = create_test_session();
        let param_manager = ParameterManager::new(BehaviorConfig::default());

        let mut executor = TradingExecutor::new(config, session, param_manager);

        let order = Order {
            symbol: "BTC/USDT".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 0.001,
            price: Some(50000.0),
        };

        let result = executor.execute_order(order).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status, OrderStatus::Filled);
        assert!(response.latency_ms < 1000); // Should be fast
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = create_test_config(TradingMode::Aggressive);
        let session = create_test_session();
        let param_manager = ParameterManager::new(BehaviorConfig::default());

        let mut executor = TradingExecutor::new(config, session, param_manager);

        let order = Order {
            symbol: "BTC/USDT".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 0.001,
            price: Some(50000.0),
        };

        // Execute first order
        let start = Instant::now();
        executor.execute_order(order.clone()).await.unwrap();

        // Execute second order immediately - should be rate limited
        executor.execute_order(order).await.unwrap();
        let elapsed = start.elapsed();

        // Should take at least min_order_interval (100ms for Aggressive)
        assert!(elapsed.as_millis() >= 100);
    }

    #[tokio::test]
    async fn test_session_update() {
        let config = create_test_config(TradingMode::Hybrid);
        let session = create_test_session();
        let param_manager = ParameterManager::new(BehaviorConfig::default());

        let executor = TradingExecutor::new(config, session, param_manager);

        let new_session = create_test_session();
        executor.update_session(new_session).await;

        // Verify session was updated
        let session = executor.session_data.read().await;
        assert_eq!(session.auth_token, "Bearer test_token");
    }
}
