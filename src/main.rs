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

use crate::api::{SymbolContext, WsServer};
use crate::core::{PositionManager, PriceFeedManager, SystemManager, TradingModeManager};
use crate::exchanges::client::ExchangeClient;
use crate::exchanges::mexc_client::MexcClient;
use crate::utils::{Config, SystemHealth, HealthChecker};

/// Описание одной торговой пары: лейбл для UI/логов,
/// URL Binance WebSocket и символ на MEXC.
struct SymbolSpec {
    label: &'static str,
    binance_ws: &'static str,
    mexc_symbol: &'static str,
}

const SYMBOL_SPECS: &[SymbolSpec] = &[
    SymbolSpec {
        label: "BTC",
        binance_ws: "wss://fstream.binance.com/ws/btcusdt@aggTrade",
        mexc_symbol: "BTC_USDT",
    },
    SymbolSpec {
        label: "ETH",
        binance_ws: "wss://fstream.binance.com/ws/ethusdt@aggTrade",
        mexc_symbol: "ETH_USDT",
    },
    SymbolSpec {
        label: "SOL",
        binance_ws: "wss://fstream.binance.com/ws/solusdt@aggTrade",
        mexc_symbol: "SOL_USDT",
    },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize rustls crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging system
    utils::logging::init_logging(Level::INFO)?;

    info!("🚀 Starting HFT Arbitrage System (multi-symbol)");

    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let config = Config::load_from_file("config.toml")?;
    info!("✅ Configuration loaded successfully");

    // Create health monitoring system
    let mut system_health = SystemHealth::new();

    // Для каждого символа поднимаем отдельный PriceFeedManager с собственными health checker'ами.
    let mut symbol_contexts: Vec<SymbolContext> = Vec::with_capacity(SYMBOL_SPECS.len());
    let mut position_managers: Vec<Arc<PositionManager>> = Vec::with_capacity(SYMBOL_SPECS.len());
    let mut feed_handles: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(SYMBOL_SPECS.len());

    for spec in SYMBOL_SPECS {
        let binance_health = HealthChecker::new(
            format!("binance-{}", spec.label.to_lowercase()),
            Duration::from_secs(30),
        );
        let mexc_health = HealthChecker::new(
            format!("mexc-{}", spec.label.to_lowercase()),
            Duration::from_secs(30),
        );

        system_health.add_component(binance_health.clone_checker());
        system_health.add_component(mexc_health.clone_checker());

        let (price_feed, state_rx, position_manager) = PriceFeedManager::new_with_health(
            spec.binance_ws.to_string(),
            config.exchanges.mexc_ws.clone(),
            spec.mexc_symbol.to_string(),
            spec.label.to_string(),
            config.monitoring.stale_timeout_ms,
            config.trading.initial_capital,
            config.trading.position_size_percent,
            config.trading.leverage,
            config.trading.max_positions,
            binance_health,
            mexc_health,
        );

        info!("✅ Price feed manager initialized for {}", spec.label);

        symbol_contexts.push(SymbolContext {
            label: spec.label.to_string(),
            state_rx,
            position_manager: position_manager.clone(),
        });
        position_managers.push(position_manager);

        // Стартуем feed в фоне
        let label = spec.label.to_string();
        feed_handles.push(tokio::spawn(async move {
            info!("📊 Starting price feed for {}", label);
            if let Err(e) = price_feed.start().await {
                tracing::error!("Price feed [{}] error: {}", label, e);
            }
        }));
    }

    let system_health = Arc::new(system_health);

    // Create trading mode manager for separate emulation/live databases
    let trading_mode_manager = Arc::new(
        TradingModeManager::new(config.trading.initial_capital)
            .expect("Failed to create trading mode manager")
    );
    info!("✅ Trading mode manager initialized");

    // Connect trading mode manager to all position managers
    for pm in &position_managers {
        pm.set_trading_mode_manager(trading_mode_manager.clone()).await;
    }

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

        // Sanity check: дергаем баланс, чтобы убедиться что ключи рабочие.
        // Если биржа отвечает ошибкой — НЕ подключаем клиент, чтобы случайный
        // переход в Live не попытался торговать с невалидными ключами.
        match mexc_client.get_balance("USDT").await {
            Ok(balance) => {
                info!(
                    "✅ MEXC API keys validated | USDT free: {} | locked: {}",
                    balance.free, balance.locked
                );
                // Привязываем один и тот же клиент ко всем position managers
                for pm in &position_managers {
                    pm.set_mexc_client(mexc_client.clone()).await;
                }
                info!("✅ MEXC API client connected to all symbols (Live trading ready)");
            }
            Err(e) => {
                tracing::error!(
                    "❌ MEXC API key validation failed: {}. Live trading disabled. \
                     Emulation (paper) mode will still work.",
                    e
                );
            }
        }
    } else {
        tracing::warn!(
            "⚠️ MEXC_API_KEY/MEXC_API_SECRET not set (or placeholder) — \
             Live trading disabled. Emulation (paper) mode will still work."
        );
    }

    // Enable trading by default for emulation mode
    for pm in &position_managers {
        pm.set_trading_enabled(true).await;
    }
    info!("✅ Trading enabled for emulation mode on {} symbols", position_managers.len());

    // Create system manager for state persistence and control (fan-out over all PMs)
    let system_manager = Arc::new(SystemManager::new(position_managers.clone()));

    // Load saved state (settings, mode, running status)
    match system_manager.load_state().await {
        Ok(state) => {
            info!("✅ System state loaded: running={}, mode={:?}", state.is_running, state.mode);

            // Apply loaded settings to all position managers (через system manager)
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

    // Create WebSocket server with multi-symbol contexts and health monitoring
    let ws_server = WsServer::new(symbol_contexts, system_manager)
        .with_health(system_health.clone());
    let app = ws_server.router();

    // Start WebSocket server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    info!("🌐 Starting WebSocket server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for Ctrl+C or any feed/server task ending
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
        _ = server_handle => {
            info!("Server task ended");
        }
        _ = futures_util::future::select_all(feed_handles) => {
            info!("One of the feed tasks ended");
        }
    }

    // Graceful shutdown: даём время на завершение активных операций
    info!("Waiting for active operations to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    info!("Shutdown complete");
    Ok(())
}
