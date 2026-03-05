use std::collections::VecDeque;

/// Синхронизация времени между биржами и локальной системой
#[derive(Debug, Clone)]
pub struct TimeSync {
    // Смещение часов биржи относительно локального времени
    binance_clock_offset: i64,
    mexc_clock_offset: i64,
    
    // История измерений для фильтрации выбросов
    binance_samples: VecDeque<i64>,
    mexc_samples: VecDeque<i64>,
    
    // Сетевая задержка (RTT / 2)
    binance_network_delay: i64,
    mexc_network_delay: i64,
    
    calibrated: bool,
}

impl TimeSync {
    pub fn new() -> Self {
        Self {
            binance_clock_offset: 0,
            mexc_clock_offset: 0,
            binance_samples: VecDeque::with_capacity(100),
            mexc_samples: VecDeque::with_capacity(100),
            binance_network_delay: 50, // Предполагаем 50ms по умолчанию
            mexc_network_delay: 50,
            calibrated: false,
        }
    }
    
    /// Обновляет синхронизацию для Binance
    pub fn update_binance(&mut self, exchange_ts: i64, local_received: i64) {
        // Вычисляем смещение: exchange_ts - (local_received - network_delay)
        // Предполагаем что exchange_ts это время когда сделка произошла на бирже
        // local_received это когда мы получили данные
        // Разница = смещение часов + сетевая задержка
        
        let offset = exchange_ts - local_received;
        
        self.binance_samples.push_back(offset);
        if self.binance_samples.len() > 100 {
            self.binance_samples.pop_front();
        }
        
        // После 20 сэмплов начинаем калибровку
        if self.binance_samples.len() >= 20 {
            self.calibrate_binance();
        }
    }
    
    /// Обновляет синхронизацию для MEXC
    pub fn update_mexc(&mut self, exchange_ts: i64, local_received: i64) {
        let offset = exchange_ts - local_received;
        
        self.mexc_samples.push_back(offset);
        if self.mexc_samples.len() > 100 {
            self.mexc_samples.pop_front();
        }
        
        if self.mexc_samples.len() >= 20 {
            self.calibrate_mexc();
        }
    }
    
    /// Калибрует смещение Binance (медиана для устойчивости к выбросам)
    fn calibrate_binance(&mut self) {
        let mut sorted: Vec<i64> = self.binance_samples.iter().copied().collect();
        sorted.sort_unstable();
        
        let median = sorted[sorted.len() / 2];
        
        // Оцениваем сетевую задержку (минимальное значение близко к RTT/2)
        let min_offset = sorted[0];
        self.binance_network_delay = (median - min_offset).max(10).min(500);
        
        // Смещение часов = медиана - сетевая задержка
        self.binance_clock_offset = median - self.binance_network_delay;
        
        if !self.calibrated && self.mexc_samples.len() >= 20 {
            self.calibrated = true;
        }
    }
    
    /// Калибрует смещение MEXC
    fn calibrate_mexc(&mut self) {
        let mut sorted: Vec<i64> = self.mexc_samples.iter().copied().collect();
        sorted.sort_unstable();
        
        let median = sorted[sorted.len() / 2];
        let min_offset = sorted[0];
        
        self.mexc_network_delay = (median - min_offset).max(10).min(500);
        self.mexc_clock_offset = median - self.mexc_network_delay;
        
        if !self.calibrated && self.binance_samples.len() >= 20 {
            self.calibrated = true;
        }
    }
    
    /// Конвертирует timestamp Binance в локальное время
    pub fn binance_to_local(&self, exchange_ts: i64) -> i64 {
        exchange_ts - self.binance_clock_offset
    }
    
    /// Конвертирует timestamp MEXC в локальное время
    pub fn mexc_to_local(&self, exchange_ts: i64) -> i64 {
        exchange_ts - self.mexc_clock_offset
    }
    
    /// Вычисляет реальную задержку MEXC относительно Binance
    /// Положительное значение = MEXC отстаёт
    pub fn calculate_mexc_lag(&self, binance_ts: i64, mexc_ts: i64) -> i64 {
        if !self.calibrated {
            // Если не откалиброваны, используем сырую разницу
            return binance_ts - mexc_ts;
        }
        
        // Конвертируем оба timestamp'а в локальное время
        let binance_local = self.binance_to_local(binance_ts);
        let mexc_local = self.mexc_to_local(mexc_ts);
        
        // Разница показывает насколько MEXC отстаёт от Binance
        binance_local - mexc_local
    }
    
    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }
    
    pub fn get_binance_network_delay(&self) -> i64 {
        self.binance_network_delay
    }
    
    pub fn get_mexc_network_delay(&self) -> i64 {
        self.mexc_network_delay
    }
}
