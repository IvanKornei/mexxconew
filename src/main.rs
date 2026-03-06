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
use crate::core::PriceFeedManager;
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
    
    // Create WebSocket server with position manager and health monitoring
    let ws_server = WsServer::new(state_rx, position_manager)
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
