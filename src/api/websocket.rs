use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

use crate::core::{PriceState, PositionManager, Position, TradingStats, TradeRecord};
use crate::core::trading_strategy::StrategySettings;

/// Команды от клиента
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientCommand {
    #[serde(rename = "updateSettings")]
    UpdateSettings { settings: StrategySettings },
    #[serde(rename = "getSettings")]
    GetSettings,
    #[serde(rename = "updateCapital")]
    UpdateCapital { 
        capital: f64,
        position_size_percent: f64,
        leverage: f64,
        max_positions: usize,
    },
}

/// Ответы сервера
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ServerResponse {
    #[serde(rename = "settings")]
    Settings(StrategySettings),
}

#[derive(Debug, Clone, Serialize)]
pub struct WsMessage {
    #[serde(flatten)]
    pub price_state: PriceState,
    pub positions: Vec<Position>,
    pub total_pnl: f64,
    pub open_positions_count: usize,
    // Добавляем статистику из базы данных
    pub trading_stats: Option<TradingStats>,
    pub recent_trades: Vec<TradeRecord>,
}

pub struct WsServer {
    state_rx: watch::Receiver<PriceState>,
    position_manager: Arc<PositionManager>,
}

impl WsServer {
    pub fn new(state_rx: watch::Receiver<PriceState>, position_manager: Arc<PositionManager>) -> Self {
        Self { 
            state_rx,
            position_manager,
        }
    }
    
    pub fn router(self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        
        Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(health_handler))
            .with_state((self.state_rx, self.position_manager))
            .layer(cors)
    }
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State((state_rx, position_manager)): State<(watch::Receiver<PriceState>, Arc<PositionManager>)>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state_rx, position_manager))
}

async fn handle_socket(
    socket: WebSocket, 
    mut state_rx: watch::Receiver<PriceState>,
    position_manager: Arc<PositionManager>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    info!("New WebSocket client connected");
    
    // Канал для отправки дополнительных сообщений (например, ответов на команды)
    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    
    // Клонируем position_manager для разных задач
    let position_manager_send = position_manager.clone();
    let position_manager_recv = position_manager.clone();
    
    // Send initial state with positions
    let initial_msg = {
        let state = state_rx.borrow_and_update().clone();
        let positions = position_manager.get_open_positions().await;
        let stats = position_manager.get_stats().await;
        let trading_stats = position_manager.get_trading_stats();
        let recent_trades = position_manager.get_recent_trades(20);
        
        WsMessage {
            price_state: state,
            positions,
            total_pnl: stats.total_pnl,
            open_positions_count: stats.open_positions,
            trading_stats,
            recent_trades,
        }
    };
    
    if let Ok(json) = serde_json::to_string(&initial_msg) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }
    
    // Spawn task to send updates with throttling
    let mut send_task = tokio::spawn(async move {
        // ОПТИМИЗАЦИЯ: Throttle до 20 обновлений/сек для WebSocket
        // Избегаем перегрузки клиента и снижаем CPU usage
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Проверяем изменения без блокировки
                    if state_rx.has_changed().unwrap_or(false) {
                        // Быстро читаем state (без долгих операций под lock)
                        let state = state_rx.borrow_and_update().clone();
                        
                        // Читаем позиции (может занять время, но не блокирует state_rx)
                        let positions = position_manager_send.get_open_positions().await;
                        let stats = position_manager_send.get_stats().await;
                        let trading_stats = position_manager_send.get_trading_stats();
                        let recent_trades = position_manager_send.get_recent_trades(20);
                        
                        let ws_msg = WsMessage {
                            price_state: state,
                            positions,
                            total_pnl: stats.total_pnl,
                            open_positions_count: stats.open_positions,
                            trading_stats,
                            recent_trades,
                        };
                        
                        // Сериализация вне критического пути
                        match serde_json::to_string(&ws_msg) {
                            Ok(json) => {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize WsMessage: {}", e);
                                break;
                            }
                        }
                    }
                }
                // Отправляем ответы на команды
                Some(response) = response_rx.recv() => {
                    if sender.send(Message::Text(response)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    
    // Spawn task to receive messages (for settings updates)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Обрабатываем входящие команды
                    if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                        match cmd {
                            ClientCommand::UpdateSettings { settings } => {
                                position_manager_recv.update_strategy_settings(
                                    settings.momentum_weight,
                                    settings.lag_weight,
                                    settings.price_diff_weight,
                                    settings.momentum_threshold,
                                    settings.quick_exit_timeout_ms,
                                ).await;
                            }
                            ClientCommand::GetSettings => {
                                // Отправляем текущие настройки обратно
                                let current_settings = position_manager_recv.get_strategy_settings().await;
                                if let Ok(json) = serde_json::to_string(&ServerResponse::Settings(current_settings)) {
                                    let _ = response_tx.send(json);
                                }
                            }
                            ClientCommand::UpdateCapital { capital, position_size_percent, leverage, max_positions } => {
                                position_manager_recv.update_capital_settings(
                                    capital,
                                    position_size_percent,
                                    leverage,
                                    max_positions,
                                ).await;
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
    
    info!("WebSocket client disconnected");
}
