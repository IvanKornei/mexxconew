use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::core::system_manager_optimized::TradingMode;
use crate::core::trading_history::{TradingHistory, TradingStats, TradeRecord};
use crate::core::trading_strategy::Position;

/// Менеджер режимов торговли - управляет разделением данных между emulation и live
pub struct TradingModeManager {
    current_mode: Arc<RwLock<TradingMode>>,
    emulation_db: Arc<TradingHistory>,
    live_db: Arc<TradingHistory>,
}

impl TradingModeManager {
    /// Создаёт новый менеджер режимов с отдельными БД
    pub fn new(initial_balance: f64) -> Result<Self, rusqlite::Error> {
        // Создаём директорию для БД если не существует
        std::fs::create_dir_all(".kiro").ok();
        
        let emulation_db = TradingHistory::new(".kiro/emulation.db", initial_balance)?;
        let live_db = TradingHistory::new(".kiro/live.db", initial_balance)?;
        
        info!("📊 Trading mode manager initialized");
        info!("  Emulation DB: .kiro/emulation.db");
        info!("  Live DB: .kiro/live.db");
        
        Ok(Self {
            current_mode: Arc::new(RwLock::new(TradingMode::Emulation)),
            emulation_db: Arc::new(emulation_db),
            live_db: Arc::new(live_db),
        })
    }
    
    /// Устанавливает текущий режим
    pub async fn set_mode(&self, mode: TradingMode) {
        *self.current_mode.write().await = mode;
        info!("🔄 Trading mode set to: {:?}", mode);
    }
    
    /// Возвращает текущий режим
    pub async fn get_current_mode(&self) -> TradingMode {
        *self.current_mode.read().await
    }
    
    /// Получает статистику для указанного режима
    pub async fn get_stats(&self, mode: TradingMode) -> Option<TradingStats> {
        let db = match mode {
            TradingMode::Emulation => &self.emulation_db,
            TradingMode::Live => &self.live_db,
        };
        
        match db.get_stats() {
            Ok(stats) => Some(stats),
            Err(e) => {
                error!("Failed to get stats for {:?} mode: {}", mode, e);
                None
            }
        }
    }
    
    /// Получает последние сделки для указанного режима
    pub async fn get_trades(&self, mode: TradingMode, limit: i64) -> Vec<TradeRecord> {
        let db = match mode {
            TradingMode::Emulation => &self.emulation_db,
            TradingMode::Live => &self.live_db,
        };
        
        match db.get_recent_trades(limit) {
            Ok(trades) => trades,
            Err(e) => {
                error!("Failed to get trades for {:?} mode: {}", mode, e);
                Vec::new()
            }
        }
    }
    
    /// Записывает сделку в БД текущего режима
    pub async fn record_trade(&self, position: &Position, exit_price: f64, pnl: f64, pnl_percent: f64) {
        let mode = self.get_current_mode().await;
        
        let db = match mode {
            TradingMode::Emulation => Arc::clone(&self.emulation_db),
            TradingMode::Live => Arc::clone(&self.live_db),
        };
        
        let position_clone = position.clone();
        
        // Выполняем запись в БД в отдельном потоке чтобы не блокировать async runtime
        let handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = db.record_trade(&position_clone, exit_price, pnl, pnl_percent) {
                error!("Failed to record trade in {:?} mode: {}", mode, e);
            }
        });
        
        // Ждём завершения записи (но не блокируем другие задачи)
        let _ = handle.await;
    }
    
    /// Получает текущий баланс для указанного режима
    pub async fn get_balance(&self, mode: TradingMode) -> f64 {
        let db = match mode {
            TradingMode::Emulation => &self.emulation_db,
            TradingMode::Live => &self.live_db,
        };
        
        match db.get_current_balance() {
            Ok(balance) => balance,
            Err(e) => {
                error!("Failed to get balance for {:?} mode: {}", mode, e);
                0.0
            }
        }
    }
    
    /// Проверяет достаточно ли баланса для открытия позиции в текущем режиме
    pub async fn can_open_position(&self, required_margin: f64) -> bool {
        let mode = self.get_current_mode().await;
        
        let db = match mode {
            TradingMode::Emulation => &self.emulation_db,
            TradingMode::Live => &self.live_db,
        };
        
        db.can_open_position(required_margin)
    }
    
    /// Получает статистику для текущего режима
    pub async fn get_current_stats(&self) -> Option<TradingStats> {
        let mode = self.get_current_mode().await;
        self.get_stats(mode).await
    }
    
    /// Получает последние сделки для текущего режима
    pub async fn get_current_trades(&self, limit: i64) -> Vec<TradeRecord> {
        let mode = self.get_current_mode().await;
        self.get_trades(mode, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::trading_strategy::{PositionSide, PositionStatus};
    use std::sync::atomic::{AtomicU64, Ordering};
    
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    
    // Вспомогательная функция для создания менеджера с уникальными БД
    fn create_test_manager(initial_balance: f64) -> Result<TradingModeManager, rusqlite::Error> {
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::fs::create_dir_all(".kiro/test").ok();
        
        let emulation_db = TradingHistory::new(
            &format!(".kiro/test/emulation_test_{}.db", test_id),
            initial_balance
        )?;
        let live_db = TradingHistory::new(
            &format!(".kiro/test/live_test_{}.db", test_id),
            initial_balance
        )?;
        
        Ok(TradingModeManager {
            current_mode: Arc::new(RwLock::new(TradingMode::Emulation)),
            emulation_db: Arc::new(emulation_db),
            live_db: Arc::new(live_db),
        })
    }
    
    #[tokio::test]
    async fn test_mode_switching() {
        let manager = create_test_manager(100.0).unwrap();
        
        // Изначально Emulation
        assert_eq!(manager.get_current_mode().await, TradingMode::Emulation);
        
        // Переключаем на Live
        manager.set_mode(TradingMode::Live).await;
        assert_eq!(manager.get_current_mode().await, TradingMode::Live);
    }
    
    #[tokio::test]
    async fn test_separate_databases() {
        let manager = create_test_manager(100.0).unwrap();
        
        // Создаём тестовую позицию
        let position = Position {
            id: "test_1".to_string(),
            entry_price: 50000.0,
            current_price: 50100.0,
            quantity: 0.01,
            side: PositionSide::Long,
            entry_time: chrono::Utc::now().timestamp_millis(),
            initial_impulse: 0.05,
            trailing_stop: 50000.0,
            highest_price: 50100.0,
            lowest_price: 50000.0,
            status: PositionStatus::Closed,
            take_profit_target: 50200.0,
        };
        
        // Записываем в Emulation
        manager.set_mode(TradingMode::Emulation).await;
        manager.record_trade(&position, 50100.0, 1.0, 0.2).await;
        
        let emulation_stats = manager.get_stats(TradingMode::Emulation).await.unwrap();
        assert_eq!(emulation_stats.total_trades, 1);
        
        // Проверяем что в Live нет сделок
        let live_stats = manager.get_stats(TradingMode::Live).await.unwrap();
        assert_eq!(live_stats.total_trades, 0);
    }
}
