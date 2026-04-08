// HFT-optimized allocator - reduces jitter by 30-50%
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod api;
mod core;
mod exchanges;
mod utils;

use tracing::{info, Level};
use std::sync::Arc;
use std::time::Duration;

use crate::api::WsServer;
use crate::core::{PriceFeedManager, SystemManager, TradingModeManager};
use crate::exchanges::mexc_client::MexcClient;
use crate::utils::{Config, SystemHealth, HealthChecker};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize rustls crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    
    // Initialize logging system
    utils::logging::init_logging(Level::INFO)?;
    
    info!("🚀 Starting HFT Arbitrage System");
    
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Load configuration
    let config = Config::load_from_file("config.toml")?;
    info!("✅ Configuration loaded successfully");
    
    // Create health monitoring system
    let mut system_health = SystemHealth::new();
    
    // Create health checkers for components
    let binance_health = HealthChecker::new("binance".to_string(), Duration::from_secs(30));
    let mexc_health = HealthChecker::new("mexc".to_string(), Duration::from_secs(30));
    
    system_health.add_component(binance_health.clone_checker());
    system_health.add_component(mexc_health.clone_checker());
    
    let system_health = Arc::new(system_health);
    
    // Create price feed manager with health checkers
    let (price_feed, state_rx, position_manager) = PriceFeedManager::new_with_health(
        config.exchanges.binance_ws.clone(),
        config.exchanges.mexc_ws.clone(),
        config.monitoring.stale_timeout_ms,
        config.trading.initial_capital,
        config.trading.position_size_percent,
        config.trading.leverage,
        config.trading.max_positions,
        binance_health,
        mexc_health,
    );
    
    info!("✅ Price feed manager initialized");
    
    // Create trading mode manager for separate emulation/live databases
    let trading_mode_manager = Arc::new(
        TradingModeManager::new(config.trading.initial_capital)
            .expect("Failed to create trading mode manager")
    );
    info!("✅ Trading mode manager initialized");
    
    // Connect trading mode manager to position manager
    position_manager.set_trading_mode_manager(trading_mode_manager.clone()).await;

    // Connect MEXC API client only when real credentials are provided.
    // Placeholder values (e.g. "your_api_key_here") are treated as unset so that
    // paper/emulation runs don't accidentally advertise Live trading as ready.
    let mexc_key = std::env::var("MEXC_API_KEY").ok().unwrap_or_default();
    let mexc_secret = std::env::var("MEXC_API_SECRET").ok().unwrap_or_default();
    let is_real_credential = |v: &str| {
        !v.is_empty()
            && !v.starts_with("your_")
            && !v.contains("placeholder")
            && !v.eq_ignore_ascii_case("changeme")
    };
    if is_real_credential(&mexc_key) && is_real_credential(&mexc_secret) {
        let mexc_client = Arc::new(MexcClient::new(mexc_key, mexc_secret));
        position_manager.set_mexc_client(mexc_client).await;
        info!("✅ MEXC API client connected (Live trading ready)");
    } else {
        tracing::warn!(
            "⚠️ MEXC_API_KEY/MEXC_API_SECRET not set (or placeholder) — \
             Live trading disabled. Emulation (paper) mode will still work."
        );
    }

    // Enable trading by default for emulation mode
    position_manager.set_trading_enabled(true).await;
    info!("✅ Trading enabled for emulation mode");
    
    // Create system manager for state persistence and control
    let system_manager = Arc::new(SystemManager::new(position_manager.clone()));
    
    // Load saved state (settings, mode, running status)
    match system_manager.load_state().await {
        Ok(state) => {
            info!("✅ System state loaded: running={}, mode={:?}", state.is_running, state.mode);
            
            // Apply loaded settings to position manager
            system_manager.update_settings(state.trading_settings).await
                .expect("Failed to apply loaded settings");
            
            // Warning if system was running in Live mode
            if state.is_running && state.mode == crate::core::TradingMode::Live {
                tracing::warn!("⚠️ System was running in LIVE mode. Confirm to continue or stop trading.");
            }
        }
        Err(e) => {
            tracing::warn!("Failed to load system state: {}. Using defaults.", e);
        }
    }
    
    // Create WebSocket server with position manager, system manager and health monitoring
    let ws_server = WsServer::new(state_rx, position_manager, system_manager)
        .with_health(system_health.clone());
    let app = ws_server.router();
    
    // Start WebSocket server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    info!("🌐 Starting WebSocket server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Start price feed
    info!("📊 Starting price feeds...");
    let feed_handle = tokio::spawn(async move {
        if let Err(e) = price_feed.start().await {
            tracing::error!("Price feed error: {}", e);
        }
    });
    
    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
        _ = server_handle => {
            info!("Server task ended");
        }
        _ = feed_handle => {
            info!("Feed task ended");
        }
    }
    
    // Graceful shutdown: даём время на завершение активных операций
    info!("Waiting for active operations to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    info!("Shutdown complete");
    Ok(())
}
