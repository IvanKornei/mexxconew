use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// Торговая позиция
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub side: PositionSide,
    pub entry_time: i64,
    pub initial_impulse: f64,
    pub trailing_stop: f64,
    pub highest_price: f64,
    pub lowest_price: f64,
    pub status: PositionStatus,
    pub take_profit_target: f64,  // Цель прибыли
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PositionStatus {
    Open,
    Closed,
    StopLoss,
    TakeProfit,
}

/// История цен для расчёта momentum
#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    pub price: f64,
    pub timestamp: i64,
}

/// Предиктивная стратегия импульсной торговли
/// 
/// КЛЮЧЕВАЯ ИДЕЯ:
/// Комбинируем несколько сигналов с настраиваемыми весами
pub struct ImpulseStrategy {
    // Основные параметры
    pub momentum_threshold: f64,      // Порог импульса (% за 100ms)
    prediction_window_ms: i64,    // Окно предсказания (ms)
    pub quick_exit_timeout_ms: i64,   // Таймаут быстрого выхода (ms)
    pub take_profit_percent: f64,     // Цель прибыли (%)
    pub stop_loss_percent: f64,       // Стоп-лосс (%)
    
    // Веса сигналов (0.0 - 1.0)
    pub momentum_weight: f64,         // Вес импульса
    pub lag_weight: f64,              // Вес статистики лага
    pub price_diff_weight: f64,       // Вес разницы цен
    
    // История для расчёта momentum
    pub binance_history: VecDeque<PriceSnapshot>,
    pub mexc_history: VecDeque<PriceSnapshot>,
    
    // Статистика лагов
    lag_history: VecDeque<i64>,
    max_history_size: usize,
}

impl Default for ImpulseStrategy {
    fn default() -> Self {
        Self {
            momentum_threshold: 0.01,      // Снижен с 0.03% до 0.01% (более чувствительно)
            prediction_window_ms: 500,
            quick_exit_timeout_ms: 3000,   // Увеличен до 3 секунд
            take_profit_percent: 0.03,     // Снижен до 0.03% (быстрее фиксируем)
            stop_loss_percent: 0.02,
            // Веса по умолчанию (сбалансированные)
            momentum_weight: 0.6,      // 60% - импульс (главный)
            lag_weight: 0.3,           // 30% - статистика лага
            price_diff_weight: 0.1,    // 10% - разница цен
            binance_history: VecDeque::with_capacity(20),
            mexc_history: VecDeque::with_capacity(20),
            lag_history: VecDeque::with_capacity(100),
            max_history_size: 20,
        }
    }
}

impl ImpulseStrategy {
    pub fn new(
        momentum_threshold: f64,
        prediction_window_ms: i64,
        quick_exit_timeout_ms: i64,
        take_profit_percent: f64,
        stop_loss_percent: f64,
        momentum_weight: f64,
        lag_weight: f64,
        price_diff_weight: f64,
    ) -> Self {
        Self {
            momentum_threshold,
            prediction_window_ms,
            quick_exit_timeout_ms,
            take_profit_percent,
            stop_loss_percent,
            momentum_weight,
            lag_weight,
            price_diff_weight,
            binance_history: VecDeque::with_capacity(20),
            mexc_history: VecDeque::with_capacity(20),
            lag_history: VecDeque::with_capacity(100),
            max_history_size: 20,
        }
    }
    
    /// Обновляет историю цен Binance
    pub fn update_binance_price(&mut self, price: f64, timestamp: i64) {
        self.binance_history.push_back(PriceSnapshot { price, timestamp });
        if self.binance_history.len() > self.max_history_size {
            self.binance_history.pop_front();
        }
    }
    
    /// Обновляет историю цен MEXC
    pub fn update_mexc_price(&mut self, price: f64, timestamp: i64) {
        self.mexc_history.push_back(PriceSnapshot { price, timestamp });
        if self.mexc_history.len() > self.max_history_size {
            self.mexc_history.pop_front();
        }
    }
    
    /// Обновляет статистику лагов
    pub fn update_lag_stats(&mut self, lag_ms: i64) {
        self.lag_history.push_back(lag_ms);
        if self.lag_history.len() > 100 {
            self.lag_history.pop_front();
        }
    }
    
    /// Вычисляет средний ПОЛОЖИТЕЛЬНЫЙ лаг (когда MEXC отстаёт)
    pub fn get_average_lag(&self) -> i64 {
        if self.lag_history.is_empty() {
            return 300; // Дефолтное значение
        }
        
        // Считаем только положительные лаги (когда MEXC отстаёт)
        let positive_lags: Vec<i64> = self.lag_history.iter()
            .filter(|&&lag| lag > 0)
            .copied()
            .collect();
        
        if positive_lags.is_empty() {
            return 0; // Нет положительных лагов
        }
        
        let sum: i64 = positive_lags.iter().sum();
        sum / positive_lags.len() as i64
    }
    
    /// Вычисляет momentum (скорость изменения цены)
    /// Возвращает % изменения за последние 100ms
    pub fn calculate_momentum(&self, history: &VecDeque<PriceSnapshot>) -> Option<f64> {
        if history.len() < 2 {
            return None;
        }
        
        let latest = history.back()?;
        let now = latest.timestamp;
        
        // Ищем цену 100ms назад
        let target_time = now - 100;
        let old_snapshot = history.iter()
            .rev()
            .find(|s| s.timestamp <= target_time)?;
        
        let price_change = ((latest.price - old_snapshot.price) / old_snapshot.price) * 100.0;
        Some(price_change)
    }
    
    /// УПРОЩЁННАЯ ЛОГИКА: Торгуем только задержку MEXC
    /// Проверяет сигнал на вход с использованием взвешенной системы
    /// Идея: Binance - ведущая биржа, MEXC отстаёт на 200-500ms
    /// Когда видим импульс на Binance → открываем на MEXC
    pub fn check_entry_signal(
        &mut self,
        binance_price: f64,
        mexc_price: f64,
    ) -> Option<PositionSide> {
        // 1. MOMENTUM SIGNAL (0-1): импульс цены на Binance
        let binance_momentum = self.calculate_momentum(&self.binance_history).unwrap_or(0.0);
        let momentum_signal = if binance_momentum.abs() >= self.momentum_threshold {
            (binance_momentum.abs() / self.momentum_threshold).min(1.0)
        } else {
            0.0
        };
        
        // 2. LAG SIGNAL (0-1): статистика лага - насколько часто MEXC отстаёт
        let lag_signal = self.calculate_lag_signal();
        
        // 3. PRICE DIFF SIGNAL (0-1): текущая разница цен
        let price_diff_percent = ((binance_price - mexc_price) / mexc_price) * 100.0;
        let price_diff_signal = (price_diff_percent.abs() / 0.1).min(1.0); // 0.1% = max signal
        
        // 4. КОМБИНИРУЕМ СИГНАЛЫ С ВЕСАМИ
        let total_signal = 
            momentum_signal * self.momentum_weight +
            lag_signal * self.lag_weight +
            price_diff_signal * self.price_diff_weight;
        
        // 5. ПОРОГ ВХОДА: если общий сигнал > 0.5, входим
        if total_signal < 0.5 {
            return None;
        }
        
        // 6. ОПРЕДЕЛЯЕМ НАПРАВЛЕНИЕ по momentum и price diff
        if binance_momentum > 0.0 && price_diff_percent > 0.0 {
            // Binance выше и растёт → Long на MEXC (догонит вверх)
            Some(PositionSide::Long)
        } else if binance_momentum < 0.0 && price_diff_percent < 0.0 {
            // Binance ниже и падает → Short на MEXC (догонит вниз)
            Some(PositionSide::Short)
        } else {
            None
        }
    }
    
    /// Вычисляет сигнал лага на основе статистики (0-1)
    /// 1.0 = MEXC стабильно отстаёт, отличная возможность
    /// 0.0 = MEXC не отстаёт или опережает
    fn calculate_lag_signal(&self) -> f64 {
        if self.lag_history.is_empty() {
            return 0.0; // Нет данных - нет сигнала
        }
        
        // Берём последние 50 измерений для статистики
        let recent_lags: Vec<i64> = self.lag_history.iter()
            .rev()
            .take(50)
            .copied()
            .collect();
        
        if recent_lags.is_empty() {
            return 0.0;
        }
        
        // Считаем процент положительных лагов (когда MEXC отстаёт > 100ms)
        let positive_count = recent_lags.iter().filter(|&&lag| lag > 100).count();
        let negative_count = recent_lags.iter().filter(|&&lag| lag < -100).count();
        
        // Если много отрицательных лагов - это плохо, возвращаем 0
        if negative_count > positive_count {
            return 0.0;
        }
        
        let positive_ratio = positive_count as f64 / recent_lags.len() as f64;
        
        // Считаем средний положительный лаг
        let positive_lags: Vec<i64> = recent_lags.iter()
            .filter(|&&lag| lag > 100)
            .copied()
            .collect();
        
        let avg_positive_lag = if !positive_lags.is_empty() {
            positive_lags.iter().sum::<i64>() as f64 / positive_lags.len() as f64
        } else {
            return 0.0; // Нет положительных лагов - нет возможности
        };
        
        // Комбинируем: частота отставания + величина отставания
        let frequency_score = positive_ratio; // 0-1
        let magnitude_score = (avg_positive_lag / 500.0).min(1.0); // 500ms = max score
        
        // Итоговый сигнал: 70% частота, 30% величина
        frequency_score * 0.7 + magnitude_score * 0.3
    }
    
    pub fn create_position(
        &self,
        entry_price: f64,
        quantity: f64,
        side: PositionSide,
        impulse_percent: f64,
    ) -> Position {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        let trailing_stop = entry_price;
        
        // Вычисляем цель прибыли
        let take_profit_target = match side {
            PositionSide::Long => entry_price * (1.0 + self.take_profit_percent / 100.0),
            PositionSide::Short => entry_price * (1.0 - self.take_profit_percent / 100.0),
        };
        
        Position {
            id: format!("pos_{}", now),
            entry_price,
            current_price: entry_price,
            quantity,
            side: side.clone(),
            entry_time: now,
            initial_impulse: impulse_percent,
            trailing_stop,
            highest_price: entry_price,
            lowest_price: entry_price,
            status: PositionStatus::Open,
            take_profit_target,
        }
    }
    
    /// Обновляет позицию с новой ценой
    /// НОВАЯ ЛОГИКА: быстрый выход по таймауту или тейк-профиту
    pub fn update_position(&self, position: &mut Position, new_price: f64) -> bool {
        position.current_price = new_price;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        let time_in_position = now - position.entry_time;
        
        // Быстрый выход по таймауту
        if time_in_position > self.quick_exit_timeout_ms {
            position.status = PositionStatus::Closed;
            return true;
        }
        
        match position.side {
            PositionSide::Long => self.update_long_position(position, new_price),
            PositionSide::Short => self.update_short_position(position, new_price),
        }
    }
    
    fn update_long_position(&self, position: &mut Position, new_price: f64) -> bool {
        // Обновляем максимальную цену
        if new_price > position.highest_price {
            position.highest_price = new_price;
        }
        
        // Проверяем тейк-профит
        if new_price >= position.take_profit_target {
            position.status = PositionStatus::TakeProfit;
            return true;
        }
        
        // Проверяем стоп-лосс
        let loss_percent = ((new_price - position.entry_price) / position.entry_price) * 100.0;
        if loss_percent <= -self.stop_loss_percent {
            position.status = PositionStatus::StopLoss;
            return true;
        }
        
        false
    }
    
    fn update_short_position(&self, position: &mut Position, new_price: f64) -> bool {
        // Обновляем минимальную цену
        if new_price < position.lowest_price {
            position.lowest_price = new_price;
        }
        
        // Проверяем тейк-профит
        if new_price <= position.take_profit_target {
            position.status = PositionStatus::TakeProfit;
            return true;
        }
        
        // Проверяем стоп-лосс
        let loss_percent = ((position.entry_price - new_price) / position.entry_price) * 100.0;
        if loss_percent <= -self.stop_loss_percent {
            position.status = PositionStatus::StopLoss;
            return true;
        }
        
        false
    }
    
    /// Вычисляет текущую прибыль/убыток позиции
    pub fn calculate_pnl(&self, position: &Position) -> f64 {
        match position.side {
            PositionSide::Long => {
                (position.current_price - position.entry_price) * position.quantity
            }
            PositionSide::Short => {
                (position.entry_price - position.current_price) * position.quantity
            }
        }
    }
    
    /// Вычисляет прибыль в процентах
    pub fn calculate_pnl_percent(&self, position: &Position) -> f64 {
        match position.side {
            PositionSide::Long => {
                ((position.current_price - position.entry_price) / position.entry_price) * 100.0
            }
            PositionSide::Short => {
                ((position.entry_price - position.current_price) / position.entry_price) * 100.0
            }
        }
    }
    
    /// Обновляет настройки стратегии
    pub fn update_settings(
        &mut self,
        momentum_weight: f64,
        lag_weight: f64,
        price_diff_weight: f64,
        momentum_threshold: f64,
        quick_exit_timeout_ms: i64,
    ) {
        self.momentum_weight = momentum_weight;
        self.lag_weight = lag_weight;
        self.price_diff_weight = price_diff_weight;
        self.momentum_threshold = momentum_threshold;
        self.quick_exit_timeout_ms = quick_exit_timeout_ms;
    }
    
    /// Возвращает текущие настройки
    pub fn get_settings(&self) -> StrategySettings {
        StrategySettings {
            momentum_weight: self.momentum_weight,
            lag_weight: self.lag_weight,
            price_diff_weight: self.price_diff_weight,
            momentum_threshold: self.momentum_threshold,
            quick_exit_timeout_ms: self.quick_exit_timeout_ms,
        }
    }
}

/// Настройки стратегии для передачи через WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySettings {
    pub momentum_weight: f64,
    pub lag_weight: f64,
    pub price_diff_weight: f64,
    pub momentum_threshold: f64,
    pub quick_exit_timeout_ms: i64,
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_momentum_calculation() {
        let mut strategy = ImpulseStrategy::default();
        
        // Добавляем историю цен
        strategy.update_binance_price(100.0, 1000);
        strategy.update_binance_price(100.05, 1100); // +0.05% за 100ms
        
        let momentum = strategy.calculate_momentum(&strategy.binance_history);
        assert!(momentum.is_some());
        assert!(momentum.unwrap() > 0.04);
    }
}
