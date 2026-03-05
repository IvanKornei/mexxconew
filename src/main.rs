// HFT-optimized allocator - reduces jitter by 30-50%
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod api;
mod core;
mod exchanges;
mod utils;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::api::WsServer;
use crate::core::PriceFeedManager;
use crate::utils::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize rustls crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Load configuration
    let config = Config::load_from_file("config.toml")?;
    info!("Configuration loaded successfully");
    
    // Create price feed manager
    let (price_feed, state_rx, position_manager) = PriceFeedManager::new(
        config.exchanges.binance_ws.clone(),
        config.exchanges.mexc_ws.clone(),
        config.monitoring.stale_timeout_ms,
        config.trading.initial_capital,
        config.trading.position_size_percent,
        config.trading.leverage,
        config.trading.max_positions,
    );
    
    // Create WebSocket server with position manager
    let ws_server = WsServer::new(state_rx, position_manager);
    let app = ws_server.router();
    
    // Start WebSocket server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    info!("Starting WebSocket server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Start price feed
    info!("Starting price feeds...");
    let feed_handle = tokio::spawn(async move {
        if let Err(e) = price_feed.start().await {
            tracing::error!("Price feed error: {}", e);
        }
    });
    
    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = server_handle => {
            info!("Server task ended");
        }
        _ = feed_handle => {
            info!("Feed task ended");
        }
    }
    
    info!("Shutdown complete");
    Ok(())
}
