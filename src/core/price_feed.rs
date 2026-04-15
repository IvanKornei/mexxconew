use tokio::sync::watch;
use tracing::{info, warn};
use std::sync::Arc;

use crate::core::{PriceState, PositionManager};
use crate::exchanges::{BinanceFuturesConnector, MexcFuturesConnector};
use crate::utils::{Result, HealthChecker};

pub struct PriceFeedManager {
    binance_connector: BinanceFuturesConnector,
    mexc_connector: MexcFuturesConnector,
    state_tx: watch::Sender<PriceState>,
    stale_timeout_ms: u64,
    position_manager: Arc<PositionManager>,
    label: String,
}

impl PriceFeedManager {
    pub fn new(
        binance_url: String,
        mexc_url: String,
        mexc_symbol: String,
        label: String,
        stale_timeout_ms: u64,
        initial_capital: f64,
        position_size_percent: f64,
        leverage: f64,
        max_positions: usize,
    ) -> (Self, watch::Receiver<PriceState>, Arc<PositionManager>) {
        let (state_tx, state_rx) = watch::channel(PriceState::default());

        // Создаём менеджер позиций с настройками из конфига
        let position_manager = Arc::new(PositionManager::new(
            initial_capital,
            position_size_percent,
            leverage,
            max_positions,
            mexc_symbol.clone(),
            label.clone(),
        ));

        let manager = Self {
            binance_connector: BinanceFuturesConnector::new(binance_url),
            mexc_connector: MexcFuturesConnector::new(mexc_url, mexc_symbol),
            state_tx,
            stale_timeout_ms,
            position_manager: position_manager.clone(),
            label,
        };

        (manager, state_rx, position_manager)
    }

    /// Создаёт новый менеджер с health checkers для мониторинга
    pub fn new_with_health(
        binance_url: String,
        mexc_url: String,
        mexc_symbol: String,
        label: String,
        stale_timeout_ms: u64,
        initial_capital: f64,
        position_size_percent: f64,
        leverage: f64,
        max_positions: usize,
        binance_health: HealthChecker,
        mexc_health: HealthChecker,
    ) -> (Self, watch::Receiver<PriceState>, Arc<PositionManager>) {
        let (state_tx, state_rx) = watch::channel(PriceState::default());

        // Создаём менеджер позиций с настройками из конфига
        let position_manager = Arc::new(PositionManager::new(
            initial_capital,
            position_size_percent,
            leverage,
            max_positions,
            mexc_symbol.clone(),
            label.clone(),
        ));

        let manager = Self {
            binance_connector: BinanceFuturesConnector::new(binance_url)
                .with_health_checker(binance_health),
            mexc_connector: MexcFuturesConnector::new(mexc_url, mexc_symbol)
                .with_health_checker(mexc_health),
            state_tx,
            stale_timeout_ms,
            position_manager: position_manager.clone(),
            label,
        };

        (manager, state_rx, position_manager)
    }
    
    pub async fn start(self) -> Result<()> {
        info!("Starting Price Feed Manager with Position Manager");
        
        let state_tx_binance = self.state_tx.clone();
        let state_tx_mexc = self.state_tx.clone();
        let state_tx_stale = self.state_tx.clone();
        let stale_timeout = self.stale_timeout_ms;
        let position_manager = self.position_manager.clone();
        let mut state_rx = self.state_tx.subscribe();
        
        // Spawn Binance feed task
        let binance_handle = tokio::spawn(async move {
            let result = self.binance_connector.connect_and_stream(move |price, timestamp| {
                state_tx_binance.send_modify(|state| {
                    state.update_binance(price, timestamp);
                });
            }).await;
            
            if let Err(e) = result {
                warn!("Binance feed ended with error: {}", e);
            }
        });
        
        // Spawn MEXC feed task
        let mexc_handle = tokio::spawn(async move {
            let result = self.mexc_connector.connect_and_stream(move |price, timestamp| {
                state_tx_mexc.send_modify(|state| {
                    state.update_mexc(price, timestamp);
                });
            }).await;
            
            if let Err(e) = result {
                warn!("MEXC feed ended with error: {}", e);
            }
        });
        
        // Spawn trading strategy task - обрабатывает каждое обновление цены
        let position_manager_trading = position_manager.clone();
        let trading_handle = tokio::spawn(async move {
            info!("🤖 Trading strategy started");
            while state_rx.changed().await.is_ok() {
                let state = state_rx.borrow().clone();
                
                // Обрабатываем состояние рынка
                position_manager_trading.process_market_state(&state).await;
            }
            info!("Trading strategy ended");
        });
        
        // Spawn stale checker task - снижена частота с 100ms до 500ms
        // для уменьшения contention на watch::channel
        let stale_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                interval.tick().await;
                state_tx_stale.send_modify(|state| {
                    state.check_stale(stale_timeout);
                });
            }
        });
        
        // Spawn metrics reporter task - выводит метрики каждые 30 секунд
        let position_manager_metrics = position_manager.clone();
        let metrics_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                interval.tick().await;
                
                info!("📊 === Performance Metrics ===");
                
                let market_state_snapshot = position_manager_metrics.get_market_state_metrics();
                if market_state_snapshot.total_messages > 0 {
                    info!("  Market State Processing:");
                    info!("    Total: {} | Avg: {}μs", 
                          market_state_snapshot.total_messages,
                          market_state_snapshot.avg_latency_us);
                    info!("    <10μs: {}% | 10-50μs: {}% | 50-100μs: {}% | >100μs: {}%",
                          (market_state_snapshot.bucket_0_10us * 100) / market_state_snapshot.total_messages,
                          (market_state_snapshot.bucket_10_50us * 100) / market_state_snapshot.total_messages,
                          (market_state_snapshot.bucket_50_100us * 100) / market_state_snapshot.total_messages,
                          ((market_state_snapshot.bucket_100_500us + 
                            market_state_snapshot.bucket_500_1ms + 
                            market_state_snapshot.bucket_1ms_plus) * 100) / market_state_snapshot.total_messages);
                }
                
                let position_update_snapshot = position_manager_metrics.get_position_update_metrics();
                if position_update_snapshot.total_messages > 0 {
                    info!("  Position Updates:");
                    info!("    Total: {} | Avg: {}μs", 
                          position_update_snapshot.total_messages,
                          position_update_snapshot.avg_latency_us);
                }
                
                // Сбрасываем метрики для следующего интервала
                position_manager_metrics.reset_metrics();
            }
        });
        
        // Wait for all tasks
        tokio::select! {
            _ = binance_handle => info!("Binance feed task ended"),
            _ = mexc_handle => info!("MEXC feed task ended"),
            _ = trading_handle => info!("Trading task ended"),
            _ = stale_handle => info!("Stale checker task ended"),
            _ = metrics_handle => info!("Metrics reporter task ended"),
        }
        
        Ok(())
    }
}
