/// Сопоставитель цен для arbitrage между биржами
/// Находит пары цен в один и тот же момент реального времени
use crate::core::{TimeSync, RingBuffer, PriceSnapshot};

#[derive(Debug, Clone)]
pub struct PriceMatcher {
    binance_prices: RingBuffer,
    mexc_prices: RingBuffer,
    time_sync: TimeSync,
    max_time_diff_ms: i64,
}

impl PriceMatcher {
    pub fn new(capacity: usize, max_time_diff_ms: i64) -> Self {
        Self {
            binance_prices: RingBuffer::new(capacity),
            mexc_prices: RingBuffer::new(capacity),
            time_sync: TimeSync::new(),
            max_time_diff_ms,
        }
    }
    
    /// Добавляет цену Binance
    pub fn add_binance_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.time_sync.update_binance(exchange_ts, local_received);
        
        self.binance_prices.push(PriceSnapshot {
            price,
            exchange_timestamp: exchange_ts,
            local_received,
        });
    }
    
    /// Добавляет цену MEXC
    pub fn add_mexc_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.time_sync.update_mexc(exchange_ts, local_received);
        
        self.mexc_prices.push(PriceSnapshot {
            price,
            exchange_timestamp: exchange_ts,
            local_received,
        });
    }
    
    /// Находит лучшую пару цен для arbitrage
    /// Возвращает (binance_price, mexc_price, time_diff_ms, mexc_lag_ms)
    pub fn find_best_pair(&self) -> Option<(f64, f64, i64, i64)> {
        let binance_last = self.binance_prices.last()?;
        let mexc_last = self.mexc_prices.last()?;
        
        // Real time = exchange_ts + network_delay
        let binance_real = binance_last.real_time();
        let mexc_real = mexc_last.real_time();
        
        // Вычисляем разницу в реальном времени
        let time_diff = mexc_real - binance_real; // Положительное если MEXC позже
        
        // Если разница слишком большая, не считаем это valid парой
        if time_diff.abs() > self.max_time_diff_ms {
            return None;
        }
        
        // Вычисляем лаг MEXC (положительный если отстаёт)
        let mexc_lag = time_diff;
        
        Some((binance_last.price, mexc_last.price, time_diff, mexc_lag))
    }
    
    /// Находит ближайшую по времени пару цен
    pub fn find_closest_pair(&self) -> Option<(f64, f64, i64, i64)> {
        let binance_prices = self.binance_prices.get_last_n(10);
        let mexc_prices = self.mexc_prices.get_last_n(10);
        
        if binance_prices.is_empty() || mexc_prices.is_empty() {
            return None;
        }
        
        let mut best_pair = None;
        let mut min_time_diff = i64::MAX;
        
        // Простой алгоритм: ищем пару с минимальной разницей в реальном времени
        for binance_snap in &binance_prices {
            for mexc_snap in &mexc_prices {
                let binance_real = binance_snap.real_time();
                let mexc_real = mexc_snap.real_time();
                let time_diff = (mexc_real - binance_real).abs();
                
                if time_diff < min_time_diff && time_diff <= self.max_time_diff_ms {
                    min_time_diff = time_diff;
                    let mexc_lag = mexc_real - binance_real;
                    best_pair = Some((binance_snap.price, mexc_snap.price, time_diff, mexc_lag));
                }
            }
        }
        
        best_pair
    }
    
    /// Возвращает синхронизированный spread
    pub fn get_synchronized_spread(&self) -> Option<f64> {
        if let Some((binance_price, mexc_price, time_diff, _)) = self.find_best_pair() {
            if time_diff.abs() <= self.max_time_diff_ms {
                let spread = ((mexc_price - binance_price) / binance_price) * 100.0;
                return Some(spread);
            }
        }
        None
    }
    
    /// Возвращает лаг MEXC (положительный если отстаёт)
    pub fn get_mexc_lag(&self) -> Option<i64> {
        self.find_best_pair().map(|(_, _, _, lag)| lag)
    }
    
    pub fn is_calibrated(&self) -> bool {
        self.time_sync.is_calibrated()
    }
    
    pub fn get_binance_count(&self) -> usize {
        self.binance_prices.len()
    }
    
    pub fn get_mexc_count(&self) -> usize {
        self.mexc_prices.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn current_timestamp_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64
    }

    #[test]
    fn test_mexc_lag_calculation() {
        let mut matcher = PriceMatcher::new(100, 1000);
        let base_time = 1_000_000_000; // Большое число для теста
        
        // Симуляция: MEXC отстаёт на 100ms
        // Binance: быстрая доставка (50ms)
        // MEXC: медленная доставка (150ms)
        
        // Добавляем несколько цен для калибровки
        for i in 0..30 {
            let time = base_time + i * 10;
            // Binance: exchange_ts = time, received = time + 50
            matcher.add_binance_price(50000.0 + i as f64, time, time + 50);
            // MEXC: exchange_ts = time, received = time + 150 (на 100ms позже)
            matcher.add_mexc_price(50100.0 + i as f64, time, time + 150);
        }
        
        // Проверяем что система калибрована
        assert!(matcher.is_calibrated(), "System should be calibrated");
        
        // Добавляем последние цены для анализа
        let test_time = base_time + 1000;
        matcher.add_binance_price(51000.0, test_time, test_time + 50);
        matcher.add_mexc_price(51100.0, test_time, test_time + 150);
        
        // Проверяем лаг MEXC
        if let Some(lag) = matcher.get_mexc_lag() {
            // MEXC должен отставать примерно на 100ms
            // Допускаем погрешность из-за калибровки
            assert!(lag > 50, "MEXC should lag by at least 50ms, got {}ms", lag);
            assert!(lag < 200, "MEXC lag should be less than 200ms, got {}ms", lag);
            println!("✅ MEXC lag: {}ms (expected ~100ms)", lag);
        } else {
            panic!("Failed to calculate MEXC lag");
        }
        
        // Проверяем spread
        if let Some(spread) = matcher.get_synchronized_spread() {
            // Цена MEXC выше на 100 USD при цене 51000
            // spread = (51100 - 51000) / 51000 * 100% = 0.196%
            let expected_spread = 0.196; // примерно
            let diff = (spread - expected_spread).abs();
            assert!(diff < 0.1, "Spread should be ~0.196%, got {:.3}%", spread);
            println!("✅ Spread: {:.3}% (expected ~0.196%)", spread);
        } else {
            panic!("Failed to calculate synchronized spread");
        }
    }

    #[test]
    fn test_no_lag_scenario() {
        let mut matcher = PriceMatcher::new(100, 1000);
        let base_time = 1_000_000_000;
        
        // Симуляция: обе биржи одинаково быстрые
        for i in 0..30 {
            let time = base_time + i * 10;
            matcher.add_binance_price(50000.0 + i as f64, time, time + 50);
            matcher.add_mexc_price(50000.0 + i as f64, time, time + 50);
        }
        
        // Добавляем тестовые цены
        let test_time = base_time + 1000;
        matcher.add_binance_price(51000.0, test_time, test_time + 50);
        matcher.add_mexc_price(51000.0, test_time, test_time + 50);
        
        if let Some(lag) = matcher.get_mexc_lag() {
            // Лаг должен быть близок к 0
            assert!(lag.abs() < 50, "Lag should be close to 0, got {}ms", lag);
            println!("✅ No lag scenario: {}ms (close to 0)", lag);
        }
    }

    #[test]
    fn test_negative_lag_scenario() {
        let mut matcher = PriceMatcher::new(100, 1000);
        let base_time = 1_000_000_000;
        
        // Симуляция: MEXC быстрее Binance (маловероятно, но тестируем)
        for i in 0..30 {
            let time = base_time + i * 10;
            // Binance медленнее
            matcher.add_binance_price(50000.0 + i as f64, time, time + 150);
            // MEXC быстрее
            matcher.add_mexc_price(50000.0 + i as f64, time, time + 50);
        }
        
        let test_time = base_time + 1000;
        matcher.add_binance_price(51000.0, test_time, test_time + 150);
        matcher.add_mexc_price(51000.0, test_time, test_time + 50);
        
        if let Some(lag) = matcher.get_mexc_lag() {
            // MEXC быстрее, значит lag отрицательный
            assert!(lag < 0, "MEXC should be faster (negative lag), got {}ms", lag);
            println!("✅ Negative lag scenario: {}ms (MEXC faster)", lag);
        }
    }
}
