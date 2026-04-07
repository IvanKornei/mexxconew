/// Trading executor - executes trades via browser or API
/// 
/// Connects trading signals with actual order execution

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::{
    emulation::browser::BrowserActions,
    trading::strategy::TradingSignal,
    exchanges::binance_client::BinanceClient,
};

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Dry run - log only, no real trades
    DryRun,
    /// Live trading via browser emulation
    Live,
    /// Paper trading - simulate with real prices
    Paper,
}

/// Trading executor
pub struct TradingExecutor {
    mode: ExecutionMode,
    browser: Arc<RwLock<Option<BrowserActions>>>,
    binance_client: Option<Arc<BinanceClient>>,
}

impl TradingExecutor {
    /// Create new trading executor
    pub fn new(
        mode: ExecutionMode,
        browser: Arc<RwLock<Option<BrowserActions>>>,
    ) -> Self {
        Self {
            mode,
            browser,
            binance_client: None,
        }
    }
    
    /// Create new trading executor with Binance client
    pub fn with_binance(
        mode: ExecutionMode,
        browser: Arc<RwLock<Option<BrowserActions>>>,
        binance_client: Arc<BinanceClient>,
    ) -> Self {
        Self {
            mode,
            browser,
            binance_client: Some(binance_client),
        }
    }
    
    /// Execute trading signal
    pub async fn execute(&self, signal: TradingSignal) -> Result<ExecutionResult, ExecutionError> {
        match signal {
            TradingSignal::OpenArbitrage {
                buy_exchange,
                sell_exchange,
                spread_percent,
                expected_profit,
                position_size,
            } => {
                info!(
                    "📊 Executing arbitrage: Buy {} @ {}, Sell {} @ {}, Size: {:.4} BTC",
                    buy_exchange, buy_exchange, sell_exchange, sell_exchange, position_size
                );
                
                self.execute_arbitrage_open(
                    &buy_exchange,
                    &sell_exchange,
                    position_size,
                    spread_percent,
                    expected_profit,
                ).await
            }
            TradingSignal::CloseArbitrage { reason } => {
                info!("📊 Closing arbitrage: {}", reason);
                self.execute_arbitrage_close(&reason).await
            }
            TradingSignal::NoAction => {
                Ok(ExecutionResult::NoAction)
            }
        }
    }
    
    /// Execute arbitrage opening
    async fn execute_arbitrage_open(
        &self,
        buy_exchange: &str,
        sell_exchange: &str,
        position_size: f64,
        spread_percent: f64,
        expected_profit: f64,
    ) -> Result<ExecutionResult, ExecutionError> {
        match self.mode {
            ExecutionMode::DryRun => {
                info!(
                    "🔍 [DRY RUN] Would open arbitrage: Buy {} {:.4} BTC, Sell {} {:.4} BTC",
                    buy_exchange, position_size, sell_exchange, position_size
                );
                info!("   Expected profit: ${:.2} ({:.3}% spread)", expected_profit, spread_percent);
                
                Ok(ExecutionResult::DryRun {
                    action: "open_arbitrage".to_string(),
                    details: format!(
                        "Buy {} {:.4} BTC, Sell {} {:.4} BTC, Profit: ${:.2}",
                        buy_exchange, position_size, sell_exchange, position_size, expected_profit
                    ),
                })
            }
            ExecutionMode::Paper => {
                info!(
                    "📝 [PAPER] Opening arbitrage: Buy {} {:.4} BTC, Sell {} {:.4} BTC",
                    buy_exchange, position_size, sell_exchange, position_size
                );
                
                // Simulate order IDs
                let buy_order_id = format!("paper_buy_{}", chrono::Utc::now().timestamp());
                let sell_order_id = format!("paper_sell_{}", chrono::Utc::now().timestamp());
                
                Ok(ExecutionResult::Paper {
                    buy_order_id,
                    sell_order_id,
                    expected_profit,
                })
            }
            ExecutionMode::Live => {
                info!("🚀 [LIVE] Opening arbitrage positions");
                
                // Execute on both exchanges simultaneously
                let (buy_result, sell_result) = tokio::join!(
                    self.execute_buy(buy_exchange, position_size),
                    self.execute_sell(sell_exchange, position_size)
                );
                
                let buy_order_id = buy_result?;
                let sell_order_id = sell_result?;
                
                info!("✅ Arbitrage opened: Buy order {}, Sell order {}", buy_order_id, sell_order_id);
                
                Ok(ExecutionResult::Live {
                    buy_order_id,
                    sell_order_id,
                    actual_profit: None, // Will be calculated on close
                })
            }
        }
    }
    
    /// Execute arbitrage closing
    async fn execute_arbitrage_close(
        &self,
        reason: &str,
    ) -> Result<ExecutionResult, ExecutionError> {
        match self.mode {
            ExecutionMode::DryRun => {
                info!("🔍 [DRY RUN] Would close arbitrage: {}", reason);
                Ok(ExecutionResult::DryRun {
                    action: "close_arbitrage".to_string(),
                    details: reason.to_string(),
                })
            }
            ExecutionMode::Paper => {
                info!("📝 [PAPER] Closing arbitrage: {}", reason);
                Ok(ExecutionResult::Paper {
                    buy_order_id: "paper_close_buy".to_string(),
                    sell_order_id: "paper_close_sell".to_string(),
                    expected_profit: 0.0,
                })
            }
            ExecutionMode::Live => {
                info!("🚀 [LIVE] Closing arbitrage positions");
                
                // Close both positions
                // This would reverse the original trades
                // TODO: Implement actual closing logic
                
                Ok(ExecutionResult::Live {
                    buy_order_id: "close_buy".to_string(),
                    sell_order_id: "close_sell".to_string(),
                    actual_profit: Some(0.0), // TODO: Calculate actual profit
                })
            }
        }
    }
    
    /// Execute buy order
    async fn execute_buy(
        &self,
        exchange: &str,
        size: f64,
    ) -> Result<String, ExecutionError> {
        if exchange == "MEXC" {
            // Use browser for MEXC
            self.execute_mexc_order("BUY", size).await
        } else {
            // Use API for Binance
            self.execute_binance_order("BUY", size).await
        }
    }
    
    /// Execute sell order
    async fn execute_sell(
        &self,
        exchange: &str,
        size: f64,
    ) -> Result<String, ExecutionError> {
        if exchange == "MEXC" {
            // Use browser for MEXC
            self.execute_mexc_order("SELL", size).await
        } else {
            // Use API for Binance
            self.execute_binance_order("SELL", size).await
        }
    }
    
    /// Execute MEXC order via browser
    async fn execute_mexc_order(
        &self,
        side: &str,
        size: f64,
    ) -> Result<String, ExecutionError> {
        let mut browser_lock = self.browser.write().await;
        
        let browser = browser_lock.as_mut()
            .ok_or_else(|| ExecutionError::BrowserNotRunning)?;
        
        info!("🌐 Executing MEXC {} order via browser: {:.4} BTC", side, size);
        
        let order_id = browser.place_market_order(side, size).await
            .map_err(|e| ExecutionError::OrderFailed(format!("MEXC order failed: {}", e)))?;
        
        info!("✅ MEXC order placed: {}", order_id);
        
        Ok(order_id)
    }
    
    /// Execute Binance order via API
    async fn execute_binance_order(
        &self,
        side: &str,
        size: f64,
    ) -> Result<String, ExecutionError> {
        info!("📡 Executing Binance {} order via API: {:.4} BTC", side, size);
        
        if let Some(ref client) = self.binance_client {
            use crate::exchanges::client::{ExchangeClient, OrderRequest, OrderSide, OrderType};
            use rust_decimal::Decimal;
            
            let order_side = match side {
                "BUY" => OrderSide::Buy,
                "SELL" => OrderSide::Sell,
                _ => return Err(ExecutionError::OrderFailed(format!("Invalid side: {}", side))),
            };
            
            let order_request = OrderRequest {
                symbol: "BTCUSDT".to_string(),
                side: order_side,
                order_type: OrderType::Market,
                quantity: Decimal::from_f64_retain(size)
                    .ok_or_else(|| ExecutionError::OrderFailed("Invalid quantity".to_string()))?,
                price: None,
            };
            
            let order = client.place_order(order_request).await
                .map_err(|e| ExecutionError::OrderFailed(format!("Binance API error: {}", e)))?;
            
            info!("✅ Binance order placed: {}", order.id);
            Ok(order.id)
        } else {
            // Fallback to simulation if no client configured
            info!("⚠️ Binance client not configured, simulating order");
            let order_id = format!("binance_sim_{}_{}", side.to_lowercase(), chrono::Utc::now().timestamp());
            Ok(order_id)
        }
    }
}

/// Execution result
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Dry run result
    DryRun {
        action: String,
        details: String,
    },
    /// Paper trading result
    Paper {
        buy_order_id: String,
        sell_order_id: String,
        expected_profit: f64,
    },
    /// Live trading result
    Live {
        buy_order_id: String,
        sell_order_id: String,
        actual_profit: Option<f64>,
    },
    /// No action taken
    NoAction,
}

/// Execution error
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Browser not running")]
    BrowserNotRunning,
    
    #[error("Order failed: {0}")]
    OrderFailed(String),
    
    #[error("Insufficient balance")]
    InsufficientBalance,
    
    #[error("Exchange error: {0}")]
    ExchangeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dry_run_execution() {
        let browser = Arc::new(RwLock::new(None));
        let executor = TradingExecutor::new(ExecutionMode::DryRun, browser);
        
        let signal = TradingSignal::OpenArbitrage {
            buy_exchange: "Binance".to_string(),
            sell_exchange: "MEXC".to_string(),
            spread_percent: 0.5,
            expected_profit: 50.0,
            position_size: 0.01,
        };
        
        let result = executor.execute(signal).await;
        assert!(result.is_ok());
    }
}
