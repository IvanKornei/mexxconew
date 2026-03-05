use serde::Deserialize;
use std::fs;
use crate::utils::errors::{Error, Result};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub system: SystemConfig,
    pub exchanges: ExchangesConfig,
    pub trading: TradingConfig,
    pub monitoring: MonitoringConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SystemConfig {
    pub log_level: String,
    pub log_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExchangesConfig {
    pub binance_ws: String,
    pub mexc_ws: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TradingConfig {
    pub symbol: String,
    pub min_spread_percent: f64,
    pub binance_fee: f64,
    pub mexc_fee: f64,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    pub leverage: f64,
    pub max_positions: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MonitoringConfig {
    pub stale_timeout_ms: u64,
    pub latency_warning_threshold_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
}

impl Config {
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;
        
        let config: Config = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;
        
        Ok(config)
    }
}
