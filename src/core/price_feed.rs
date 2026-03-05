use tokio::sync::watch;
use tracing::{info, warn};
use std::sync::Arc;

use crate::core::{PriceState, PositionManager};
use crate::exchanges::{BinanceFuturesConnector, MexcFuturesConnector};
use crate::utils::Result;

pub struct PriceFeedManager {
    binance_connector: BinanceFuturesConnector,
    mexc_connector: MexcFuturesConnector,
    state_tx: watch::Sender<PriceState>,
    stale_timeout_ms: u64,
    position_manager: Arc<PositionManager>,
}

impl PriceFeedManager {
    pub fn new(
        binance_url: String,
        mexc_url: String,
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
        ));
        
        let manager = Self {
            binance_connector: BinanceFuturesConnector::new(binance_url),
            mexc_connector: MexcFuturesConnector::new(mexc_url),
            state_tx,
            stale_timeout_ms,
            position_manager: position_manager.clone(),
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
        
        // Wait for all tasks
        tokio::select! {
            _ = binance_handle => info!("Binance feed task ended"),
            _ = mexc_handle => info!("MEXC feed task ended"),
            _ = trading_handle => info!("Trading task ended"),
            _ = stale_handle => info!("Stale checker task ended"),
        }
        
        Ok(())
    }
}
