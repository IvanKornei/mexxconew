/// Trading engine - main orchestrator
/// 
/// Connects all components:
/// - Price monitoring (PriceFeedManager)
/// - Spread calculation (SpreadCalculator)
/// - Strategy (ArbitrageStrategy)
/// - Execution (TradingExecutor)
/// - Risk management (RiskManager)
/// - Position tracking (PositionTracker)

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use crate::core::{
    spread::SpreadCalculator,
    PriceState,
};
use crate::emulation::browser::BrowserActions;
use crate::trading::{
    strategy::{ArbitrageStrategy, StrategyConfig, TradingSignal},
    executor::{TradingExecutor, ExecutionMode, ExecutionResult},
};
use tokio::sync::watch;

/// Trading engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Execution mode
    pub mode: ExecutionMode,
    /// Strategy configuration
    pub strategy: StrategyConfig,
    /// Analysis interval (ms)
    pub analysis_interval_ms: u64,
    /// Enable auto-trading
    pub auto_trading_enabled: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::DryRun,
            strategy: StrategyConfig::default(),
            analysis_interval_ms: 100, // Analyze every 100ms for HFT
            auto_trading_enabled: false,
        }
    }
}

/// Trading engine state
pub struct TradingEngine {
    config: EngineConfig,
    strategy: Arc<ArbitrageStrategy>,
    executor: Arc<TradingExecutor>,
    is_running: Arc<RwLock<bool>>,
    stats: Arc<RwLock<TradingStats>>,
}

/// Trading statistics
#[derive(Debug, Clone, Default)]
pub struct TradingStats {
    pub total_signals: u64,
    pub signals_executed: u64,
    pub signals_skipped: u64,
    pub total_profit: f64,
    pub total_loss: f64,
    pub win_rate: f64,
    pub last_signal_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl TradingEngine {
    /// Create new trading engine
    pub fn new(
        config: EngineConfig,
        state_rx: watch::Receiver<PriceState>,
        spread_calculator: Arc<SpreadCalculator>,
        browser: Arc<RwLock<Option<BrowserActions>>>,
    ) -> Self {
        let strategy = Arc::new(ArbitrageStrategy::new(
            config.strategy.clone(),
            spread_calculator,
            state_rx,
        ));
        
        let executor = Arc::new(TradingExecutor::new(
            config.mode,
            browser,
        ));
        
        Self {
            config,
            strategy,
            executor,
            is_running: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(TradingStats::default())),
        }
    }
    
    /// Start trading engine
    pub async fn start(&self) {
        let mut is_running = self.is_running.write().await;
        
        if *is_running {
            warn!("Trading engine already running");
            return;
        }
        
        *is_running = true;
        drop(is_running);
        
        info!("🚀 Starting trading engine");
        info!("   Mode: {:?}", self.config.mode);
        info!("   Auto-trading: {}", self.config.auto_trading_enabled);
        info!("   Analysis interval: {}ms", self.config.analysis_interval_ms);
        
        // Spawn trading loop
        let strategy = self.strategy.clone();
        let executor = self.executor.clone();
        let is_running = self.is_running.clone();
        let stats = self.stats.clone();
        let auto_trading = self.config.auto_trading_enabled;
        let interval_ms = self.config.analysis_interval_ms;
        
        tokio::spawn(async move {
            Self::trading_loop(
                strategy,
                executor,
                is_running,
                stats,
                auto_trading,
                interval_ms,
            ).await;
        });
        
        info!("✅ Trading engine started");
    }
    
    /// Stop trading engine
    pub async fn stop(&self) {
        let mut is_running = self.is_running.write().await;
        
        if !*is_running {
            warn!("Trading engine not running");
            return;
        }
        
        *is_running = false;
        
        info!("⏹️ Trading engine stopped");
    }
    
    /// Main trading loop
    async fn trading_loop(
        strategy: Arc<ArbitrageStrategy>,
        executor: Arc<TradingExecutor>,
        is_running: Arc<RwLock<bool>>,
        stats: Arc<RwLock<TradingStats>>,
        auto_trading: bool,
        interval_ms: u64,
    ) {
        let mut tick = interval(Duration::from_millis(interval_ms));
        
        loop {
            tick.tick().await;
            
            // Check if still running
            {
                let running = is_running.read().await;
                if !*running {
                    info!("Trading loop stopped");
                    break;
                }
            }
            
            // Analyze market
            let signal = strategy.analyze().await;
            
            // Update stats
            {
                let mut stats_lock = stats.write().await;
                stats_lock.total_signals += 1;
                
                if !matches!(signal, TradingSignal::NoAction) {
                    stats_lock.last_signal_time = Some(chrono::Utc::now());
                }
            }
            
            // Handle signal
            match signal {
                TradingSignal::NoAction => {
                    // No action needed
                    continue;
                }
                ref sig => {
                    info!("📊 Trading signal: {:?}", sig);
                    
                    if auto_trading {
                        // Execute automatically
                        match executor.execute(sig.clone()).await {
                            Ok(result) => {
                                Self::handle_execution_result(&result, &strategy, &stats).await;
                                
                                let mut stats_lock = stats.write().await;
                                stats_lock.signals_executed += 1;
                            }
                            Err(e) => {
                                error!("❌ Execution failed: {}", e);
                                
                                let mut stats_lock = stats.write().await;
                                stats_lock.signals_skipped += 1;
                            }
                        }
                    } else {
                        // Manual mode - just log
                        info!("⚠️ Auto-trading disabled. Signal logged but not executed.");
                        
                        let mut stats_lock = stats.write().await;
                        stats_lock.signals_skipped += 1;
                    }
                }
            }
        }
    }
    
    /// Handle execution result
    async fn handle_execution_result(
        result: &ExecutionResult,
        strategy: &Arc<ArbitrageStrategy>,
        stats: &Arc<RwLock<TradingStats>>,
    ) {
        match result {
            ExecutionResult::Live { actual_profit, .. } => {
                // Mark position in strategy
                if let Some(profit) = actual_profit {
                    let mut stats_lock = stats.write().await;
                    
                    if *profit > 0.0 {
                        stats_lock.total_profit += profit;
                    } else {
                        stats_lock.total_loss += profit.abs();
                    }
                    
                    // Update win rate
                    let total_trades = stats_lock.signals_executed;
                    if total_trades > 0 {
                        stats_lock.win_rate = stats_lock.total_profit / 
                            (stats_lock.total_profit + stats_lock.total_loss);
                    }
                    
                    strategy.mark_position_closed().await;
                } else {
                    // Position opened
                    // Signal will be passed to mark_position_opened separately
                }
            }
            ExecutionResult::Paper { expected_profit, .. } => {
                let mut stats_lock = stats.write().await;
                stats_lock.total_profit += expected_profit;
            }
            ExecutionResult::DryRun { .. } => {
                // Just logging, no stats update
            }
            ExecutionResult::NoAction => {}
        }
    }
    
    /// Get current statistics
    pub async fn get_stats(&self) -> TradingStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
    
    /// Check if engine is running
    pub async fn is_running(&self) -> bool {
        let is_running = self.is_running.read().await;
        *is_running
    }
    
    /// Get current position
    pub async fn get_current_position(&self) -> Option<crate::trading::strategy::ArbitragePosition> {
        self.strategy.get_current_position().await
    }
    
    /// Enable/disable auto-trading
    pub fn set_auto_trading(&mut self, enabled: bool) {
        self.config.auto_trading_enabled = enabled;
        info!("Auto-trading {}", if enabled { "enabled" } else { "disabled" });
    }
    
    /// Change execution mode
    pub fn set_execution_mode(&mut self, mode: ExecutionMode) {
        self.config.mode = mode;
        info!("Execution mode changed to: {:?}", mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_engine_config_defaults() {
        let config = EngineConfig::default();
        assert_eq!(config.mode, ExecutionMode::DryRun);
        assert_eq!(config.auto_trading_enabled, false);
    }
}
