use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::exchanges::client::{
    Balance, ClientResult, ExchangeClient, Order, OrderRequest, OrderSide, OrderStatus, OrderType,
};
use crate::utils::ApiError;

use crate::emulation::{
    behavior::PeriodicActivitySimulator,
    config::EmulationConfig,
    mode_manager::ModeManager,
    persistence::SessionPersistence,
    recovery::RecoveryHandler,
    session::SessionInitializer,
    trading::{TradingExecutor, ParameterManager},
};

/// MEXC emulator that implements ExchangeClient with browser emulation
///
/// Integrates all emulation components:
/// - SessionInitializer for session setup
/// - TradingExecutor for order execution
/// - PeriodicActivitySimulator for background activity
/// - ModeManager for Hybrid/Aggressive switching
/// - RecoveryHandler for error handling
pub struct MEXCEmulator {
    config: EmulationConfig,
    session_initializer: Arc<RwLock<SessionInitializer>>,
    trading_executor: Arc<RwLock<Option<TradingExecutor>>>,
    mode_manager: Arc<ModeManager>,
    recovery_handler: Arc<RecoveryHandler>,
    periodic_activity_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl MEXCEmulator {
    /// Create a new MEXC emulator
    ///
    /// This will:
    /// 1. Check for existing valid session
    /// 2. Initialize new session if needed
    /// 3. Start periodic activity simulator in background
    pub async fn new(config: EmulationConfig) -> Result<Self, ApiError> {
        info!("🎭 Initializing MEXC Emulator");

        // Create session initializer
        let session_initializer = SessionInitializer::new(config.clone())
            .map_err(|e| ApiError::Connection(format!("Failed to create session initializer: {}", e)))?;
        
        let session_initializer = Arc::new(RwLock::new(session_initializer));

        // Check for existing session
        let session_data = {
            let init = session_initializer.read().await;
            let persistence: &SessionPersistence = init.persistence();
            
            match persistence.load_session() {
                Ok(Some(session)) if session.is_valid() => {
                    info!("✅ Found valid existing session");
                    Some(session)
                }
                _ => {
                    info!("No valid session found, will initialize new one");
                    None
                }
            }
        };

        // Initialize session if needed
        let session_data = if let Some(session) = session_data {
            session
        } else {
            let mut init: tokio::sync::RwLockWriteGuard<SessionInitializer> = session_initializer.write().await;
            init.initialize_session()
                .await
                .map_err(|e| ApiError::Connection(format!("Session initialization failed: {}", e)))?
        };

        // Create trading executor
        let parameter_manager = ParameterManager::new(config.behavior.clone());
        let trading_executor = TradingExecutor::new(
            config.clone(),
            session_data,
            parameter_manager,
        );

        // Create mode manager
        let mode_manager = Arc::new(ModeManager::new(config.clone()));

        // Create recovery handler
        let recovery_handler = Arc::new(RecoveryHandler::new());

        let emulator = Self {
            config: config.clone(),
            session_initializer,
            trading_executor: Arc::new(RwLock::new(Some(trading_executor))),
            mode_manager,
            recovery_handler,
            periodic_activity_handle: Arc::new(RwLock::new(None)),
        };

        // Start periodic activity simulator in background
        emulator.start_periodic_activity().await;

        info!("✅ MEXC Emulator initialized successfully");

        Ok(emulator)
    }

    /// Start periodic activity simulator in background task
    async fn start_periodic_activity(&self) {
        let mut simulator = PeriodicActivitySimulator::new(
            self.config.behavior.clone(),
            self.config.periodic_activity.clone(),
            self.config.trading_mode,
        );

        let handle = tokio::spawn(async move {
            if let Err(e) = simulator.run_background_loop().await {
                warn!("Periodic activity simulator error: {}", e);
            }
        });

        *self.periodic_activity_handle.write().await = Some(handle);
        info!("🎬 Periodic activity simulator started in background");
    }

    /// Handle error with recovery strategy
    async fn handle_error(&self, error: &crate::emulation::errors::EmulationError) -> ClientResult<()> {
        let strategy = self.recovery_handler.get_recovery_strategy(error);
        
        match strategy {
            crate::emulation::recovery::RecoveryStrategy::ReinitializeSession => {
                let mut init = self.session_initializer.write().await;
                self.recovery_handler
                    .handle_auth_error(&mut *init)
                    .await
                    .map_err(|e| ApiError::Authentication(e.to_string()))?;
                
                // Update trading executor with new session
                if let Some(new_session) = init.persistence().load_session().unwrap() {
                    if let Some(executor) = self.trading_executor.read().await.as_ref() {
                        executor.update_session(new_session).await;
                    }
                }
            }
            crate::emulation::recovery::RecoveryStrategy::SwitchModeAndReinitialize => {
                let mut init = self.session_initializer.write().await;
                self.recovery_handler
                    .handle_blocked_error(&self.mode_manager, &mut *init, 403)
                    .await
                    .map_err(|e| ApiError::RateLimit(e.to_string()))?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait]
impl ExchangeClient for MEXCEmulator {
    async fn place_order(&self, order: OrderRequest) -> ClientResult<Order> {
        // Convert OrderRequest to emulation Order
        let emulation_order = crate::emulation::trading::Order {
            symbol: order.symbol.clone(),
            side: match order.side {
                OrderSide::Buy => crate::emulation::trading::OrderSide::Buy,
                OrderSide::Sell => crate::emulation::trading::OrderSide::Sell,
            },
            order_type: match order.order_type {
                OrderType::Market => crate::emulation::trading::OrderType::Market,
                OrderType::Limit => crate::emulation::trading::OrderType::Limit,
            },
            quantity: order.quantity.to_string().parse().unwrap_or(0.0),
            price: order.price.map(|p| p.to_string().parse().unwrap_or(0.0)),
        };

        // Execute order through trading executor
        let response = {
            let mut executor_guard = self.trading_executor.write().await;
            if let Some(executor) = executor_guard.as_mut() {
                executor
                    .execute_order(emulation_order)
                    .await
                    .map_err(|e| {
                        // Handle error with recovery
                        let _ = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(self.handle_error(&e))
                        });
                        ApiError::OrderFailed(e.to_string())
                    })?
            } else {
                return Err(ApiError::Connection("Trading executor not initialized".to_string()));
            }
        };

        // Increment trade count for mode manager
        self.mode_manager.increment_trade_count().await;

        // Convert response back to ExchangeClient format
        Ok(Order {
            id: response.order_id,
            symbol: order.symbol,
            status: match response.status {
                crate::emulation::trading::OrderStatus::Filled => OrderStatus::Filled,
                crate::emulation::trading::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
                crate::emulation::trading::OrderStatus::Pending => OrderStatus::New,
                crate::emulation::trading::OrderStatus::Rejected => OrderStatus::Cancelled,
            },
            filled_quantity: Decimal::from_f64_retain(response.filled_quantity)
                .unwrap_or(Decimal::ZERO),
        })
    }

    async fn cancel_order(&self, order_id: &str) -> ClientResult<()> {
        // In real implementation: send cancel request through trading executor
        info!("Cancelling order: {}", order_id);
        Ok(())
    }

    async fn get_balance(&self, asset: &str) -> ClientResult<Balance> {
        // In real implementation: query balance through trading executor
        info!("Getting balance for: {}", asset);
        
        Ok(Balance {
            asset: asset.to_string(),
            free: Decimal::from(100),
            locked: Decimal::ZERO,
        })
    }
}

impl Drop for MEXCEmulator {
    fn drop(&mut self) {
        // Cancel periodic activity task on drop
        if let Ok(mut handle) = self.periodic_activity_handle.try_write() {
            if let Some(h) = handle.take() {
                h.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mexc_emulator_creation() {
        // Set required environment variable
        std::env::set_var("EMULATION_SESSION_KEY", "0".repeat(64));

        let config = EmulationConfig::default();
        let result = MEXCEmulator::new(config).await;

        // May fail without actual MEXC credentials, but should not panic
        let _ = result;
    }
}
