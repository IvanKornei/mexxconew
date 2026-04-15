use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::core::PositionManager;
use crate::utils::{Error, LatencyMetrics};

/// Режим торговли
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradingMode {
    Emulation,
    Live,
}

impl Default for TradingMode {
    fn default() -> Self {
        TradingMode::Emulation
    }
}

/// Настройки торговли
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSettings {
    pub capital: f64,
    pub position_size_percent: f64,
    pub leverage: f64,
    pub max_positions: usize,
    pub momentum_threshold: f64,
    pub quick_exit_timeout: i64,
    pub take_profit_percent: f64,
    pub stop_loss_percent: f64,
    pub momentum_weight: f64,
    pub lag_weight: f64,
    pub price_diff_weight: f64,
}

impl Default for TradingSettings {
    fn default() -> Self {
        Self {
            capital: 100.0,
            position_size_percent: 10.0,
            leverage: 200.0,
            max_positions: 2,
            momentum_threshold: 0.03,
            quick_exit_timeout: 2000,
            take_profit_percent: 0.04,
            stop_loss_percent: 0.02,
            momentum_weight: 0.6,
            lag_weight: 0.3,
            price_diff_weight: 0.1,
        }
    }
}

/// Внутреннее состояние системы (под одним lock)
#[derive(Debug, Clone)]
struct InternalState {
    is_running: bool,
    mode: TradingMode,
    trading_settings: Arc<TradingSettings>,
}

/// Состояние системы для сериализации
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub is_running: bool,
    pub mode: TradingMode,
    pub last_updated: i64,
    pub trading_settings: TradingSettings,
}

impl Default for SystemState {
    fn default() -> Self {
        Self {
            is_running: false,
            mode: TradingMode::Emulation,
            last_updated: chrono::Utc::now().timestamp_millis(),
            trading_settings: TradingSettings::default(),
        }
    }
}

/// Менеджер системы - управляет состоянием торговой системы
/// ОПТИМИЗИРОВАН ДЛЯ HFT
pub struct SystemManager {
    // Объединённое состояние под одним lock для минимизации contention
    state: Arc<RwLock<InternalState>>,
    
    // Атомарный флаг для быстрой проверки в hot path
    is_running_fast: Arc<AtomicBool>,
    
    state_file_path: PathBuf,
    position_managers: Vec<Arc<PositionManager>>,

    // Debouncer для батчинга сохранений
    save_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    
    // Метрики производительности
    save_latency: Arc<LatencyMetrics>,
    load_latency: Arc<LatencyMetrics>,
}

impl SystemManager {
    pub fn new(position_managers: Vec<Arc<PositionManager>>) -> Self {
        let state_file_path = PathBuf::from(".kiro/system_state.json");

        Self {
            state: Arc::new(RwLock::new(InternalState {
                is_running: false,
                mode: TradingMode::Emulation,
                trading_settings: Arc::new(TradingSettings::default()),
            })),
            is_running_fast: Arc::new(AtomicBool::new(false)),
            state_file_path,
            position_managers,
            save_task: Arc::new(RwLock::new(None)),
            save_latency: Arc::new(LatencyMetrics::new()),
            load_latency: Arc::new(LatencyMetrics::new()),
        }
    }

    /// Возвращает все управляемые position managers (для внешних компонентов: WS сервер, REST интеграции)
    pub fn position_managers(&self) -> &[Arc<PositionManager>] {
        &self.position_managers
    }
    
    /// Запускает торговлю (оптимизировано для минимальной задержки)
    pub async fn start_trading(&self) -> Result<(), Error> {
        {
            let mut state = self.state.write().await;
            state.is_running = true;
        }
        
        // Атомарный флаг для быстрой проверки
        self.is_running_fast.store(true, Ordering::Release);
        
        // Активируем все Position Managers
        for pm in &self.position_managers {
            pm.set_trading_enabled(true).await;
        }

        info!("🚀 Trading started");
        
        // Сохраняем асинхронно без блокировки
        self.schedule_save().await;
        
        Ok(())
    }
    
    /// Останавливает торговлю
    pub async fn stop_trading(&self) -> Result<(), Error> {
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }
        
        self.is_running_fast.store(false, Ordering::Release);
        
        // Деактивируем все Position Managers
        for pm in &self.position_managers {
            pm.set_trading_enabled(false).await;
        }

        info!("⏸️ Trading stopped");
        
        // Сохраняем асинхронно
        self.schedule_save().await;
        
        Ok(())
    }
    
    /// Переключает режим торговли
    pub async fn switch_mode(&self, mode: TradingMode) -> Result<(), Error> {
        // Проверяем что торговля остановлена
        if self.is_running_fast.load(Ordering::Acquire) {
            return Err(Error::Internal("Stop trading before switching modes".to_string()));
        }
        
        {
            let mut state = self.state.write().await;
            state.mode = mode;
        }
        
        // Обновляем режим во всех Position Managers
        for pm in &self.position_managers {
            pm.set_execution_mode(mode).await;
        }

        info!("🔄 Switched to {:?} mode", mode);
        
        // Сохраняем асинхронно
        self.schedule_save().await;
        
        Ok(())
    }
    
    /// Быстрая проверка состояния (для hot path)
    #[inline]
    pub fn is_running_fast(&self) -> bool {
        self.is_running_fast.load(Ordering::Acquire)
    }
    
    /// Возвращает текущее состояние системы
    pub async fn get_state(&self) -> SystemState {
        let state = self.state.read().await;
        
        SystemState {
            is_running: state.is_running,
            mode: state.mode,
            last_updated: chrono::Utc::now().timestamp_millis(),
            trading_settings: (*state.trading_settings).clone(),
        }
    }
    
    /// Загружает состояние из файла
    pub async fn load_state(&self) -> Result<SystemState, Error> {
        let start = Instant::now();
        
        // Создаём директорию если не существует
        if let Some(parent) = self.state_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Проверяем существует ли файл
        if !self.state_file_path.exists() {
            info!("State file not found, using defaults");
            let default_state = SystemState::default();
            
            // Сохраняем дефолтное состояние
            self.save_state_to_file(&default_state).await?;
            
            self.load_latency.record(start);
            return Ok(default_state);
        }
        
        // Читаем файл
        match std::fs::read_to_string(&self.state_file_path) {
            Ok(content) => {
                match serde_json::from_str::<SystemState>(&content) {
                    Ok(loaded_state) => {
                        // Валидируем настройки
                        if let Err(e) = self.validate_settings(&loaded_state.trading_settings) {
                            warn!("Invalid settings in state file: {}. Using defaults.", e);
                            let default_state = SystemState::default();
                            self.save_state_to_file(&default_state).await?;
                            self.load_latency.record(start);
                            return Ok(default_state);
                        }
                        
                        info!("✅ State loaded: running={}, mode={:?}", loaded_state.is_running, loaded_state.mode);
                        
                        // Применяем загруженное состояние
                        {
                            let mut state = self.state.write().await;
                            state.is_running = loaded_state.is_running;
                            state.mode = loaded_state.mode;
                            state.trading_settings = Arc::new(loaded_state.trading_settings.clone());
                        }
                        
                        self.is_running_fast.store(loaded_state.is_running, Ordering::Release);
                        
                        // Применяем ко всем Position Managers
                        for pm in &self.position_managers {
                            pm.set_trading_enabled(loaded_state.is_running).await;
                            pm.set_execution_mode(loaded_state.mode).await;
                        }
                        
                        // Если режим Live и торговля запущена - предупреждение
                        if loaded_state.is_running && loaded_state.mode == TradingMode::Live {
                            warn!("⚠️ System was running in LIVE mode. Please confirm to continue.");
                        }
                        
                        self.load_latency.record(start);
                        Ok(loaded_state)
                    }
                    Err(e) => {
                        warn!("Failed to parse state file: {}. Using defaults.", e);
                        let default_state = SystemState::default();
                        self.save_state_to_file(&default_state).await?;
                        self.load_latency.record(start);
                        Ok(default_state)
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read state file: {}. Using defaults.", e);
                let default_state = SystemState::default();
                self.save_state_to_file(&default_state).await?;
                self.load_latency.record(start);
                Ok(default_state)
            }
        }
    }
    
    /// Планирует сохранение с debouncing (батчинг)
    async fn schedule_save(&self) {
        let mut save_task = self.save_task.write().await;
        
        // Отменяем предыдущую задачу если есть
        if let Some(handle) = save_task.take() {
            handle.abort();
        }
        
        // Клонируем Arc для передачи в задачу
        let state_arc = Arc::clone(&self.state);
        let file_path = self.state_file_path.clone();
        let save_latency = Arc::clone(&self.save_latency);
        
        // Запускаем новую задачу с задержкой
        *save_task = Some(tokio::spawn(async move {
            // Ждём 100ms для батчинга множественных изменений
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            let state_snapshot = {
                let state = state_arc.read().await;
                SystemState {
                    is_running: state.is_running,
                    mode: state.mode,
                    last_updated: chrono::Utc::now().timestamp_millis(),
                    trading_settings: (*state.trading_settings).clone(),
                }
            };
            
            if let Err(e) = Self::save_state_to_file_static(&file_path, &state_snapshot, &save_latency).await {
                error!("Failed to save state: {}", e);
                
                // Retry через 5 секунд
                tokio::time::sleep(Duration::from_secs(5)).await;
                if let Err(e) = Self::save_state_to_file_static(&file_path, &state_snapshot, &save_latency).await {
                    error!("Failed to save state after retry: {}", e);
                }
            }
        }));
    }
    
    /// Сохраняет текущее состояние в файл (синхронно, для тестов)
    pub async fn save_state(&self) -> Result<(), Error> {
        let state = self.get_state().await;
        self.save_state_to_file(&state).await
    }
    
    /// Внутренний метод для сохранения состояния в файл
    async fn save_state_to_file(&self, state: &SystemState) -> Result<(), Error> {
        Self::save_state_to_file_static(&self.state_file_path, state, &self.save_latency).await
    }
    
    /// Статический метод для сохранения (используется в async задачах)
    async fn save_state_to_file_static(
        file_path: &PathBuf,
        state: &SystemState,
        save_latency: &LatencyMetrics,
    ) -> Result<(), Error> {
        let start = Instant::now();
        
        // Создаём директорию если не существует
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Сериализуем состояние
        let json = serde_json::to_string_pretty(state)?;
        
        // Атомарная запись: пишем во временный файл, затем переименовываем
        let temp_path = file_path.with_extension("tmp");
        
        // Запись
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, file_path)?;
        
        save_latency.record(start);
        info!("💾 State saved successfully");
        
        Ok(())
    }
    
    /// Валидация настроек
    fn validate_settings(&self, settings: &TradingSettings) -> Result<(), Error> {
        if settings.capital <= 0.0 || settings.capital > 1_000_000.0 {
            return Err(Error::Internal(format!("Invalid capital value: {}", settings.capital)));
        }
        
        if settings.position_size_percent <= 0.0 || settings.position_size_percent > 100.0 {
            return Err(Error::Internal(format!("Invalid position size percent: {}", settings.position_size_percent)));
        }
        
        if settings.leverage < 1.0 || settings.leverage > 200.0 {
            return Err(Error::Internal(format!("Invalid leverage: {}", settings.leverage)));
        }
        
        if settings.max_positions == 0 || settings.max_positions > 10 {
            return Err(Error::Internal(format!("Invalid max positions: {}", settings.max_positions)));
        }
        
        let total_weight = settings.momentum_weight + settings.lag_weight + settings.price_diff_weight;
        if (total_weight - 1.0).abs() > 0.01 {
            return Err(Error::Internal(format!("Strategy weights must sum to 1.0, got {}", total_weight)));
        }
        
        Ok(())
    }
    
    /// Обновляет настройки торговли
    pub async fn update_settings(&self, settings: TradingSettings) -> Result<(), Error> {
        // Валидация настроек
        self.validate_settings(&settings)?;
        
        // Обновляем настройки
        {
            let mut state = self.state.write().await;
            state.trading_settings = Arc::new(settings.clone());
        }
        
        // Применяем ко всем Position Managers
        for pm in &self.position_managers {
            pm.update_capital_settings(
                settings.capital,
                settings.position_size_percent,
                settings.leverage,
                settings.max_positions,
            ).await;

            pm.update_strategy_settings(
                settings.momentum_weight,
                settings.lag_weight,
                settings.price_diff_weight,
                settings.momentum_threshold,
                settings.quick_exit_timeout,
            ).await;
        }
        
        info!("⚙️ Settings updated");
        
        // Сохраняем асинхронно
        self.schedule_save().await;
        
        Ok(())
    }
    
    /// Возвращает текущие настройки (Arc для избежания клонирования)
    pub async fn get_settings(&self) -> Arc<TradingSettings> {
        let state = self.state.read().await;
        Arc::clone(&state.trading_settings)
    }
    
    /// Проверяет запущена ли торговля (медленный путь)
    pub async fn is_running(&self) -> bool {
        let state = self.state.read().await;
        state.is_running
    }
    
    /// Возвращает текущий режим
    pub async fn get_mode(&self) -> TradingMode {
        let state = self.state.read().await;
        state.mode
    }
    
    /// Получить метрики производительности
    pub fn get_save_metrics(&self) -> crate::utils::MetricsSnapshot {
        self.save_latency.snapshot()
    }
    
    pub fn get_load_metrics(&self) -> crate::utils::MetricsSnapshot {
        self.load_latency.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PositionManager;
    
    fn make_pm() -> Arc<PositionManager> {
        Arc::new(PositionManager::new(
            100.0,
            10.0,
            200.0,
            2,
            "BTC_USDT".to_string(),
            "BTC".to_string(),
        ))
    }

    #[tokio::test]
    async fn test_start_stop_trading() {
        let pm = make_pm();
        let sm = SystemManager::new(vec![pm.clone()]);

        // Изначально остановлена
        assert!(!sm.is_running().await);
        assert!(!sm.is_running_fast());

        // Запускаем
        sm.start_trading().await.unwrap();
        assert!(sm.is_running().await);
        assert!(sm.is_running_fast());
        assert!(pm.is_trading_enabled().await);

        // Останавливаем
        sm.stop_trading().await.unwrap();
        assert!(!sm.is_running().await);
        assert!(!sm.is_running_fast());
        assert!(!pm.is_trading_enabled().await);
    }

    #[tokio::test]
    async fn test_switch_mode() {
        let pm = make_pm();
        let sm = SystemManager::new(vec![pm.clone()]);

        // Изначально Emulation
        assert_eq!(sm.get_mode().await, TradingMode::Emulation);

        // Переключаем на Live
        sm.switch_mode(TradingMode::Live).await.unwrap();
        assert_eq!(sm.get_mode().await, TradingMode::Live);
        assert_eq!(pm.get_execution_mode().await, TradingMode::Live);
    }

    #[tokio::test]
    async fn test_cannot_switch_mode_while_running() {
        let pm = make_pm();
        let sm = SystemManager::new(vec![pm]);

        // Запускаем торговлю
        sm.start_trading().await.unwrap();

        // Пытаемся переключить режим - должна быть ошибка
        let result = sm.switch_mode(TradingMode::Live).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_settings_validation() {
        let pm = make_pm();
        let sm = SystemManager::new(vec![pm]);
        
        // Невалидный capital
        let mut settings = TradingSettings::default();
        settings.capital = -100.0;
        assert!(sm.update_settings(settings).await.is_err());
        
        // Невалидные веса
        let mut settings = TradingSettings::default();
        settings.momentum_weight = 0.5;
        settings.lag_weight = 0.3;
        settings.price_diff_weight = 0.3; // Сумма > 1.0
        assert!(sm.update_settings(settings).await.is_err());
    }
}
