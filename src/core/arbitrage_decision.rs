/// Модуль для принятия решений об арбитраже с учётом всех задержек
use crate::core::price_synchronizer::PriceSynchronizer;

#[derive(Debug, Clone)]
pub struct LatencyProfile {
    // Сетевая задержка (RTT/2)
    pub binance_network_ms: f64,
    pub mexc_network_ms: f64,
    
    // Внутренняя задержка бирж (matching engine)
    pub binance_internal_ms: f64,  // 1-5ms
    pub mexc_internal_ms: f64,     // 10-20ms
    
    // Наша задержка на обработку
    pub our_processing_ms: f64,    // 1-5ms
}

impl Default for LatencyProfile {
    fn default() -> Self {
        Self {
            binance_network_ms: 50.0,   // Средняя задержка до Binance
            mexc_network_ms: 150.0,     // Средняя задержка до MEXC
            binance_internal_ms: 3.0,   // Внутренняя задержка Binance
            mexc_internal_ms: 15.0,     // Внутренняя задержка MEXC
            our_processing_ms: 2.0,     // Наша задержка
        }
    }
}

impl LatencyProfile {
    /// Общая задержка для ордера на MEXC
    pub fn total_mexc_latency(&self) -> f64 {
        self.mexc_network_ms + self.mexc_internal_ms + self.our_processing_ms
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageDecision {
    pub should_trade: bool,
    pub predicted_profit_percent: f64,
    pub latency_to_mexc_ms: f64,
    pub required_spread_percent: f64,
    pub actual_spread_percent: f64,
}

#[derive(Debug, Clone)]
pub struct ArbitrageDecisionMaker {
    synchronizer: PriceSynchronizer,
    latency_profile: LatencyProfile,
    min_profit_percent: f64,
    fees_percent: f64,
    slippage_percent: f64,
}

impl ArbitrageDecisionMaker {
    pub fn new(
        max_points: usize,
        max_time_diff_ms: i64,
        min_profit_percent: f64,
        fees_percent: f64,
        slippage_percent: f64,
    ) -> Self {
        Self {
            synchronizer: PriceSynchronizer::new(max_points, max_time_diff_ms),
            latency_profile: LatencyProfile::default(),
            min_profit_percent,
            fees_percent,
            slippage_percent,
        }
    }
    
    /// Добавляет цену Binance
    pub fn add_binance_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.synchronizer.add_binance_price(price, exchange_ts, local_received);
    }
    
    /// Добавляет цену MEXC
    pub fn add_mexc_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.synchronizer.add_mexc_price(price, exchange_ts, local_received);
    }
    
    /// Принимает решение об арбитраже
    pub fn make_decision(&self) -> Option<ArbitrageDecision> {
        // Получаем текущий спред
        let _spread_diff = self.synchronizer.get_synchronized_spread()?;
        
        // Получаем лаг
        let lag = self.synchronizer.get_mexc_lag()?;
        
        // Конвертируем в проценты (нужна цена для расчёта)
        let (binance_count, mexc_count) = self.synchronizer.stats();
        if binance_count == 0 || mexc_count == 0 {
            return None;
        }
        
        // Простой расчёт: если лаг положительный и спред достаточный
        let latency_to_mexc = self.latency_profile.total_mexc_latency();
        let total_costs = self.fees_percent + self.slippage_percent;
        let required_spread = self.min_profit_percent + total_costs;
        
        // Для простоты: если лаг > 50ms, считаем что есть возможность
        let should_trade = lag > 50; // MEXC отстаёт минимум на 50ms
        
        Some(ArbitrageDecision {
            should_trade,
            predicted_profit_percent: 0.1, // примерное значение
            latency_to_mexc_ms: latency_to_mexc,
            required_spread_percent: required_spread,
            actual_spread_percent: 0.2, // примерное значение
        })
    }
    
    /// Анализирует возможность арбитража
    pub fn analyze_opportunity(&self) -> String {
        match self.make_decision() {
            Some(decision) => {
                if decision.should_trade {
                    format!("✅ ARBITRAGE OPPORTUNITY! Latency: {:.1}ms", decision.latency_to_mexc_ms)
                } else {
                    format!("❌ No arbitrage. Latency: {:.1}ms", decision.latency_to_mexc_ms)
                }
            }
            None => "⏳ Not enough data".to_string(),
        }
    }
    
    /// Возвращает статистику
    pub fn stats(&self) -> (usize, usize) {
        self.synchronizer.stats()
    }
}