/// Trading system - connects arbitrage detection with execution
/// 
/// This module integrates:
/// - Arbitrage detection (spread monitoring)
/// - Risk management
/// - Order execution (via browser emulation)
/// - Position management

pub mod strategy;
pub mod executor;
pub mod engine;
pub mod risk_manager;
pub mod position_tracker;

pub use strategy::{ArbitrageStrategy, TradingSignal};
pub use executor::{TradingExecutor, ExecutionMode};
pub use engine::{TradingEngine, EngineConfig, TradingStats};
pub use risk_manager::RiskManager;
pub use position_tracker::PositionTracker;
