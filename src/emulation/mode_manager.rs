use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::emulation::{
    config::EmulationConfig,
    errors::Result,
    session::SessionInitializer,
    TradingMode,
};

/// Mode manager for handling Hybrid/Aggressive mode switching
///
/// Handles:
/// - Automatic mode switching on errors
/// - Session reinitialization when switching modes
/// - Trade count tracking for periodic checks
pub struct ModeManager {
    current_mode: Arc<RwLock<TradingMode>>,
    config: Arc<RwLock<EmulationConfig>>,
    trade_count: Arc<RwLock<u64>>,
    last_mode_switch: Arc<RwLock<std::time::Instant>>,
}

impl ModeManager {
    /// Create a new mode manager
    pub fn new(config: EmulationConfig) -> Self {
        let initial_mode = config.trading_mode;
        
        Self {
            current_mode: Arc::new(RwLock::new(initial_mode)),
            config: Arc::new(RwLock::new(config)),
            trade_count: Arc::new(RwLock::new(0)),
            last_mode_switch: Arc::new(RwLock::new(std::time::Instant::now())),
        }
    }

    /// Get current trading mode
    pub async fn current_mode(&self) -> TradingMode {
        *self.current_mode.read().await
    }

    /// Switch to a different mode
    pub async fn switch_mode(&self, new_mode: TradingMode) -> Result<()> {
        let mut current = self.current_mode.write().await;
        
        if *current == new_mode {
            return Ok(());
        }

        info!("🔄 Switching trading mode: {:?} → {:?}", *current, new_mode);
        
        *current = new_mode;
        *self.last_mode_switch.write().await = std::time::Instant::now();
        
        // Update config
        let mut config = self.config.write().await;
        config.trading_mode = new_mode;

        Ok(())
    }

    /// Handle blocked error by switching to Hybrid mode and reinitializing session
    pub async fn handle_blocked_error(
        &self,
        session_initializer: &mut SessionInitializer,
    ) -> Result<()> {
        let current = self.current_mode().await;

        if current.is_aggressive() {
            warn!("⚠️ Blocked in Aggressive mode, switching to Hybrid and reinitializing session");
            
            // Switch to Hybrid mode
            self.switch_mode(TradingMode::Hybrid).await?;
            
            // Reinitialize session
            let _new_session = session_initializer.initialize_session().await?;
            
            info!("✅ Session reinitialized in Hybrid mode");
            
            Ok(())
        } else {
            // Already in Hybrid mode, just reinitialize session
            warn!("⚠️ Blocked in Hybrid mode, reinitializing session");
            session_initializer.initialize_session().await?;
            Ok(())
        }
    }

    /// Increment trade count
    pub async fn increment_trade_count(&self) {
        let mut count = self.trade_count.write().await;
        *count += 1;
    }

    /// Get current trade count
    pub async fn trade_count(&self) -> u64 {
        *self.trade_count.read().await
    }

    /// Reset trade count (e.g., after periodic check)
    pub async fn reset_trade_count(&self) {
        *self.trade_count.write().await = 0;
    }

    /// Check if periodic position check is needed (Hybrid mode only)
    ///
    /// In Hybrid mode, check positions after 15-25 trades
    pub async fn should_check_positions(&self) -> bool {
        let mode = self.current_mode().await;
        if !mode.is_hybrid() {
            return false;
        }

        let count = self.trade_count().await;
        let threshold = rand::random::<u64>() % 11 + 15; // 15-25 trades
        
        count >= threshold
    }

    /// Check if break is needed (Hybrid mode only)
    ///
    /// In Hybrid mode, take break after 2-3 hours of continuous trading
    pub async fn should_take_break(&self) -> bool {
        let mode = self.current_mode().await;
        if !mode.is_hybrid() {
            return false;
        }

        let last_switch = self.last_mode_switch.read().await;
        let elapsed = last_switch.elapsed();
        
        // Random threshold between 2-3 hours
        let threshold_hours = rand::random::<u64>() % 2 + 2;
        let threshold = std::time::Duration::from_secs(threshold_hours * 3600);
        
        elapsed >= threshold
    }

    /// Get time since last mode switch
    pub async fn time_since_mode_switch(&self) -> std::time::Duration {
        let last_switch = self.last_mode_switch.read().await;
        last_switch.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(mode: TradingMode) -> EmulationConfig {
        let mut config = EmulationConfig::default();
        config.trading_mode = mode;
        config
    }

    #[tokio::test]
    async fn test_mode_manager_creation() {
        let config = create_test_config(TradingMode::Hybrid);
        let manager = ModeManager::new(config);

        assert_eq!(manager.current_mode().await, TradingMode::Hybrid);
        assert_eq!(manager.trade_count().await, 0);
    }

    #[tokio::test]
    async fn test_mode_switching() {
        let config = create_test_config(TradingMode::Aggressive);
        let manager = ModeManager::new(config);

        assert_eq!(manager.current_mode().await, TradingMode::Aggressive);

        manager.switch_mode(TradingMode::Hybrid).await.unwrap();
        assert_eq!(manager.current_mode().await, TradingMode::Hybrid);
    }

    #[tokio::test]
    async fn test_trade_count_tracking() {
        let config = create_test_config(TradingMode::Hybrid);
        let manager = ModeManager::new(config);

        assert_eq!(manager.trade_count().await, 0);

        manager.increment_trade_count().await;
        assert_eq!(manager.trade_count().await, 1);

        manager.increment_trade_count().await;
        manager.increment_trade_count().await;
        assert_eq!(manager.trade_count().await, 3);

        manager.reset_trade_count().await;
        assert_eq!(manager.trade_count().await, 0);
    }

    #[tokio::test]
    async fn test_position_check_logic_hybrid() {
        let config = create_test_config(TradingMode::Hybrid);
        let manager = ModeManager::new(config);

        // Initially should not need check
        assert!(!manager.should_check_positions().await);

        // After many trades, should need check
        for _ in 0..20 {
            manager.increment_trade_count().await;
        }

        // Might need check now (probabilistic due to random threshold)
        let _ = manager.should_check_positions().await;
    }

    #[tokio::test]
    async fn test_position_check_logic_aggressive() {
        let config = create_test_config(TradingMode::Aggressive);
        let manager = ModeManager::new(config);

        // Aggressive mode never checks positions
        for _ in 0..100 {
            manager.increment_trade_count().await;
        }

        assert!(!manager.should_check_positions().await);
    }

    #[tokio::test]
    async fn test_break_logic_hybrid() {
        let config = create_test_config(TradingMode::Hybrid);
        let manager = ModeManager::new(config);

        // Initially should not need break
        assert!(!manager.should_take_break().await);
    }

    #[tokio::test]
    async fn test_break_logic_aggressive() {
        let config = create_test_config(TradingMode::Aggressive);
        let manager = ModeManager::new(config);

        // Aggressive mode never takes breaks
        assert!(!manager.should_take_break().await);
    }

    #[tokio::test]
    async fn test_time_tracking() {
        let config = create_test_config(TradingMode::Hybrid);
        let manager = ModeManager::new(config);

        let elapsed = manager.time_since_mode_switch().await;
        assert!(elapsed.as_millis() < 100);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let elapsed = manager.time_since_mode_switch().await;
        assert!(elapsed.as_millis() >= 100);
    }
}
