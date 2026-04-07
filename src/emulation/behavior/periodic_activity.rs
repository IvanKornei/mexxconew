use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::emulation::{
    behavior::BehaviorSimulator,
    config::{BehaviorConfig, PeriodicActivityConfig},
    errors::Result,
    TradingMode,
};

/// Types of periodic activities to simulate
#[derive(Debug, Clone, Copy)]
pub enum Activity {
    CheckBalance,
    ViewOpenPositions,
    ViewTradeHistory,
    ViewOtherPair(&'static str),
    TakeBreak,
}

/// Periodic activity simulator for background human-like behavior
///
/// Runs in a separate tokio task and performs random UI actions
/// at intervals determined by the trading mode.
pub struct PeriodicActivitySimulator {
    behavior_simulator: BehaviorSimulator,
    activity_config: PeriodicActivityConfig,
    trading_mode: TradingMode,
    action_count: u64,
}

impl PeriodicActivitySimulator {
    /// Create a new periodic activity simulator
    pub fn new(
        behavior_config: BehaviorConfig,
        activity_config: PeriodicActivityConfig,
        trading_mode: TradingMode,
    ) -> Self {
        Self {
            behavior_simulator: BehaviorSimulator::new(behavior_config),
            activity_config,
            trading_mode,
            action_count: 0,
        }
    }

    /// Run the background activity loop
    ///
    /// This should be spawned in a separate tokio task.
    /// It will run indefinitely, performing random activities at intervals.
    pub async fn run_background_loop(&mut self) -> Result<()> {
        info!(
            "🎬 Starting periodic activity simulator in {:?} mode",
            self.trading_mode
        );

        loop {
            let interval = self.calculate_next_interval();
            debug!("Next activity in {:?}", interval);
            sleep(interval).await;

            if let Err(e) = self.perform_random_activity().await {
                tracing::warn!("Failed to perform periodic activity: {}", e);
            }

            self.action_count += 1;
        }
    }

    /// Calculate the next activity interval based on trading mode
    fn calculate_next_interval(&self) -> Duration {
        let mut rng = thread_rng();

        let (min_secs, max_secs) = match self.trading_mode {
            TradingMode::Hybrid => (600, 1200),      // 10-20 minutes
            TradingMode::Aggressive => (1800, 3600), // 30-60 minutes
        };

        Duration::from_secs(rng.gen_range(min_secs..=max_secs))
    }

    /// Perform a random activity based on configured probabilities
    async fn perform_random_activity(&mut self) -> Result<()> {
        let activity = self.select_random_activity();
        
        info!("🎭 Performing periodic activity: {:?}", activity);

        match activity {
            Activity::CheckBalance => self.simulate_check_balance().await,
            Activity::ViewOpenPositions => self.simulate_view_positions().await,
            Activity::ViewTradeHistory => self.simulate_view_history().await,
            Activity::ViewOtherPair(pair) => self.simulate_view_other_pair(pair).await,
            Activity::TakeBreak => self.simulate_break().await,
        }
    }

    /// Select a random activity based on configured probabilities
    fn select_random_activity(&self) -> Activity {
        let mut rng = thread_rng();
        let roll: f64 = rng.gen();

        let mut cumulative = 0.0;

        cumulative += self.activity_config.check_balance_probability;
        if roll < cumulative {
            return Activity::CheckBalance;
        }

        cumulative += self.activity_config.view_positions_probability;
        if roll < cumulative {
            return Activity::ViewOpenPositions;
        }

        cumulative += self.activity_config.view_history_probability;
        if roll < cumulative {
            return Activity::ViewTradeHistory;
        }

        cumulative += self.activity_config.view_other_pair_probability;
        if roll < cumulative {
            let pairs = ["ETH_USDT", "SOL_USDT", "BNB_USDT"];
            let pair = pairs[rng.gen_range(0..pairs.len())];
            return Activity::ViewOtherPair(pair);
        }

        // Default to check balance if probabilities don't sum to 1
        Activity::CheckBalance
    }

    /// Simulate checking account balance
    async fn simulate_check_balance(&mut self) -> Result<()> {
        debug!("Simulating balance check");

        // Simulate clicking on balance tab
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Wait for data to load
        sleep(Duration::from_millis(300)).await;

        // Simulate brief viewing
        sleep(self.behavior_simulator.random_pause(1000, 2000)).await;

        Ok(())
    }

    /// Simulate viewing open positions
    async fn simulate_view_positions(&mut self) -> Result<()> {
        debug!("Simulating view open positions");

        // Click on positions tab
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Wait for data to load
        sleep(Duration::from_millis(400)).await;

        // Simulate scrolling through positions
        let scroll_events = self.behavior_simulator.simulate_scroll(200);
        for (_, delay) in scroll_events {
            sleep(delay).await;
        }

        // View for a bit
        sleep(self.behavior_simulator.random_pause(2000, 4000)).await;

        Ok(())
    }

    /// Simulate viewing trade history
    async fn simulate_view_history(&mut self) -> Result<()> {
        debug!("Simulating view trade history");

        // Click on history tab
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Wait for data to load
        sleep(Duration::from_millis(500)).await;

        // Simulate scrolling through history
        let scroll_events = self.behavior_simulator.simulate_scroll(300);
        for (_, delay) in scroll_events {
            sleep(delay).await;
        }

        // View for a bit
        sleep(self.behavior_simulator.random_pause(1500, 3000)).await;

        Ok(())
    }

    /// Simulate viewing another trading pair
    async fn simulate_view_other_pair(&mut self, pair: &str) -> Result<()> {
        debug!("Simulating view of pair: {}", pair);

        // Click on pair selector
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Type pair name
        let typing = self.behavior_simulator.simulate_typing(pair);
        for (_, delay) in typing {
            sleep(delay).await;
        }

        // Select pair
        sleep(Duration::from_millis(200)).await;
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Wait for chart to load
        sleep(Duration::from_millis(800)).await;

        // View chart briefly
        sleep(self.behavior_simulator.random_pause(3000, 7000)).await;

        // Navigate back to BTC/USDT
        sleep(self.behavior_simulator.simulate_click_delay()).await;
        let typing = self.behavior_simulator.simulate_typing("BTC_USDT");
        for (_, delay) in typing {
            sleep(delay).await;
        }
        sleep(self.behavior_simulator.simulate_click_delay()).await;

        Ok(())
    }

    /// Simulate taking a break (only in Hybrid mode)
    async fn simulate_break(&mut self) -> Result<()> {
        if self.trading_mode.is_aggressive() {
            return Ok(());
        }

        let duration_mins = {
            let mut rng = thread_rng();
            let [min, max] = self.activity_config.break_duration_minutes;
            rng.gen_range(min..=max)
        };

        info!("☕ Taking a break for {} minutes", duration_mins);
        sleep(Duration::from_secs(duration_mins * 60)).await;
        info!("✅ Break finished, resuming activity");

        Ok(())
    }

    /// Check if it's time for a break (Hybrid mode only)
    pub fn should_take_break(&self) -> bool {
        if self.trading_mode.is_aggressive() {
            return false;
        }

        let mut rng = thread_rng();
        let [min_hours, max_hours] = self.activity_config.break_interval_hours;
        
        // Simplified: take break every ~2-3 hours worth of actions
        // In real implementation, track actual time
        let actions_per_hour = 6; // Hybrid mode: 6 actions/hour
        let break_threshold = rng.gen_range(min_hours..=max_hours) * actions_per_hour;

        self.action_count >= break_threshold
    }

    /// Get the number of actions performed
    pub fn action_count(&self) -> u64 {
        self.action_count
    }

    /// Reset action count (e.g., after a break)
    pub fn reset_action_count(&mut self) {
        self.action_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulation::config::{BehaviorConfig, PeriodicActivityConfig};

    fn default_configs() -> (BehaviorConfig, PeriodicActivityConfig) {
        (BehaviorConfig::default(), PeriodicActivityConfig::default())
    }

    #[test]
    fn test_simulator_creation() {
        let (behavior_config, activity_config) = default_configs();
        let simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        assert_eq!(simulator.action_count(), 0);
    }

    #[test]
    fn test_interval_calculation_hybrid() {
        let (behavior_config, activity_config) = default_configs();
        let simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        let interval = simulator.calculate_next_interval();
        
        // Should be between 10-20 minutes
        assert!(interval.as_secs() >= 600);
        assert!(interval.as_secs() <= 1200);
    }

    #[test]
    fn test_interval_calculation_aggressive() {
        let (behavior_config, activity_config) = default_configs();
        let simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Aggressive,
        );

        let interval = simulator.calculate_next_interval();
        
        // Should be between 30-60 minutes
        assert!(interval.as_secs() >= 1800);
        assert!(interval.as_secs() <= 3600);
    }

    #[test]
    fn test_activity_selection() {
        let (behavior_config, activity_config) = default_configs();
        let simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        // Test that activity selection works (should not panic)
        for _ in 0..10 {
            let activity = simulator.select_random_activity();
            // Just verify it returns something
            match activity {
                Activity::CheckBalance
                | Activity::ViewOpenPositions
                | Activity::ViewTradeHistory
                | Activity::ViewOtherPair(_) => {}
                _ => panic!("Unexpected activity type"),
            }
        }
    }

    #[tokio::test]
    async fn test_check_balance_simulation() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        let result = simulator.simulate_check_balance().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_view_positions_simulation() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        let result = simulator.simulate_view_positions().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_view_other_pair_simulation() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        let result = simulator.simulate_view_other_pair("ETH_USDT").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_break_logic_hybrid() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        // Initially should not need break
        assert!(!simulator.should_take_break());

        // Simulate many actions
        simulator.action_count = 100;
        
        // Now might need a break (probabilistic)
        let _ = simulator.should_take_break();
    }

    #[test]
    fn test_break_logic_aggressive() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Aggressive,
        );

        // Aggressive mode never takes breaks
        simulator.action_count = 1000;
        assert!(!simulator.should_take_break());
    }

    #[test]
    fn test_action_count_tracking() {
        let (behavior_config, activity_config) = default_configs();
        let mut simulator = PeriodicActivitySimulator::new(
            behavior_config,
            activity_config,
            TradingMode::Hybrid,
        );

        assert_eq!(simulator.action_count(), 0);

        simulator.action_count = 5;
        assert_eq!(simulator.action_count(), 5);

        simulator.reset_action_count();
        assert_eq!(simulator.action_count(), 0);
    }
}
