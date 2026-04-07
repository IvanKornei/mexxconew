use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::emulation::{TradingMode, errors::{EmulationError, Result}};

/// Main emulation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmulationConfig {
    pub trading_mode: TradingMode,
    pub browser_profile: BrowserProfile,
    pub session_storage_path: PathBuf,
    pub encryption_key_env: String,
    pub cookies: CookieConfig,
    pub rate_limits: RateLimitConfig,
    pub jitter: JitterConfig,
    pub session: SessionConfig,
    pub proxy: ProxyConfig,
    pub behavior: BehaviorConfig,
    pub periodic_activity: PeriodicActivityConfig,
}

/// Cookie extraction configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CookieConfig {
    pub auto_extract_on_startup: bool,
    pub preferred_browser: String,
    pub auto_refresh_interval_minutes: u64,
    pub manual_cookies_env: String,
}

impl Default for CookieConfig {
    fn default() -> Self {
        Self {
            auto_extract_on_startup: true,
            preferred_browser: "Auto".to_string(),
            auto_refresh_interval_minutes: 30,
            manual_cookies_env: "MEXC_COOKIES".to_string(),
        }
    }
}

/// Browser profile configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum BrowserProfile {
    Chrome145Windows64,
}

impl std::fmt::Display for BrowserProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserProfile::Chrome145Windows64 => write!(f, "Chrome145Windows64"),
        }
    }
}

impl Default for BrowserProfile {
    fn default() -> Self {
        Self::Chrome145Windows64
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    pub max_orders_per_minute: u32,
    pub background_actions_per_hour_hybrid: u32,
    pub background_actions_per_hour_aggressive: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_orders_per_minute: 60,
            background_actions_per_hour_hybrid: 6,
            background_actions_per_hour_aggressive: 2,
        }
    }
}

/// Human jitter configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JitterConfig {
    pub enabled_in_hybrid: bool,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    pub mean_delay_ms: f64,
    pub std_dev_ms: f64,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            enabled_in_hybrid: true,
            min_delay_ms: 20,
            max_delay_ms: 80,
            mean_delay_ms: 50.0,
            std_dev_ms: 15.0,
        }
    }
}

/// Session management configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionConfig {
    pub max_session_duration_hours: u64,
    pub auto_refresh_before_expiry_minutes: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_session_duration_hours: 24,
            auto_refresh_before_expiry_minutes: 30,
        }
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sticky_session: Option<bool>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: None,
            sticky_session: None,
        }
    }
}

/// Behavior simulation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorConfig {
    pub typing_delay_range_ms: [u64; 2],
    pub mouse_movement_steps: usize,
    pub scroll_acceleration: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            typing_delay_range_ms: [100, 300],
            mouse_movement_steps: 50,
            scroll_acceleration: true,
        }
    }
}

/// Periodic activity simulation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeriodicActivityConfig {
    pub check_balance_probability: f64,
    pub view_positions_probability: f64,
    pub view_history_probability: f64,
    pub view_other_pair_probability: f64,
    pub break_duration_minutes: [u64; 2],
    pub break_interval_hours: [u64; 2],
}

impl Default for PeriodicActivityConfig {
    fn default() -> Self {
        Self {
            check_balance_probability: 0.3,
            view_positions_probability: 0.4,
            view_history_probability: 0.2,
            view_other_pair_probability: 0.1,
            break_duration_minutes: [2, 5],
            break_interval_hours: [2, 3],
        }
    }
}

impl Default for EmulationConfig {
    fn default() -> Self {
        Self {
            trading_mode: TradingMode::default(),
            browser_profile: BrowserProfile::default(),
            session_storage_path: PathBuf::from(".kiro/emulation/session.enc"),
            encryption_key_env: "EMULATION_SESSION_KEY".to_string(),
            cookies: CookieConfig::default(),
            rate_limits: RateLimitConfig::default(),
            jitter: JitterConfig::default(),
            session: SessionConfig::default(),
            proxy: ProxyConfig::default(),
            behavior: BehaviorConfig::default(),
            periodic_activity: PeriodicActivityConfig::default(),
        }
    }
}

impl EmulationConfig {
    /// Load emulation configuration from TOML table
    pub fn from_toml(table: &toml::Table) -> Result<Self> {
        let emulation_table = table
            .get("emulation")
            .and_then(|v| v.as_table())
            .ok_or_else(|| EmulationError::ConfigError("Missing [emulation] section".to_string()))?;

        let config: EmulationConfig = emulation_table
            .clone()
            .try_into()
            .map_err(|e: toml::de::Error| {
                EmulationError::ConfigError(format!("Failed to parse emulation config: {}", e))
            })?;

        Ok(config)
    }

    /// Get background actions interval in seconds based on trading mode
    pub fn background_actions_interval_secs(&self) -> u64 {
        let actions_per_hour = match self.trading_mode {
            TradingMode::Hybrid => self.rate_limits.background_actions_per_hour_hybrid,
            TradingMode::Aggressive => self.rate_limits.background_actions_per_hour_aggressive,
        };

        if actions_per_hour == 0 {
            3600 // Default to 1 hour if misconfigured
        } else {
            3600 / actions_per_hour as u64
        }
    }

    /// Check if jitter should be applied based on trading mode
    pub fn should_apply_jitter(&self) -> bool {
        self.trading_mode.is_hybrid() && self.jitter.enabled_in_hybrid
    }
}
