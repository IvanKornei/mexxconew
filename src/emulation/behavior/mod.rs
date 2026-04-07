pub mod periodic_activity;

use rand::{thread_rng, Rng};
use std::time::Duration;

pub use periodic_activity::PeriodicActivitySimulator;

/// 2D Point for mouse movement
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Behavior simulator for human-like interactions
/// 
/// Simulates realistic human behavior patterns including:
/// - Mouse movement along Bezier curves
/// - Typing with variable delays
/// - Clicking with natural pauses
/// - Scrolling with acceleration/deceleration
pub struct BehaviorSimulator {
    config: crate::emulation::config::BehaviorConfig,
}

impl BehaviorSimulator {
    /// Create a new behavior simulator with the given configuration
    pub fn new(config: crate::emulation::config::BehaviorConfig) -> Self {
        Self { config }
    }

    /// Simulate mouse movement from one point to another using Bezier curve
    /// 
    /// Generates a smooth, natural-looking trajectory using cubic Bezier curve:
    /// B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
    /// 
    /// Returns a vector of intermediate points along the curve
    pub fn simulate_mouse_movement(&self, from: Point, to: Point) -> Vec<Point> {
        let steps = self.config.mouse_movement_steps;
        let mut points = Vec::with_capacity(steps);
        
        // Generate random control points for natural curve
        let control1 = self.generate_control_point(from, to, 0.3);
        let control2 = self.generate_control_point(from, to, 0.7);
        
        // Generate points along Bezier curve
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let point = self.bezier_curve(from, control1, control2, to, t);
            points.push(point);
        }
        
        points
    }

    /// Generate a control point for Bezier curve with some randomness
    fn generate_control_point(&self, from: Point, to: Point, ratio: f64) -> Point {
        let mut rng = thread_rng();
        
        // Linear interpolation with random offset
        let base_x = from.x + (to.x - from.x) * ratio;
        let base_y = from.y + (to.y - from.y) * ratio;
        
        // Add random perpendicular offset for natural curve
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        // Perpendicular vector
        let perp_x = -dy / distance;
        let perp_y = dx / distance;
        
        // Random offset (up to 20% of distance)
        let offset = rng.gen_range(-0.2..0.2) * distance;
        
        Point::new(
            base_x + perp_x * offset,
            base_y + perp_y * offset,
        )
    }

    /// Calculate point on cubic Bezier curve
    /// B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
    fn bezier_curve(&self, p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;
        
        Point::new(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }

    /// Simulate typing text with human-like delays between characters
    /// 
    /// Returns a vector of (character, delay_before_next) tuples
    pub fn simulate_typing(&self, text: &str) -> Vec<(char, Duration)> {
        let mut rng = thread_rng();
        let (min_delay, max_delay) = (
            self.config.typing_delay_range_ms[0],
            self.config.typing_delay_range_ms[1],
        );
        
        text.chars()
            .map(|c| {
                let delay_ms = rng.gen_range(min_delay..=max_delay);
                (c, Duration::from_millis(delay_ms))
            })
            .collect()
    }

    /// Simulate a click action with pre-click pause
    /// 
    /// Returns the delay to wait before clicking
    pub fn simulate_click_delay(&self) -> Duration {
        let mut rng = thread_rng();
        // Random delay between 50-200ms before click
        Duration::from_millis(rng.gen_range(50..=200))
    }

    /// Simulate scroll action with acceleration/deceleration
    /// 
    /// Returns a vector of (scroll_amount, delay) tuples representing
    /// scroll events with natural acceleration and deceleration
    pub fn simulate_scroll(&self, total_distance: i32) -> Vec<(i32, Duration)> {
        let mut rng = thread_rng();
        let mut events = Vec::new();
        
        if !self.config.scroll_acceleration {
            // Simple constant-speed scrolling
            let num_events = (total_distance.abs() / 100).max(1);
            let amount_per_event = total_distance / num_events;
            
            for _ in 0..num_events {
                events.push((amount_per_event, Duration::from_millis(rng.gen_range(30..=60))));
            }
            
            return events;
        }
        
        // Scrolling with acceleration and deceleration
        let num_events = (total_distance.abs() / 50).max(3);
        let sign = total_distance.signum();
        
        for i in 0..num_events {
            let progress = i as f64 / num_events as f64;
            
            // Acceleration curve: slow start, fast middle, slow end
            let speed_factor = if progress < 0.3 {
                // Acceleration phase
                progress / 0.3
            } else if progress > 0.7 {
                // Deceleration phase
                (1.0 - progress) / 0.3
            } else {
                // Constant speed phase
                1.0
            };
            
            let amount = (50.0 * speed_factor).max(10.0) as i32 * sign;
            let delay = Duration::from_millis(rng.gen_range(20..=50));
            
            events.push((amount, delay));
        }
        
        events
    }

    /// Generate a random pause duration for natural behavior
    pub fn random_pause(&self, min_ms: u64, max_ms: u64) -> Duration {
        let mut rng = thread_rng();
        Duration::from_millis(rng.gen_range(min_ms..=max_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> crate::emulation::config::BehaviorConfig {
        crate::emulation::config::BehaviorConfig::default()
    }

    #[test]
    fn test_mouse_movement_generation() {
        let simulator = BehaviorSimulator::new(default_config());
        let from = Point::new(0.0, 0.0);
        let to = Point::new(100.0, 100.0);
        
        let points = simulator.simulate_mouse_movement(from, to);
        
        // Should have correct number of points
        assert_eq!(points.len(), default_config().mouse_movement_steps + 1);
        
        // First point should be start
        assert!((points[0].x - from.x).abs() < 0.01);
        assert!((points[0].y - from.y).abs() < 0.01);
        
        // Last point should be end
        let last = points.last().unwrap();
        assert!((last.x - to.x).abs() < 0.01);
        assert!((last.y - to.y).abs() < 0.01);
    }

    #[test]
    fn test_bezier_curve_endpoints() {
        let simulator = BehaviorSimulator::new(default_config());
        let p0 = Point::new(0.0, 0.0);
        let p1 = Point::new(25.0, 50.0);
        let p2 = Point::new(75.0, 50.0);
        let p3 = Point::new(100.0, 100.0);
        
        // At t=0, should be at p0
        let start = simulator.bezier_curve(p0, p1, p2, p3, 0.0);
        assert!((start.x - p0.x).abs() < 0.01);
        assert!((start.y - p0.y).abs() < 0.01);
        
        // At t=1, should be at p3
        let end = simulator.bezier_curve(p0, p1, p2, p3, 1.0);
        assert!((end.x - p3.x).abs() < 0.01);
        assert!((end.y - p3.y).abs() < 0.01);
    }

    #[test]
    fn test_typing_simulation() {
        let simulator = BehaviorSimulator::new(default_config());
        let text = "hello";
        
        let typing = simulator.simulate_typing(text);
        
        // Should have one entry per character
        assert_eq!(typing.len(), text.len());
        
        // Verify characters match
        let chars: String = typing.iter().map(|(c, _)| c).collect();
        assert_eq!(chars, text);
        
        // Verify delays are within configured range
        let config = default_config();
        for (_, delay) in typing {
            let delay_ms = delay.as_millis() as u64;
            assert!(delay_ms >= config.typing_delay_range_ms[0]);
            assert!(delay_ms <= config.typing_delay_range_ms[1]);
        }
    }

    #[test]
    fn test_typing_has_variable_delays() {
        let simulator = BehaviorSimulator::new(default_config());
        let text = "test123";
        
        let typing = simulator.simulate_typing(text);
        let delays: Vec<u64> = typing.iter().map(|(_, d)| d.as_millis() as u64).collect();
        
        // Not all delays should be the same
        let first = delays[0];
        let all_same = delays.iter().all(|&d| d == first);
        assert!(!all_same, "All typing delays are identical - no variation!");
    }

    #[test]
    fn test_click_delay() {
        let simulator = BehaviorSimulator::new(default_config());
        
        let delay = simulator.simulate_click_delay();
        let delay_ms = delay.as_millis() as u64;
        
        // Should be within expected range
        assert!(delay_ms >= 50);
        assert!(delay_ms <= 200);
    }

    #[test]
    fn test_scroll_without_acceleration() {
        let mut config = default_config();
        config.scroll_acceleration = false;
        let simulator = BehaviorSimulator::new(config);
        
        let events = simulator.simulate_scroll(500);
        
        // Should have multiple events
        assert!(events.len() > 0);
        
        // Total should approximately equal requested distance
        let total: i32 = events.iter().map(|(amount, _)| amount).sum();
        assert!((total - 500).abs() < 100);
    }

    #[test]
    fn test_scroll_with_acceleration() {
        let simulator = BehaviorSimulator::new(default_config());
        
        let events = simulator.simulate_scroll(1000);
        
        // Should have multiple events
        assert!(events.len() >= 3);
        
        // First event should be smaller (acceleration)
        // Middle events should be larger
        // Last event should be smaller (deceleration)
        let first_amount = events[0].0.abs();
        let middle_amount = events[events.len() / 2].0.abs();
        let last_amount = events.last().unwrap().0.abs();
        
        assert!(middle_amount >= first_amount || middle_amount >= last_amount,
            "Middle scroll should be faster than start or end");
    }

    #[test]
    fn test_random_pause() {
        let simulator = BehaviorSimulator::new(default_config());
        
        let pause = simulator.random_pause(100, 500);
        let pause_ms = pause.as_millis() as u64;
        
        assert!(pause_ms >= 100);
        assert!(pause_ms <= 500);
    }
}
