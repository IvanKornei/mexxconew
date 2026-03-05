use serde::Serialize;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::core::TimeSync;

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
        
        // Обновляем синхронизацию времени
        self.time_sync.update_binance(exchange_timestamp, now);
        
        self.recalculate_derived();
    }
    
    #[inline(always)]
    pub fn update_mexc(&mut self, price: f64, exchange_timestamp: i64) {
        let now = current_timestamp_ms();
        self.mexc = price;
        self.mexc_timestamp = exchange_timestamp;
        self.mexc_received_at = now;
        
        // Обновляем синхронизацию времени
        self.time_sync.update_mexc(exchange_timestamp, now);
        
        self.recalculate_derived();
    }
    
    /// Пересчитывает все производные метрики атомарно
    #[inline(always)]
    fn recalculate_derived(&mut self) {
        let now = current_timestamp_ms();
        self.system_timestamp = now;
        
        // Spread calculation
        if self.binance > 0.0 && self.mexc > 0.0 {
            self.spread = ((self.mexc - self.binance) / self.binance) * 100.0;
            self.price_diff = self.mexc - self.binance;
            self.price_diff_percent = (self.price_diff / self.binance) * 100.0;
        }
        
        // Вычисляем лаг MEXC используя синхронизацию времени
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
        
        // Анализ возможности скальпинга
        // ИСПОЛЬЗУЕМ mexc_lag_ms (синхронизированный лаг)
        const MIN_LAG_MS: i64 = 100;
        const MIN_IMPULSE_PERCENT: f64 = 0.01;
        const MEXC_FEE_PERCENT: f64 = 0.0;
        const SLIPPAGE_PERCENT: f64 = 0.005;
        const TOTAL_COST_PERCENT: f64 = MEXC_FEE_PERCENT + SLIPPAGE_PERCENT;
        
        // Торгуем только когда MEXC отстаёт (положительный лаг)
        if self.mexc_lag_ms > MIN_LAG_MS {
            self.potential_profit_percent = self.price_diff_percent.abs() - TOTAL_COST_PERCENT;
            self.is_scalping_opportunity = self.potential_profit_percent > MIN_IMPULSE_PERCENT;
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
}

#[inline]
pub fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
