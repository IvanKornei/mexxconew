use serde::{Deserialize, Serialize};

/// Менеджер капитала - реалистичное управление деньгами
#[derive(Debug, Clone)]
pub struct CapitalManager {
    initial_capital: f64,      // Начальный капитал ($)
    current_capital: f64,      // Текущий капитал ($) - это баланс на бирже
    position_size_percent: f64, // Процент на сделку (0.0-1.0)
    leverage: f64,             // Плечо
    cumulative_volume: f64,    // Накопленный объём торговли ($)
}

impl CapitalManager {
    pub fn new(initial_capital: f64, position_size_percent: f64, leverage: f64) -> Self {
        Self {
            initial_capital,
            current_capital: initial_capital, // Баланс = начальному капиталу
            position_size_percent,
            leverage,
            cumulative_volume: 0.0,
        }
    }
    
    /// Возвращает текущую комиссию taker в зависимости от объёма
    pub fn get_taker_fee(&self) -> f64 {
        if self.cumulative_volume >= 1_000_000.0 {
            0.0001 // 0.01% для объёма >= $1M
        } else {
            0.0 // 0% для объёма < $1M (новые аккаунты)
        }
    }
    
    /// Рассчитывает размер позиции в BTC
    pub fn calculate_position_size(&self, btc_price: f64) -> f64 {
        // Сколько $ выделяем на сделку (от баланса на бирже)
        let position_usd = self.current_capital * self.position_size_percent;
        
        // С учётом плеча - это размер ордера
        let effective_usd = position_usd * self.leverage;
        
        // Конвертируем в BTC
        effective_usd / btc_price
    }
    
    /// Рассчитывает комиссию за сделку (открытие + закрытие)
    pub fn calculate_fee(&self, position_value_usd: f64) -> f64 {
        let taker_fee = self.get_taker_fee();
        // Комиссия берётся от размера ордера (с плечом), а не от баланса
        position_value_usd * taker_fee * 2.0
    }
    
    /// Проверяет достаточно ли средств
    pub fn can_open_position(&self, btc_price: f64) -> bool {
        let required_usd = self.current_capital * self.position_size_percent;
        required_usd > 0.0 && self.current_capital > 0.0
    }
    
    /// Обновляет капитал после закрытия позиции
    pub fn update_capital(&mut self, pnl_usd: f64, position_value_usd: f64) {
        // Добавляем к накопленному объёму (открытие + закрытие)
        self.cumulative_volume += position_value_usd * 2.0;
        
        // Комиссия вычитается из баланса
        let fee = self.calculate_fee(position_value_usd);
        
        // PnL с учётом плеча уже рассчитан в position_manager
        // Обновляем баланс: добавляем чистую прибыль и вычитаем комиссию
        self.current_capital += pnl_usd - fee;
    }
    
    pub fn get_current_capital(&self) -> f64 {
        self.current_capital
    }
    
    pub fn get_initial_capital(&self) -> f64 {
        self.initial_capital
    }
    
    pub fn get_cumulative_volume(&self) -> f64 {
        self.cumulative_volume
    }
    
    pub fn get_return_percent(&self) -> f64 {
        ((self.current_capital - self.initial_capital) / self.initial_capital) * 100.0
    }
    
    pub fn get_leverage(&self) -> f64 {
        self.leverage
    }
    
    pub fn get_position_size_percent(&self) -> f64 {
        self.position_size_percent
    }
    
    pub fn is_liquidated(&self) -> bool {
        self.current_capital <= 0.0
    }
}

/// Настройки капитала для передачи через WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalSettings {
    pub capital: f64,
    pub position_size_percent: f64,
    pub leverage: f64,
    pub max_positions: usize,
}

impl Default for CapitalSettings {
    fn default() -> Self {
        Self {
            capital: 100.0,
            position_size_percent: 10.0,
            leverage: 1.0,
            max_positions: 2,
        }
    }
}
