/// Arbitrage trading strategy
/// 
/// Monitors spread between Binance and MEXC, generates trading signals

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::core::{
    spread::SpreadCalculator,
    PriceState,
};
use tokio::sync::watch;

/// Trading signal
#[derive(Debug, Clone)]
pub enum TradingSignal {
    /// Open arbitrage position
    OpenArbitrage {
        /// Buy on this exchange
        buy_exchange: String,
        /// Sell on this exchange
        sell_exchange: String,
        /// Expected spread (%)
        spread_percent: f64,
        /// Expected profit (USDT)
        expected_profit: f64,
        /// Recommended position size (BTC)
        position_size: f64,
    },
    /// Close arbitrage position
    CloseArbitrage {
        /// Reason for closing
        reason: String,
    },
    /// No action
    NoAction,
}

/// Arbitrage strategy configuration
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Minimum spread to open position (%)
    pub min_spread_open: f64,
    /// Spread to close position (%)
    pub spread_close: f64,
    /// Maximum position size (BTC)
    pub max_position_size: f64,
    /// Minimum position size (BTC)
    pub min_position_size: f64,
    /// Capital per trade (USDT)
    pub capital_per_trade: f64,
    /// Maximum leverage
    pub max_leverage: u32,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            min_spread_open: 0.3,    // 0.3% minimum spread
            spread_close: 0.1,        // Close when spread < 0.1%
            max_position_size: 0.1,   // 0.1 BTC max
            min_position_size: 0.001, // 0.001 BTC min
            capital_per_trade: 1000.0, // $1000 per trade
            max_leverage: 200,
        }
    }
}

/// Arbitrage strategy
pub struct ArbitrageStrategy {
    config: StrategyConfig,
    spread_calculator: Arc<SpreadCalculator>,
    state_rx: watch::Receiver<PriceState>,
    current_position: Arc<RwLock<Option<ArbitragePosition>>>,
}

/// Current arbitrage position
#[derive(Debug, Clone)]
pub struct ArbitragePosition {
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub entry_spread: f64,
    pub position_size: f64,
    pub entry_time: chrono::DateTime<chrono::Utc>,
}

/// Simple spread structure for internal use
struct SimpleSpread {
    binance_price: f64,
    mexc_price: f64,
    spread_percent: f64,
}

impl ArbitrageStrategy {
    /// Create new arbitrage strategy
    pub fn new(
        config: StrategyConfig,
        spread_calculator: Arc<SpreadCalculator>,
        state_rx: watch::Receiver<PriceState>,
    ) -> Self {
        Self {
            config,
            spread_calculator,
            state_rx,
            current_position: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Analyze market and generate trading signal
    pub async fn analyze(&self) -> TradingSignal {
        // Get current state from watch channel
        let state = self.state_rx.borrow().clone();
        
        // Get current position
        let current_position = self.current_position.read().await;
        
        // If we have an open position, check if we should close it
        if let Some(position) = current_position.as_ref() {
            // Calculate spread using net spread method
            let spread_percent = self.spread_calculator.calculate_net_spread(state.mexc, state.binance);
            
            // Create a simple spread struct for compatibility
            let spread = SimpleSpread {
                binance_price: state.binance,
                mexc_price: state.mexc,
                spread_percent,
            };
            
            // Clone position to avoid borrow issues
            let position_clone = position.clone();
            drop(current_position);
            
            return self.check_close_signal(&position_clone, &spread).await;
        }
        
        drop(current_position);
        
        // No position - check if we should open one
        let spread_percent = self.spread_calculator.calculate_net_spread(state.mexc, state.binance);
        
        let spread = SimpleSpread {
            binance_price: state.binance,
            mexc_price: state.mexc,
            spread_percent,
        };
        
        self.check_open_signal(&spread).await
    }
    
    /// Check if we should open a new position
    async fn check_open_signal(&self, spread: &SimpleSpread) -> TradingSignal {
        let spread_percent = spread.spread_percent;
        
        // Check if spread is profitable
        if spread_percent < self.config.min_spread_open {
            debug!(
                "Spread {:.3}% below minimum {:.3}%",
                spread_percent,
                self.config.min_spread_open
            );
            return TradingSignal::NoAction;
        }
        
        // Determine which exchange to buy/sell
        let (buy_exchange, sell_exchange) = if spread.binance_price < spread.mexc_price {
            ("Binance".to_string(), "MEXC".to_string())
        } else {
            ("MEXC".to_string(), "Binance".to_string())
        };
        
        // Calculate position size
        let position_size = self.calculate_position_size(spread).await;
        
        if position_size < self.config.min_position_size {
            debug!("Position size {:.4} BTC too small", position_size);
            return TradingSignal::NoAction;
        }
        
        // Calculate expected profit
        let avg_price = (spread.binance_price + spread.mexc_price) / 2.0;
        let expected_profit = position_size * avg_price * (spread_percent / 100.0);
        
        info!(
            "🎯 Arbitrage opportunity detected: {:.3}% spread, expected profit: ${:.2}",
            spread_percent, expected_profit
        );
        
        TradingSignal::OpenArbitrage {
            buy_exchange,
            sell_exchange,
            spread_percent,
            expected_profit,
            position_size,
        }
    }
    
    /// Check if we should close existing position
    async fn check_close_signal(
        &self,
        position: &ArbitragePosition,
        spread: &SimpleSpread,
    ) -> TradingSignal {
        let spread_percent = spread.spread_percent;
        
        // Check if spread has narrowed enough to close
        if spread_percent <= self.config.spread_close {
            info!(
                "📉 Closing arbitrage: spread narrowed to {:.3}%",
                spread_percent
            );
            return TradingSignal::CloseArbitrage {
                reason: format!("Spread narrowed to {:.3}%", spread_percent),
            };
        }
        
        // Check if spread reversed (loss scenario)
        if spread_percent < 0.0 {
            warn!("⚠️ Spread reversed! Closing position to prevent loss");
            return TradingSignal::CloseArbitrage {
                reason: "Spread reversed".to_string(),
            };
        }
        
        // Check position duration (close after 1 hour)
        let duration = chrono::Utc::now() - position.entry_time;
        if duration.num_minutes() > 60 {
            info!("⏰ Closing arbitrage: position held for 1 hour");
            return TradingSignal::CloseArbitrage {
                reason: "Time limit reached".to_string(),
            };
        }
        
        debug!(
            "Holding position: spread {:.3}%, duration {}m",
            spread_percent,
            duration.num_minutes()
        );
        
        TradingSignal::NoAction
    }
    
    /// Calculate optimal position size
    async fn calculate_position_size(&self, spread: &SimpleSpread) -> f64 {
        let avg_price = (spread.binance_price + spread.mexc_price) / 2.0;
        
        // Calculate size based on capital
        let size_from_capital = self.config.capital_per_trade / avg_price;
        
        // Limit to max position size
        let size = size_from_capital.min(self.config.max_position_size);
        
        // Round to 3 decimals
        (size * 1000.0).round() / 1000.0
    }
    
    /// Mark position as opened
    pub async fn mark_position_opened(&self, signal: &TradingSignal) {
        if let TradingSignal::OpenArbitrage {
            buy_exchange,
            sell_exchange,
            spread_percent,
            position_size,
            ..
        } = signal
        {
            let position = ArbitragePosition {
                buy_exchange: buy_exchange.clone(),
                sell_exchange: sell_exchange.clone(),
                entry_spread: *spread_percent,
                position_size: *position_size,
                entry_time: chrono::Utc::now(),
            };
            
            let mut position_lock = self.current_position.write().await;
            *position_lock = Some(position);
            
            info!("✅ Position marked as opened");
        }
    }
    
    /// Mark position as closed
    pub async fn mark_position_closed(&self) {
        let mut position_lock = self.current_position.write().await;
        *position_lock = None;
        
        info!("✅ Position marked as closed");
    }
    
    /// Get current position
    pub async fn get_current_position(&self) -> Option<ArbitragePosition> {
        let position_lock = self.current_position.read().await;
        position_lock.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_strategy_config_defaults() {
        let config = StrategyConfig::default();
        assert_eq!(config.min_spread_open, 0.3);
        assert_eq!(config.max_position_size, 0.1);
    }
}
