/// ИСПРАВЛЕННАЯ ВЕРСИЯ: Синхронизатор цен с атомарным доступом
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub struct PricePoint {
    pub timestamp: i64,
    pub price: f64,
    pub exchange_ts: i64,
}

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
    #[inline]
    pub fn add_binance_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        let real_time = exchange_ts + (local_received - exchange_ts);
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
    #[inline]
    pub fn add_mexc_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        let real_time = exchange_ts + (local_received - exchange_ts);
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
    
    /// ИСПРАВЛЕНО: Атомарное получение всех данных одновременно
    /// Возвращает: (spread, lag, binance_price, mexc_price)
    #[inline]
    pub fn get_synchronized_data(&self) -> Option<(f64, i64, f64, f64)> {
        if self.binance_prices.is_empty() || self.mexc_prices.is_empty() {
            return None;
        }
        
        let binance_last = self.binance_prices.back()?;
        let mexc_last = self.mexc_prices.back()?;
        
        let spread = mexc_last.price - binance_last.price;
        let lag = mexc_last.timestamp - binance_last.timestamp;
        
        Some((spread, lag, binance_last.price, mexc_last.price))
    }
    
    /// Находит разницу в ценах в один и тот же момент времени
    #[inline]
    pub fn get_synchronized_spread(&self) -> Option<f64> {
        self.get_synchronized_data().map(|(spread, _, _, _)| spread)
    }
    
    /// Находит лаг MEXC относительно Binance
    #[inline]
    pub fn get_mexc_lag(&self) -> Option<i64> {
        self.get_synchronized_data().map(|(_, lag, _, _)| lag)
    }
    
    /// Возвращает количество сохранённых точек
    #[inline]
    pub fn stats(&self) -> (usize, usize) {
        (self.binance_prices.len(), self.mexc_prices.len())
    }
    
    /// НОВОЕ: Получает последние N точек для анализа тренда
    pub fn get_last_n_binance(&self, n: usize) -> Vec<PricePoint> {
        self.binance_prices.iter()
            .rev()
            .take(n)
            .copied()
            .collect()
    }
    
    /// НОВОЕ: Получает последние N точек MEXC для анализа тренда
    pub fn get_last_n_mexc(&self, n: usize) -> Vec<PricePoint> {
        self.mexc_prices.iter()
            .rev()
            .take(n)
            .copied()
            .collect()
    }
}
