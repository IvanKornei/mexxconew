/// Модуль для принятия решений об арбитраже с учётом всех задержек
/// ИСПРАВЛЕННАЯ ВЕРСИЯ с корректной логикой
use crate::core::PriceSynchronizer;

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
            binance_network_ms: 50.0,
            mexc_network_ms: 150.0,
            binance_internal_ms: 3.0,
            mexc_internal_ms: 15.0,
            our_processing_ms: 2.0,
        }
    }
}

impl LatencyProfile {
    /// Общая задержка для ордера на Binance
    #[inline]
    pub fn total_binance_latency(&self) -> f64 {
        self.binance_network_ms + self.binance_internal_ms + self.our_processing_ms
    }
    
    /// Общая задержка для ордера на MEXC
    #[inline]
    pub fn total_mexc_latency(&self) -> f64 {
        self.mexc_network_ms + self.mexc_internal_ms + self.our_processing_ms
    }
    
    /// Время RTT для MEXC (туда и обратно)
    #[inline]
    pub fn mexc_rtt(&self) -> f64 {
        self.total_mexc_latency() * 2.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArbitrageDecision {
    pub should_trade: bool,
    pub predicted_profit_percent: f64,
    pub predicted_mexc_price: f64,
    pub current_binance_price: f64,
    pub current_mexc_price: f64,
    pub latency_to_mexc_ms: f64,
    pub required_spread_percent: f64,
    pub actual_spread_percent: f64,
    pub mexc_lag_ms: i64,
}

impl Default for ArbitrageDecision {
    fn default() -> Self {
        Self {
            should_trade: false,
            predicted_profit_percent: 0.0,
            predicted_mexc_price: 0.0,
            current_binance_price: 0.0,
            current_mexc_price: 0.0,
            latency_to_mexc_ms: 0.0,
            required_spread_percent: 0.0,
            actual_spread_percent: 0.0,
            mexc_lag_ms: 0,
        }
    }
}

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
    
    /// Обновляет профиль задержек на основе реальных измерений
    pub fn update_latency_profile(&mut self, binance_network_ms: f64, mexc_network_ms: f64) {
        self.latency_profile.binance_network_ms = binance_network_ms;
        self.latency_profile.mexc_network_ms = mexc_network_ms;
    }
    
    /// Добавляет цену Binance
    #[inline]
    pub fn add_binance_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.synchronizer.add_binance_price(price, exchange_ts, local_received);
    }
    
    /// Добавляет цену MEXC
    #[inline]
    pub fn add_mexc_price(&mut self, price: f64, exchange_ts: i64, local_received: i64) {
        self.synchronizer.add_mexc_price(price, exchange_ts, local_received);
    }
    
    /// Прогнозирует цену на MEXC через заданное время
    /// КРИТИЧНО: Используем простой прогноз, но можно улучшить
    #[inline]
    fn predict_mexc_price(&self, current_mexc: f64, _time_ahead_ms: f64) -> f64 {
        // Простой прогноз: цена не меняется
        // TODO: Добавить прогноз на основе:
        // 1. Тренда последних N точек
        // 2. Волатильности
        // 3. Order book imbalance (если доступен)
        current_mexc
    }
    
    /// Принимает решение об арбитраже
    /// ОПТИМИЗИРОВАНО: Минимум аллокаций, inline для hot path
    #[inline]
    pub fn make_decision(&self) -> Option<ArbitrageDecision> {
        // ИСПРАВЛЕНО: Атомарное получение всех данных
        let (spread_diff, lag, binance_price, mexc_price) = 
            self.synchronizer.get_synchronized_data()?;
        
        // Проверка минимальных требований
        let (binance_count, mexc_count) = self.synchronizer.stats();
        if binance_count < 2 || mexc_count < 2 {
            return None; // Недостаточно данных
        }
        
        // Время доставки ордера до MEXC
        let latency_to_mexc = self.latency_profile.total_mexc_latency();
        
        // Прогнозируем цену MEXC на момент исполнения
        let predicted_mexc_price = self.predict_mexc_price(mexc_price, latency_to_mexc);
        
        // ИСПРАВЛЕНО: Корректный расчёт спредов
        let current_spread_percent = (mexc_price - binance_price) / binance_price * 100.0;
        let predicted_spread_percent = (predicted_mexc_price - binance_price) / binance_price * 100.0;
        
        // Общие затраты
        let total_costs = self.fees_percent + self.slippage_percent;
        let required_spread = self.min_profit_percent + total_costs;
        
        // ИСПРАВЛЕНО: Прогнозируемая прибыль
        let predicted_profit = predicted_spread_percent - total_costs;
        
        // ИСПРАВЛЕНО: Решение на основе прибыльности, а не только лага
        let should_trade = predicted_profit >= self.min_profit_percent 
            && lag > 30  // MEXC должен отставать минимум на 30ms
            && current_spread_percent > 0.0; // Базовая проверка направления
        
        Some(ArbitrageDecision {
            should_trade,
            predicted_profit_percent: predicted_profit,
            predicted_mexc_price,
            current_binance_price: binance_price,
            current_mexc_price: mexc_price,
            latency_to_mexc_ms: latency_to_mexc,
            required_spread_percent: required_spread,
            actual_spread_percent: current_spread_percent,
            mexc_lag_ms: lag,
        })
    }
    
    /// Возвращает статистику
    #[inline]
    pub fn stats(&self) -> (usize, usize) {
        self.synchronizer.stats()
    }
}
