use serde::Serialize;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::core::TimeSync;
use crate::core::price_synchronizer::PriceSynchronizer;
use crate::core::arbitrage_decision::ArbitrageDecisionMaker;

#[derive(Debug, Clone, Serialize)]
pub struct PriceState {
    pub binance: f64,
    pub mexc: f64,
    pub spread: f64,
    pub is_stale: bool,
    pub latency_ms: u64,
    pub binance_timestamp: i64,
    pub mexc_timestamp: i64,
    pub system_timestamp: i64,
    
    // Метрики задержки
    pub mexc_lag_ms: i64,
    pub price_diff: f64,
    pub price_diff_percent: f64,
    
    // Реальное время получения данных
    pub binance_received_at: i64,
    pub mexc_received_at: i64,
    pub real_lag_ms: i64,
    
    // Анализ возможности скальпинга
    pub is_scalping_opportunity: bool,
    pub potential_profit_percent: f64,
    
    #[serde(skip)]
    time_sync: TimeSync,
    
    #[serde(skip)]
    price_synchronizer: PriceSynchronizer,
    
    #[serde(skip)]
    decision_maker: ArbitrageDecisionMaker,
}

// История состояний для анализа
#[derive(Debug, Clone, Serialize)]
pub struct PriceHistory {
    pub states: VecDeque<PriceState>,
    max_size: usize,
}

impl PriceHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            states: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    pub fn push(&mut self, state: PriceState) {
        if self.states.len() >= self.max_size {
            self.states.pop_front();
        }
        self.states.push_back(state);
    }
    
    pub fn get_recent(&self, count: usize) -> Vec<PriceState> {
        self.states.iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }
}

impl Default for PriceState {
    fn default() -> Self {
        Self {
            binance: 0.0,
            mexc: 0.0,
            spread: 0.0,
            is_stale: true,
            latency_ms: 0,
            binance_timestamp: 0,
            mexc_timestamp: 0,
            system_timestamp: current_timestamp_ms(),
            mexc_lag_ms: 0,
            price_diff: 0.0,
            price_diff_percent: 0.0,
            binance_received_at: 0,
            mexc_received_at: 0,
            real_lag_ms: 0,
            is_scalping_opportunity: false,
            potential_profit_percent: 0.0,
            time_sync: TimeSync::new(),
            price_synchronizer: PriceSynchronizer::new(100, 1000),
            decision_maker: ArbitrageDecisionMaker::new(100, 1000, 0.05, 0.1, 0.005),
        }
    }
}

impl PriceState {
    #[inline(always)]
    pub fn update_binance(&mut self, price: f64, exchange_timestamp: i64) {
        let now = current_timestamp_ms();
        self.binance = price;
        self.binance_timestamp = exchange_timestamp;
        self.binance_received_at = now;
        
        // Обновляем синхронизацию времени и добавляем в синхронизатор
        self.time_sync.update_binance(exchange_timestamp, now);
        self.price_synchronizer.add_binance_price(price, exchange_timestamp, now);
        self.decision_maker.add_binance_price(price, exchange_timestamp, now);
        
        self.recalculate_derived();
    }
    
    #[inline(always)]
    pub fn update_mexc(&mut self, price: f64, exchange_timestamp: i64) {
        let now = current_timestamp_ms();
        self.mexc = price;
        self.mexc_timestamp = exchange_timestamp;
        self.mexc_received_at = now;
        
        // Обновляем синхронизацию времени и добавляем в синхронизатор
        self.time_sync.update_mexc(exchange_timestamp, now);
        self.price_synchronizer.add_mexc_price(price, exchange_timestamp, now);
        self.decision_maker.add_mexc_price(price, exchange_timestamp, now);
        
        self.recalculate_derived();
    }
    
    /// Пересчитывает все производные метрики атомарно
    #[inline(always)]
    fn recalculate_derived(&mut self) {
        let now = current_timestamp_ms();
        self.system_timestamp = now;
        
        // Простой расчёт spread на основе последних цен
        if self.binance > 0.0 && self.mexc > 0.0 {
            self.spread = ((self.mexc - self.binance) / self.binance) * 100.0;
            self.price_diff = self.mexc - self.binance;
            self.price_diff_percent = self.spread;
        }
        
        // Вычисляем лаг MEXC используя TimeSync
        if self.binance_timestamp > 0 && self.mexc_timestamp > 0 {
            self.mexc_lag_ms = self.time_sync.calculate_mexc_lag(
                self.binance_timestamp,
                self.mexc_timestamp
            );
        }
        
        // Задержка получения данных (для информации)
        if self.binance_received_at > 0 && self.mexc_received_at > 0 {
            self.real_lag_ms = self.mexc_received_at - self.binance_received_at;
        }
        
        // Анализ возможности скальпинга с учётом всех задержек
        if let Some(decision) = self.decision_maker.make_decision() {
            self.is_scalping_opportunity = decision.should_trade;
            self.potential_profit_percent = decision.predicted_profit_percent;
        } else {
            self.is_scalping_opportunity = false;
            self.potential_profit_percent = 0.0;
        }
        
        // Latency calculation
        let latest_exchange_ts = if self.binance_timestamp > self.mexc_timestamp {
            self.binance_timestamp
        } else {
            self.mexc_timestamp
        };
        
        if latest_exchange_ts > 0 {
            let latency = now - latest_exchange_ts;
            self.latency_ms = if latency > 0 { latency as u64 } else { 0 };
        }
    }
    
    pub fn check_stale(&mut self, timeout_ms: u64) {
        let now = current_timestamp_ms();
        let binance_age = now - self.binance_timestamp;
        let mexc_age = now - self.mexc_timestamp;
        
        self.is_stale = binance_age > timeout_ms as i64 || mexc_age > timeout_ms as i64;
    }
    
    /// Возвращает информацию о калибровке
    pub fn get_calibration_info(&self) -> String {
        format!(
            "TimeSync calibrated: {}",
            self.time_sync.is_calibrated()
        )
    }
    
    /// Возвращает анализ возможности арбитража (теперь в ImpulseStrategy)
    pub fn get_arbitrage_analysis(&self) -> String {
        format!(
            "Spread: {:.3}% | Lag: {}ms | Opportunity: {}",
            self.spread,
            self.mexc_lag_ms,
            if self.is_scalping_opportunity { "YES" } else { "NO" }
        )
    }
}

#[inline]
pub fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
