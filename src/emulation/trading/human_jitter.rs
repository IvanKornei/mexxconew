use rand::thread_rng;
use rand_distr::{Distribution, Normal};
use std::time::Duration;

/// Human jitter generator for adding realistic delays to trading operations
/// 
/// Uses normal distribution to generate delays that mimic human reaction times,
/// avoiding detectable fixed patterns.
pub struct HumanJitter {
    distribution: Normal<f64>,
    min_delay_ms: f64,
    max_delay_ms: f64,
}

impl HumanJitter {
    /// Create a new HumanJitter with default parameters
    /// 
    /// Default: mean=50ms, std_dev=15ms, range=[20ms, 80ms]
    pub fn new() -> Self {
        Self::with_params(50.0, 15.0, 20.0, 80.0)
    }

    /// Create a new HumanJitter with custom parameters
    /// 
    /// # Arguments
    /// * `mean_ms` - Mean delay in milliseconds (center of distribution)
    /// * `std_dev_ms` - Standard deviation in milliseconds (spread)
    /// * `min_delay_ms` - Minimum delay to clamp to
    /// * `max_delay_ms` - Maximum delay to clamp to
    pub fn with_params(mean_ms: f64, std_dev_ms: f64, min_delay_ms: f64, max_delay_ms: f64) -> Self {
        let distribution = Normal::new(mean_ms, std_dev_ms)
            .expect("Invalid normal distribution parameters");
        
        Self {
            distribution,
            min_delay_ms,
            max_delay_ms,
        }
    }

    /// Create HumanJitter from configuration
    pub fn from_config(config: &crate::emulation::config::JitterConfig) -> Self {
        Self::with_params(
            config.mean_delay_ms,
            config.std_dev_ms,
            config.min_delay_ms as f64,
            config.max_delay_ms as f64,
        )
    }

    /// Generate a random delay duration
    /// 
    /// Returns a Duration sampled from the normal distribution and clamped
    /// to the configured min/max range.
    pub fn generate_delay(&self) -> Duration {
        let mut rng = thread_rng();
        let delay_ms = self.distribution
            .sample(&mut rng)
            .clamp(self.min_delay_ms, self.max_delay_ms);
        
        Duration::from_millis(delay_ms as u64)
    }

    /// Apply jitter by sleeping for a random duration
    /// 
    /// This is an async function that will pause execution for a random
    /// amount of time based on the configured distribution.
    pub async fn apply_jitter(&self) {
        let delay = self.generate_delay();
        tokio::time::sleep(delay).await;
    }

    /// Get the configured mean delay in milliseconds
    pub fn mean_delay_ms(&self) -> f64 {
        self.distribution.mean()
    }

    /// Get the configured standard deviation in milliseconds
    pub fn std_dev_ms(&self) -> f64 {
        self.distribution.std_dev()
    }

    /// Get the minimum delay in milliseconds
    pub fn min_delay_ms(&self) -> f64 {
        self.min_delay_ms
    }

    /// Get the maximum delay in milliseconds
    pub fn max_delay_ms(&self) -> f64 {
        self.max_delay_ms
    }
}

impl Default for HumanJitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_creation() {
        let jitter = HumanJitter::new();
        assert_eq!(jitter.mean_delay_ms(), 50.0);
        assert_eq!(jitter.std_dev_ms(), 15.0);
        assert_eq!(jitter.min_delay_ms(), 20.0);
        assert_eq!(jitter.max_delay_ms(), 80.0);
    }

    #[test]
    fn test_custom_params() {
        let jitter = HumanJitter::with_params(100.0, 20.0, 50.0, 150.0);
        assert_eq!(jitter.mean_delay_ms(), 100.0);
        assert_eq!(jitter.std_dev_ms(), 20.0);
        assert_eq!(jitter.min_delay_ms(), 50.0);
        assert_eq!(jitter.max_delay_ms(), 150.0);
    }

    #[test]
    fn test_delay_generation_within_bounds() {
        let jitter = HumanJitter::new();
        
        // Generate 100 samples and verify they're all within bounds
        for _ in 0..100 {
            let delay = jitter.generate_delay();
            let delay_ms = delay.as_millis() as f64;
            
            assert!(delay_ms >= jitter.min_delay_ms(), 
                "Delay {} is below minimum {}", delay_ms, jitter.min_delay_ms());
            assert!(delay_ms <= jitter.max_delay_ms(), 
                "Delay {} is above maximum {}", delay_ms, jitter.max_delay_ms());
        }
    }

    #[test]
    fn test_delay_distribution() {
        let jitter = HumanJitter::new();
        let mut delays = Vec::new();
        
        // Generate 1000 samples
        for _ in 0..1000 {
            let delay = jitter.generate_delay();
            delays.push(delay.as_millis() as f64);
        }
        
        // Calculate mean
        let mean = delays.iter().sum::<f64>() / delays.len() as f64;
        
        // Mean should be close to configured mean (within 10%)
        assert!((mean - jitter.mean_delay_ms()).abs() < jitter.mean_delay_ms() * 0.1,
            "Mean {} is too far from expected {}", mean, jitter.mean_delay_ms());
    }

    #[test]
    fn test_no_fixed_pattern() {
        let jitter = HumanJitter::new();
        let mut delays = Vec::new();
        
        // Generate 10 samples
        for _ in 0..10 {
            let delay = jitter.generate_delay();
            delays.push(delay.as_millis());
        }
        
        // Verify not all delays are the same (no fixed pattern)
        let first = delays[0];
        let all_same = delays.iter().all(|&d| d == first);
        assert!(!all_same, "All delays are the same - fixed pattern detected!");
    }

    #[tokio::test]
    async fn test_apply_jitter() {
        let jitter = HumanJitter::new();
        let start = std::time::Instant::now();
        
        jitter.apply_jitter().await;
        
        let elapsed = start.elapsed();
        
        // Verify we actually waited
        assert!(elapsed.as_millis() >= jitter.min_delay_ms() as u128);
        assert!(elapsed.as_millis() <= (jitter.max_delay_ms() + 10.0) as u128); // +10ms tolerance
    }
}
