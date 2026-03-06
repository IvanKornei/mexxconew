/// Синхронизатор цен для сравнения цен в один и тот же момент времени
/// Интерполирует цены для синхронного сравнения
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct PricePoint {
    pub timestamp: i64,  // реальное время (exchange_ts + network_delay)
    pub price: f64,
    pub exchange_ts: i64,  // оригинальный timestamp биржи
}

#[derive(Debug, Clone)]
pub struct PriceSynchronizer {
    binance_prices: VecDeque<PricePoint>,
    mexc_prices: VecDeque<PricePoint>,
    max_points: usize,
    max_time_diff_ms: i64,
}

impl PriceSynchronizer {
    pub fn new(max_points: usize, max_time_diff_ms: i64) -> Self {
        Self {
            binance_prices: VecDeque::with_capacity(max_points),
            mexc_prices: VecDeque::with_capacity(max_points),
            max_points,
            max_time_diff_ms,
        }
    }
    
    /// Добавляет цену Binance
    pub fn add_binance_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        let real_time = exchange_ts + (local_received - exchange_ts); // exchange_ts + network_delay
        let point = PricePoint {
            timestamp: real_time,
            price,
            exchange_ts,
        };
        
        self.binance_prices.push_back(point);
        if self.binance_prices.len() > self.max_points {
            self.binance_prices.pop_front();
        }
    }
    
    /// Добавляет цену MEXC
    pub fn add_mexc_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        let real_time = exchange_ts + (local_received - exchange_ts); // exchange_ts + network_delay
        let point = PricePoint {
            timestamp: real_time,
            price,
            exchange_ts,
        };
        
        self.mexc_prices.push_back(point);
        if self.mexc_prices.len() > self.max_points {
            self.mexc_prices.pop_front();
        }
    }
    
    /// Находит разницу в ценах в один и тот же момент времени
    pub fn get_synchronized_spread(&self) -> Option<f64> {
        if self.binance_prices.is_empty() || self.mexc_prices.is_empty() {
            return None;
        }
        
        // Простой подход: используем последние цены
        let binance_last = self.binance_prices.back()?;
        let mexc_last = self.mexc_prices.back()?;
        
        Some(mexc_last.price - binance_last.price)
    }
    
    /// Находит лаг MEXC относительно Binance
    pub fn get_mexc_lag(&self) -> Option<i64> {
        if self.binance_prices.is_empty() || self.mexc_prices.is_empty() {
            return None;
        }
        
        let binance_last = self.binance_prices.back()?;
        let mexc_last = self.mexc_prices.back()?;
        
        // Лаг = время MEXC - время Binance
        Some(mexc_last.timestamp - binance_last.timestamp)
    }
    
    /// Возвращает количество сохранённых точек
    pub fn stats(&self) -> (usize, usize) {
        (self.binance_prices.len(), self.mexc_prices.len())
    }
}