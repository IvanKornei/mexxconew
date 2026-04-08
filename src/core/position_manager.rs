use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::trading_strategy::{ImpulseStrategy, Position};
use crate::core::capital_manager::CapitalManager;
use crate::core::{TradingStats, TradeRecord};
use crate::core::PriceState;
use crate::utils::LatencyMetrics;
use crate::core::system_manager_optimized::TradingMode;
use crate::core::trading_mode_manager::TradingModeManager;
use crate::exchanges::binance_client::BinanceClient;
use crate::exchanges::client::{ExchangeClient, OrderRequest, OrderSide, OrderType};
use crate::exchanges::mexc_client::MexcClient;
use rust_decimal::Decimal;

/// Менеджер позиций с управлением капиталом
pub struct PositionManager {
    strategy: Arc<RwLock<ImpulseStrategy>>,
    positions: Arc<RwLock<HashMap<String, Position>>>,
    closed_trades: Arc<RwLock<VecDeque<TradeRecord>>>,
    capital_manager: Arc<RwLock<CapitalManager>>,
    max_positions: Arc<RwLock<usize>>,
    active_count: Arc<AtomicUsize>,
    
    // Управление торговлей
    is_trading_enabled: Arc<RwLock<bool>>,
    execution_mode: Arc<RwLock<TradingMode>>,
    
    // Менеджер режимов торговли
    trading_mode_manager: Arc<RwLock<Option<Arc<TradingModeManager>>>>,
    
    // Клиенты для исполнения ордеров (прямые REST API)
    mexc_client: Arc<RwLock<Option<Arc<MexcClient>>>>,
    binance_client: Arc<RwLock<Option<Arc<BinanceClient>>>>,
    
    // Метрики производительности
    market_state_metrics: Arc<LatencyMetrics>,
    position_update_metrics: Arc<LatencyMetrics>,
}

impl PositionManager {
    pub fn new(capital: f64, position_size_percent: f64, leverage: f64, max_positions: usize) -> Self {
        info!("💰 Position manager initialized | Capital: ${} | Position: {}% | Leverage: {}x", 
              capital, position_size_percent, leverage);
        
        let capital_manager = CapitalManager::new(capital, position_size_percent / 100.0, leverage);
        
        Self {
            strategy: Arc::new(RwLock::new(ImpulseStrategy::default())),
            positions: Arc::new(RwLock::new(HashMap::new())),
            closed_trades: Arc::new(RwLock::new(VecDeque::new())),
            capital_manager: Arc::new(RwLock::new(capital_manager)),
            max_positions: Arc::new(RwLock::new(max_positions)),
            active_count: Arc::new(AtomicUsize::new(0)),
            is_trading_enabled: Arc::new(RwLock::new(false)),
            execution_mode: Arc::new(RwLock::new(TradingMode::Emulation)),
            trading_mode_manager: Arc::new(RwLock::new(None)),
            mexc_client: Arc::new(RwLock::new(None)),
            binance_client: Arc::new(RwLock::new(None)),
            market_state_metrics: Arc::new(LatencyMetrics::new()),
            position_update_metrics: Arc::new(LatencyMetrics::new()),
        }
    }
    
    /// Устанавливает MEXC REST клиент для реальных ордеров
    pub async fn set_mexc_client(&self, client: Arc<MexcClient>) {
        *self.mexc_client.write().await = Some(client);
        info!("📡 MEXC API client connected to position manager");
    }

    /// Устанавливает Binance client
    pub async fn set_binance_client(&self, client: Arc<BinanceClient>) {
        *self.binance_client.write().await = Some(client);
        info!("📡 Binance client connected to position manager");
    }
    
    /// Устанавливает TradingModeManager для записи сделок в правильную БД
    pub async fn set_trading_mode_manager(&self, manager: Arc<TradingModeManager>) {
        *self.trading_mode_manager.write().await = Some(manager);
        info!("📊 Trading mode manager connected to position manager");
    }
    
    pub async fn process_market_state(&self, state: &PriceState) {
        // ПРОВЕРКА: Если торговля отключена, игнорируем все сигналы
        let is_enabled = *self.is_trading_enabled.read().await;
        if !is_enabled {
            // Логируем только первый раз чтобы не спамить
            static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                info!("⏸️ Trading is disabled, ignoring market signals");
            }
            return;
        }
        
        // Начинаем измерение производительности
        let start = Instant::now();
        
        // Обновляем стратегию БЕЗ долгой блокировки
        // Копируем данные для минимизации времени под блокировкой
        let binance_price = state.binance;
        let mexc_price = state.mexc;
        let binance_ts = state.binance_timestamp;
        let mexc_ts = state.mexc_timestamp;
        let lag = state.mexc_lag_ms;
        
        {
            let mut strategy = self.strategy.write().await;
            strategy.update_binance_price(binance_price, binance_ts);
            strategy.update_mexc_price(mexc_price, mexc_ts);
            strategy.update_lag_stats(lag);
        }
        
        // Обновляем позиции - создаём snapshot для избежания race condition
        let positions_to_update: Vec<(String, Position)> = {
            let positions = self.positions.read().await;
            positions.iter().map(|(id, pos)| (id.clone(), pos.clone())).collect()
        };
        
        let mut to_close = Vec::new();
        let mut to_save = Vec::new();
        
        for (id, mut position) in positions_to_update {
            let update_start = Instant::now();
            
            let strategy = self.strategy.read().await;
            let should_close = strategy.update_position(&mut position, state.mexc);
            
            // Записываем метрику обновления позиции
            self.position_update_metrics.record(update_start);
            
            if should_close {
                // PnL в USD (разница цен * количество BTC)
                // Leverage УЖЕ учтен в размере позиции (quantity), не нужно умножать еще раз!
                let pnl = strategy.calculate_pnl(&position);
                let pnl_percent = strategy.calculate_pnl_percent(&position);
                let duration_ms = chrono::Utc::now().timestamp_millis() - position.entry_time;
                
                info!(
                    "💰 CLOSED | {} | {:?} | Entry: {:.2} | Exit: {:.2} | PnL: ${:.2} ({:.3}%) | {:?} | {}ms",
                    id, position.side, position.entry_price, position.current_price,
                    pnl, pnl_percent, position.status, duration_ms
                );
                
                // Записываем сделку через TradingModeManager если он подключен
                let mode_manager = self.trading_mode_manager.read().await;
                if let Some(ref manager) = *mode_manager {
                    manager.record_trade(&position, position.current_price, pnl, pnl_percent).await;
                }
                drop(mode_manager);
                
                // Сохраняем в память (для обратной совместимости)
                let trade = TradeRecord {
                    id: 0,
                    position_id: position.id.clone(),
                    side: format!("{:?}", position.side),
                    entry_price: position.entry_price,
                    exit_price: position.current_price,
                    quantity: position.quantity,
                    entry_time: position.entry_time,
                    exit_time: chrono::Utc::now().timestamp_millis(),
                    pnl,
                    pnl_percent,
                    status: format!("{:?}", position.status),
                    initial_impulse: position.initial_impulse,
                };
                
                let mut trades = self.closed_trades.write().await;
                trades.push_front(trade);
                // Храним ВСЕ сделки за сессию (без лимита)
                
                // Обновляем капитал с учётом комиссий
                let mut capital = self.capital_manager.write().await;
                let position_value_usd = position.entry_price * position.quantity;
                capital.update_capital(pnl, position_value_usd);
                
                let current_capital = capital.get_current_capital();
                let fee = capital.calculate_fee(position_value_usd);
                let cumulative_volume = capital.get_cumulative_volume();
                let taker_fee_percent = capital.get_taker_fee() * 100.0;
                
                info!(
                    "💵 Capital: ${:.2} | PnL: ${:.2} | Fee: ${:.4} ({:.2}%) | Net: ${:.2} | Volume: ${:.0} | Return: {:.2}%",
                    current_capital, pnl, fee, taker_fee_percent, pnl - fee, cumulative_volume, capital.get_return_percent()
                );
                
                // Проверяем достижение $1M объёма
                if cumulative_volume >= 1_000_000.0 && cumulative_volume - position_value_usd * 2.0 < 1_000_000.0 {
                    info!("🎉 Volume milestone reached! Fee reduced to 0.01%");
                }
                
                // Проверяем ликвидацию
                if capital.is_liquidated() {
                    info!("🔥 LIQUIDATED! Capital depleted.");
                }
                
                // Уменьшаем счётчик
                self.active_count.fetch_sub(1, Ordering::SeqCst);
                
                to_close.push(id);
            } else {
                to_save.push((id, position));
            }
        }
        
        // Применяем изменения
        {
            let mut positions = self.positions.write().await;
            for id in to_close {
                positions.remove(&id);
            }
            for (id, position) in to_save {
                positions.insert(id, position);
            }
        }
        
        // Проверяем вход с атомарной защитой
        let max_positions = *self.max_positions.read().await;
        let current_count = self.active_count.load(Ordering::SeqCst);
        
        if current_count < max_positions {
            // Проверяем что цены валидные (не 0 и не NaN)
            if state.mexc <= 0.0 || state.mexc.is_nan() || state.binance <= 0.0 || state.binance.is_nan() {
                // Цены еще не получены, пропускаем
                return;
            }
            
            // Проверяем достаточно ли капитала
            let capital = self.capital_manager.read().await;
            let current_capital = capital.get_current_capital();
            let position_size_percent = capital.get_position_size_percent();
            let can_open = capital.can_open_position(state.mexc);
            
            if !can_open {
                info!("⚠️ Insufficient capital to open position | Balance: ${:.2} | Position %: {:.1}% | Required: ${:.2}", 
                      current_capital, 
                      position_size_percent * 100.0,
                      current_capital * position_size_percent);
                drop(capital);
                return;
            }
            
            // Рассчитываем размер позиции исходя из капитала
            let position_size_btc = capital.calculate_position_size(state.mexc);
            drop(capital);
            
            // Проверяем есть ли уже открытые позиции
            let existing_positions = self.positions.read().await;
            
            // Пытаемся увеличить счётчик
            let prev = self.active_count.fetch_add(1, Ordering::SeqCst);
            
            // Проверяем что мы не превысили лимит
            if prev >= max_positions {
                // Откатываем
                self.active_count.fetch_sub(1, Ordering::SeqCst);
                return;
            }
            
            let mut strategy = self.strategy.write().await;
            
            if let Some(side) = strategy.check_entry_signal(state.binance, state.mexc) {
                // Проверяем нет ли уже позиции в том же направлении
                let has_same_direction = existing_positions.values().any(|pos| pos.side == side);
                
                if has_same_direction {
                    // Уже есть позиция в этом направлении - не открываем
                    self.active_count.fetch_sub(1, Ordering::SeqCst);
                    return;
                }
                
                drop(existing_positions);
                
                let avg_lag = strategy.get_average_lag();
                let position = strategy.create_position(
                    state.mexc,
                    position_size_btc,
                    side.clone(),
                    state.price_diff_percent.abs(),
                );
                
                let capital = self.capital_manager.read().await;
                
                // Проверяем режим выполнения
                let execution_mode = *self.execution_mode.read().await;
                match execution_mode {
                    TradingMode::Emulation => {
                        info!(
                            "🚀 OPENED (EMULATION) | {} | {:?} | Entry: {:.2} | Size: {:.6} BTC | Capital: ${:.2} | AvgLag: {}ms",
                            position.id, side, position.entry_price, position_size_btc,
                            capital.get_current_capital(), avg_lag
                        );
                    }
                    TradingMode::Live => {
                        info!(
                            "🚀 OPENING (LIVE) | {} | {:?} | Entry: {:.2} | Size: {:.6} BTC | Capital: ${:.2} | AvgLag: {}ms",
                            position.id, side, position.entry_price, position_size_btc,
                            capital.get_current_capital(), avg_lag
                        );
                        
                        // Исполняем реальный ордер на MEXC
                        match self.execute_live_order(&position).await {
                            Ok(order_id) => {
                                info!("✅ Live order executed on MEXC: {}", order_id);
                            }
                            Err(e) => {
                                warn!("❌ Failed to execute live order: {}. Position will be tracked as emulation.", e);
                                // Продолжаем как emulation если не удалось
                            }
                        }
                    }
                }
                
                let mut positions = self.positions.write().await;
                positions.insert(position.id.clone(), position);
            } else {
                // Сигнала нет - откатываем счётчик
                self.active_count.fetch_sub(1, Ordering::SeqCst);
            }
        }
        
        // Записываем общую метрику обработки market state
        self.market_state_metrics.record(start);
    }
    
    pub async fn get_open_positions(&self) -> Vec<Position> {
        let positions = self.positions.read().await;
        positions.values().cloned().collect()
    }
    
    pub async fn get_stats(&self) -> PositionStats {
        let positions = self.positions.read().await;
        let strategy = self.strategy.read().await;
        
        let total_pnl: f64 = positions.values()
            .map(|p| strategy.calculate_pnl(p))
            .sum();
        
        PositionStats {
            open_positions: positions.len(),
            total_pnl,
        }
    }
    
    pub async fn get_trading_stats(&self) -> Option<TradingStats> {
        let trades = self.closed_trades.read().await;
        
        if trades.is_empty() {
            return None;
        }
        
        let capital = self.capital_manager.read().await;
        
        let total_trades = trades.len() as i64;
        let winning_trades = trades.iter().filter(|t| t.pnl > 0.0).count() as i64;
        let losing_trades = total_trades - winning_trades;
        let total_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
        let win_rate = (winning_trades as f64 / total_trades as f64) * 100.0;
        let avg_profit = trades.iter().map(|t| t.pnl_percent).sum::<f64>() / total_trades as f64;
        let best_trade = trades.iter().map(|t| t.pnl_percent).fold(f64::NEG_INFINITY, f64::max);
        let worst_trade = trades.iter().map(|t| t.pnl_percent).fold(f64::INFINITY, f64::min);
        
        Some(TradingStats {
            total_trades,
            winning_trades,
            losing_trades,
            total_pnl,
            win_rate,
            avg_profit,
            best_trade,
            worst_trade,
            current_balance: capital.get_current_capital(),
            initial_balance: capital.get_initial_capital(),
            total_return_percent: capital.get_return_percent(),
        })
    }
    
    pub async fn get_recent_trades(&self, limit: i64) -> Vec<TradeRecord> {
        let trades = self.closed_trades.read().await;
        trades.iter().take(limit as usize).cloned().collect()
    }
    
    pub async fn get_current_balance(&self) -> f64 {
        let capital = self.capital_manager.read().await;
        capital.get_current_capital()
    }
    
    pub async fn update_strategy_settings(
        &self,
        momentum_weight: f64,
        lag_weight: f64,
        price_diff_weight: f64,
        momentum_threshold: f64,
        quick_exit_timeout_ms: i64,
    ) {
        let mut strategy = self.strategy.write().await;
        strategy.update_settings(
            momentum_weight,
            lag_weight,
            price_diff_weight,
            momentum_threshold,
            quick_exit_timeout_ms,
        );
        info!(
            "⚙️ Settings updated | Momentum: {:.1}% | Lag: {:.1}% | Price: {:.1}%",
            momentum_weight * 100.0,
            lag_weight * 100.0,
            price_diff_weight * 100.0
        );
    }
    
    pub async fn update_capital_settings(
        &self,
        capital: f64,
        position_size_percent: f64,
        leverage: f64,
        max_positions: usize,
    ) {
        // Создаём новый CapitalManager с новыми настройками
        let new_capital_manager = CapitalManager::new(capital, position_size_percent / 100.0, leverage);
        
        let mut capital_manager = self.capital_manager.write().await;
        *capital_manager = new_capital_manager;
        
        // Обновляем max_positions
        let mut max_pos = self.max_positions.write().await;
        *max_pos = max_positions;
        
        info!(
            "💰 Capital settings updated | Capital: ${} | Position: {}% | Leverage: {}x | Max Positions: {}",
            capital, position_size_percent, leverage, max_positions
        );
    }
    
    pub async fn get_strategy_settings(&self) -> crate::core::trading_strategy::StrategySettings {
        let strategy = self.strategy.read().await;
        strategy.get_settings()
    }
    
    /// Получить метрики производительности обработки market state
    pub fn get_market_state_metrics(&self) -> crate::utils::MetricsSnapshot {
        self.market_state_metrics.snapshot()
    }
    
    /// Получить метрики производительности обновления позиций
    pub fn get_position_update_metrics(&self) -> crate::utils::MetricsSnapshot {
        self.position_update_metrics.snapshot()
    }
    
    /// Сбросить метрики производительности
    pub fn reset_metrics(&self) {
        self.market_state_metrics.reset();
        self.position_update_metrics.reset();
    }
    
    /// Устанавливает флаг активности торговли
    pub async fn set_trading_enabled(&self, enabled: bool) {
        *self.is_trading_enabled.write().await = enabled;
        info!("🔧 Trading enabled set to: {}", enabled);
    }
    
    /// Проверяет активна ли торговля
    pub async fn is_trading_enabled(&self) -> bool {
        *self.is_trading_enabled.read().await
    }
    
    /// Устанавливает режим выполнения (Emulation/Live)
    pub async fn set_execution_mode(&self, mode: TradingMode) {
        *self.execution_mode.write().await = mode;
        info!("Execution mode set to: {:?}", mode);
    }
    
    /// Возвращает текущий режим выполнения
    pub async fn get_execution_mode(&self) -> TradingMode {
        *self.execution_mode.read().await
    }
    
    /// Исполняет реальный ордер на MEXC через REST API
    async fn execute_live_order(&self, position: &Position) -> Result<String, String> {
        let side = match position.side {
            crate::core::trading_strategy::PositionSide::Long => OrderSide::Buy,
            crate::core::trading_strategy::PositionSide::Short => OrderSide::Sell,
        };

        let client_guard = self.mexc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| "MEXC API client not configured".to_string())?;

        let quantity = Decimal::from_f64_retain(position.quantity)
            .ok_or_else(|| format!("Invalid quantity: {}", position.quantity))?;

        let order_request = OrderRequest {
            symbol: "BTC_USDT".to_string(),
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
        };

        match client.place_order(order_request).await {
            Ok(order) => Ok(order.id),
            Err(e) => Err(format!("MEXC order failed: {}", e)),
        }
    }
}

pub struct PositionStats {
    pub open_positions: usize,
    pub total_pnl: f64,
}
