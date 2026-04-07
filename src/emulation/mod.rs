// Emulation system modules
pub mod config;
pub mod errors;
pub mod tls;
pub mod trading;
pub mod behavior;
pub mod persistence;
pub mod session;
pub mod mode_manager;
pub mod proxy;
pub mod recovery;
pub mod browser_cookies;
pub mod browser;
pub mod mexc_cookie_client;

use serde::{Deserialize, Serialize};

// Re-export commonly used types
// pub use tls::profiles::{InitializationProfile, TradingProfile};

/// Trading mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TradingMode {
    /// Hybrid mode: adds human-like delays and periodic background activity
    Hybrid,
    /// Aggressive mode: minimal delays for maximum speed
    Aggressive,
}

impl Default for TradingMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

impl TradingMode {
    /// Returns true if this is Hybrid mode
    pub fn is_hybrid(&self) -> bool {
        matches!(self, Self::Hybrid)
    }

    /// Returns true if this is Aggressive mode
    pub fn is_aggressive(&self) -> bool {
        matches!(self, Self::Aggressive)
    }
}
