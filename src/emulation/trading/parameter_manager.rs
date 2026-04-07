use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::emulation::{
    behavior::BehaviorSimulator,
    config::BehaviorConfig,
    errors::Result,
};

/// Trading parameters for order execution
#[derive(Debug, Clone)]
pub struct OrderParameters {
    pub position_size: f64,
    pub leverage: u8,
    pub stop_loss_percent: f64,
    pub take_profit_percent: f64,
}

impl Default for OrderParameters {
    fn default() -> Self {
        Self {
            position_size: 0.001,
            leverage: 200,
            stop_loss_percent: 1.0,
            take_profit_percent: 2.0,
        }
    }
}

/// Parameter manager for trading configuration
///
/// Manages trading parameters that are set once through UI emulation
/// and reused for subsequent trades to avoid suspicious patterns.
pub struct ParameterManager {
    parameters: OrderParameters,
    behavior_simulator: BehaviorSimulator,
    last_update: Instant,
    update_interval: Duration,
}

impl ParameterManager {
    /// Create a new parameter manager with default parameters
    pub fn new(behavior_config: BehaviorConfig) -> Self {
        Self {
            parameters: OrderParameters::default(),
            behavior_simulator: BehaviorSimulator::new(behavior_config),
            last_update: Instant::now(),
            update_interval: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Create with custom parameters
    pub fn with_parameters(
        behavior_config: BehaviorConfig,
        parameters: OrderParameters,
    ) -> Self {
        Self {
            parameters,
            behavior_simulator: BehaviorSimulator::new(behavior_config),
            last_update: Instant::now(),
            update_interval: Duration::from_secs(3600),
        }
    }

    /// Initialize parameters through UI emulation
    ///
    /// Simulates:
    /// - Clicking on parameter fields
    /// - Typing values with delays
    /// - Confirming settings
    pub async fn initialize_parameters(
        &mut self,
        position_size: f64,
        leverage: u8,
        stop_loss_percent: f64,
        take_profit_percent: f64,
    ) -> Result<()> {
        info!("🎯 Initializing trading parameters through UI emulation");

        // Simulate clicking on position size field
        debug!("Setting position size: {}", position_size);
        tokio::time::sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Simulate typing position size
        let position_str = format!("{:.4}", position_size);
        let typing = self.behavior_simulator.simulate_typing(&position_str);
        for (_, delay) in typing {
            tokio::time::sleep(delay).await;
        }

        // Pause before next field
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate clicking on leverage field
        debug!("Setting leverage: {}x", leverage);
        tokio::time::sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Simulate typing leverage
        let leverage_str = leverage.to_string();
        let typing = self.behavior_simulator.simulate_typing(&leverage_str);
        for (_, delay) in typing {
            tokio::time::sleep(delay).await;
        }

        // Pause before next field
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate setting stop loss
        debug!("Setting stop loss: {}%", stop_loss_percent);
        tokio::time::sleep(self.behavior_simulator.simulate_click_delay()).await;

        let stop_loss_str = format!("{:.1}", stop_loss_percent);
        let typing = self.behavior_simulator.simulate_typing(&stop_loss_str);
        for (_, delay) in typing {
            tokio::time::sleep(delay).await;
        }

        // Pause before next field
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate setting take profit
        debug!("Setting take profit: {}%", take_profit_percent);
        tokio::time::sleep(self.behavior_simulator.simulate_click_delay()).await;

        let take_profit_str = format!("{:.1}", take_profit_percent);
        let typing = self.behavior_simulator.simulate_typing(&take_profit_str);
        for (_, delay) in typing {
            tokio::time::sleep(delay).await;
        }

        // Simulate confirming settings
        tokio::time::sleep(Duration::from_millis(200)).await;
        tokio::time::sleep(self.behavior_simulator.simulate_click_delay()).await;

        // Update stored parameters
        self.parameters = OrderParameters {
            position_size,
            leverage,
            stop_loss_percent,
            take_profit_percent,
        };
        self.last_update = Instant::now();

        info!("✅ Trading parameters initialized successfully");
        Ok(())
    }

    /// Get current order parameters
    pub fn get_order_parameters(&self) -> &OrderParameters {
        &self.parameters
    }

    /// Update parameters if needed (once per hour)
    ///
    /// Checks if parameters need updating based on time elapsed.
    /// If update is needed, performs UI emulation to set new values.
    pub async fn update_parameters_if_needed(
        &mut self,
        new_parameters: Option<OrderParameters>,
    ) -> Result<bool> {
        if self.last_update.elapsed() < self.update_interval {
            debug!(
                "Parameters updated {} seconds ago, no update needed",
                self.last_update.elapsed().as_secs()
            );
            return Ok(false);
        }

        info!("⏰ Parameters update interval reached, updating...");

        let params = new_parameters.unwrap_or_else(|| self.parameters.clone());

        self.initialize_parameters(
            params.position_size,
            params.leverage,
            params.stop_loss_percent,
            params.take_profit_percent,
        )
        .await?;

        Ok(true)
    }

    /// Set the update interval
    pub fn set_update_interval(&mut self, interval: Duration) {
        self.update_interval = interval;
    }

    /// Get time since last update
    pub fn time_since_last_update(&self) -> Duration {
        self.last_update.elapsed()
    }

    /// Check if parameters need updating
    pub fn needs_update(&self) -> bool {
        self.last_update.elapsed() >= self.update_interval
    }

    /// Force update of parameters without time check
    pub async fn force_update(&mut self, parameters: OrderParameters) -> Result<()> {
        self.initialize_parameters(
            parameters.position_size,
            parameters.leverage,
            parameters.stop_loss_percent,
            parameters.take_profit_percent,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulation::config::BehaviorConfig;

    fn default_behavior_config() -> BehaviorConfig {
        BehaviorConfig::default()
    }

    #[test]
    fn test_parameter_manager_creation() {
        let manager = ParameterManager::new(default_behavior_config());
        let params = manager.get_order_parameters();

        assert_eq!(params.position_size, 0.001);
        assert_eq!(params.leverage, 200);
    }

    #[test]
    fn test_custom_parameters() {
        let custom_params = OrderParameters {
            position_size: 0.002,
            leverage: 100,
            stop_loss_percent: 2.0,
            take_profit_percent: 3.0,
        };

        let manager =
            ParameterManager::with_parameters(default_behavior_config(), custom_params.clone());
        let params = manager.get_order_parameters();

        assert_eq!(params.position_size, custom_params.position_size);
        assert_eq!(params.leverage, custom_params.leverage);
    }

    #[tokio::test]
    async fn test_initialize_parameters() {
        let mut manager = ParameterManager::new(default_behavior_config());

        let result = manager.initialize_parameters(0.005, 150, 1.5, 2.5).await;
        assert!(result.is_ok());

        let params = manager.get_order_parameters();
        assert_eq!(params.position_size, 0.005);
        assert_eq!(params.leverage, 150);
        assert_eq!(params.stop_loss_percent, 1.5);
        assert_eq!(params.take_profit_percent, 2.5);
    }

    #[tokio::test]
    async fn test_update_interval() {
        let mut manager = ParameterManager::new(default_behavior_config());
        manager.set_update_interval(Duration::from_secs(1));

        // Should not need update immediately
        assert!(!manager.needs_update());

        // Wait for interval
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should need update now
        assert!(manager.needs_update());
    }

    #[tokio::test]
    async fn test_update_parameters_if_needed() {
        let mut manager = ParameterManager::new(default_behavior_config());
        manager.set_update_interval(Duration::from_secs(1));

        // Should not update immediately
        let updated = manager.update_parameters_if_needed(None).await.unwrap();
        assert!(!updated);

        // Wait for interval
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should update now
        let new_params = OrderParameters {
            position_size: 0.003,
            leverage: 125,
            stop_loss_percent: 1.2,
            take_profit_percent: 2.8,
        };

        let updated = manager
            .update_parameters_if_needed(Some(new_params.clone()))
            .await
            .unwrap();
        assert!(updated);

        let params = manager.get_order_parameters();
        assert_eq!(params.position_size, new_params.position_size);
    }

    #[tokio::test]
    async fn test_force_update() {
        let mut manager = ParameterManager::new(default_behavior_config());

        let new_params = OrderParameters {
            position_size: 0.007,
            leverage: 175,
            stop_loss_percent: 1.8,
            take_profit_percent: 3.2,
        };

        // Force update without waiting for interval
        let result = manager.force_update(new_params.clone()).await;
        assert!(result.is_ok());

        let params = manager.get_order_parameters();
        assert_eq!(params.position_size, new_params.position_size);
        assert_eq!(params.leverage, new_params.leverage);
    }

    #[test]
    fn test_time_since_last_update() {
        let manager = ParameterManager::new(default_behavior_config());
        let elapsed = manager.time_since_last_update();

        // Should be very recent
        assert!(elapsed.as_millis() < 100);
    }
}
