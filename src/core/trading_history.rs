use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::info;
use std::path::Path;

use crate::core::trading_strategy::{Position, PositionSide, PositionStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub id: i64,
    pub position_id: String,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub quantity: f64,
    pub entry_time: i64,
    pub exit_time: i64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub status: String,
    pub initial_impulse: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingStats {
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub avg_profit: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub current_balance: f64,
    pub initial_balance: f64,
    pub total_return_percent: f64,
}

pub struct TradingHistory {
    conn: Arc<Mutex<Connection>>,
    initial_balance: f64,
}

impl TradingHistory {
    pub fn new(db_path: &str, initial_balance: f64) -> SqlResult<Self> {
        // Проверяем существование БД для определения первого запуска
        let is_first_run = !Path::new(db_path).exists();
        
        let conn = Connection::open(db_path)?;
        
        // HFT оптимизации для SQLite
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;          -- Write-Ahead Logging (быстрее fsync)
             PRAGMA synchronous = NORMAL;        -- Баланс между скоростью и надёжностью
             PRAGMA cache_size = -64000;         -- 64MB кеш
             PRAGMA temp_store = MEMORY;         -- Временные таблицы в RAM
             PRAGMA mmap_size = 268435456;       -- 256MB memory-mapped I/O
             PRAGMA page_size = 4096;            -- Оптимальный размер страницы
             PRAGMA busy_timeout = 5000;         -- 5 секунд ожидания при блокировке
             PRAGMA wal_autocheckpoint = 1000;"  // Checkpoint каждые 1000 страниц
        )?;
        
        // Создаём таблицы
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                position_id TEXT NOT NULL,
                side TEXT NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                quantity REAL NOT NULL,
                entry_time INTEGER NOT NULL,
                exit_time INTEGER NOT NULL,
                pnl REAL NOT NULL,
                pnl_percent REAL NOT NULL,
                status TEXT NOT NULL,
                initial_impulse REAL NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        
        // Индексы для быстрых запросов
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_trades_exit_time ON trades(exit_time DESC)",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS balance_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                balance REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        
        // Записываем начальный баланс только при первом запуске
        if is_first_run {
            conn.execute(
                "INSERT INTO balance_history (balance, timestamp) VALUES (?1, ?2)",
                [initial_balance, chrono::Utc::now().timestamp_millis() as f64],
            )?;
            info!("💰 Initial balance set: ${:.2}", initial_balance);
        }
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            initial_balance,
        })
    }
    
    /// Записывает закрытую позицию в историю
    pub fn record_trade(&self, position: &Position, exit_price: f64, pnl: f64, pnl_percent: f64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        
        let side = match position.side {
            PositionSide::Long => "Long",
            PositionSide::Short => "Short",
        };
        
        let status = match position.status {
            PositionStatus::Closed => "Closed",
            PositionStatus::StopLoss => "StopLoss",
            PositionStatus::TakeProfit => "TakeProfit",
            PositionStatus::Open => "Open",
        };
        
        conn.execute(
            "INSERT INTO trades (position_id, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_percent, status, initial_impulse)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                position.id,
                side,
                position.entry_price,
                exit_price,
                position.quantity,
                position.entry_time,
                chrono::Utc::now().timestamp_millis(),
                pnl,
                pnl_percent,
                status,
                position.initial_impulse,
            ],
        )?;
        
        // Обновляем баланс
        let current_balance = self.get_current_balance()?;
        let new_balance = current_balance + pnl;
        
        conn.execute(
            "INSERT INTO balance_history (balance, timestamp) VALUES (?1, ?2)",
            [new_balance, chrono::Utc::now().timestamp_millis() as f64],
        )?;
        
        info!(
            "📝 Trade recorded | {} | PnL: ${:.2} ({:.3}%) | New balance: ${:.2}",
            position.id,
            pnl,
            pnl_percent,
            new_balance
        );
        
        Ok(())
    }
    
    /// Получает текущий баланс
    pub fn get_current_balance(&self) -> SqlResult<f64> {
        let conn = self.conn.lock().unwrap();
        
        let balance: f64 = conn.query_row(
            "SELECT balance FROM balance_history ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        
        Ok(balance)
    }
    
    /// Получает статистику торговли
    pub fn get_stats(&self) -> SqlResult<TradingStats> {
        let conn = self.conn.lock().unwrap();
        
        let total_trades: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let winning_trades: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades WHERE pnl > 0",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let losing_trades: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades WHERE pnl < 0",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let total_pnl: f64 = conn.query_row(
            "SELECT COALESCE(SUM(pnl), 0) FROM trades",
            [],
            |row| row.get(0),
        ).unwrap_or(0.0);
        
        let avg_profit: f64 = if total_trades > 0 {
            conn.query_row(
                "SELECT AVG(pnl_percent) FROM trades",
                [],
                |row| row.get(0),
            ).unwrap_or(0.0)
        } else {
            0.0
        };
        
        let best_trade: f64 = conn.query_row(
            "SELECT COALESCE(MAX(pnl_percent), 0) FROM trades",
            [],
            |row| row.get(0),
        ).unwrap_or(0.0);
        
        let worst_trade: f64 = conn.query_row(
            "SELECT COALESCE(MIN(pnl_percent), 0) FROM trades",
            [],
            |row| row.get(0),
        ).unwrap_or(0.0);
        
        let current_balance = self.get_current_balance().unwrap_or(self.initial_balance);
        
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };
        
        let total_return_percent = if self.initial_balance > 0.0 {
            ((current_balance - self.initial_balance) / self.initial_balance) * 100.0
        } else {
            0.0
        };
        
        Ok(TradingStats {
            total_trades,
            winning_trades,
            losing_trades,
            total_pnl,
            win_rate,
            avg_profit,
            best_trade,
            worst_trade,
            current_balance,
            initial_balance: self.initial_balance,
            total_return_percent,
        })
    }
    
    /// Получает последние N сделок
    pub fn get_recent_trades(&self, limit: i64) -> SqlResult<Vec<TradeRecord>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, position_id, side, entry_price, exit_price, quantity, entry_time, exit_time, pnl, pnl_percent, status, initial_impulse
             FROM trades
             ORDER BY id DESC
             LIMIT ?1"
        )?;
        
        let trades = stmt.query_map([limit], |row| {
            Ok(TradeRecord {
                id: row.get(0)?,
                position_id: row.get(1)?,
                side: row.get(2)?,
                entry_price: row.get(3)?,
                exit_price: row.get(4)?,
                quantity: row.get(5)?,
                entry_time: row.get(6)?,
                exit_time: row.get(7)?,
                pnl: row.get(8)?,
                pnl_percent: row.get(9)?,
                status: row.get(10)?,
                initial_impulse: row.get(11)?,
            })
        })?;
        
        let mut result = Vec::new();
        for trade in trades {
            if let Ok(t) = trade {
                result.push(t);
            }
        }
        
        Ok(result)
    }
    
    /// Проверяет достаточно ли баланса для открытия позиции
    pub fn can_open_position(&self, required_margin: f64) -> bool {
        if let Ok(balance) = self.get_current_balance() {
            balance >= required_margin
        } else {
            false
        }
    }
}
