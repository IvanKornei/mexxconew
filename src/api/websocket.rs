use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

use crate::core::{PriceState, PositionManager, Position, TradingStats, TradeRecord};
use crate::core::trading_strategy::StrategySettings;
use crate::utils::{SystemHealth};

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
    system_health: Option<Arc<SystemHealth>>,
}

impl WsServer {
    pub fn new(state_rx: watch::Receiver<PriceState>, position_manager: Arc<PositionManager>) -> Self {
        Self { 
            state_rx,
            position_manager,
            system_health: None,
        }
    }
    
    /// Устанавливает систему мониторинга здоровья
    pub fn with_health(mut self, health: Arc<SystemHealth>) -> Self {
        self.system_health = Some(health);
        self
    }
    
    pub fn router(self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        
        let health = self.system_health.clone();
        
        Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(move || health_handler(health)))
            .route("/health/detailed", get(move || detailed_health_handler(self.system_health.clone())))
            .with_state((self.state_rx, self.position_manager))
            .layer(cors)
    }
}

async fn health_handler(health: Option<Arc<SystemHealth>>) -> &'static str {
    if let Some(health) = health {
        match health.overall_status() {
            crate::utils::HealthStatus::Healthy => "OK",
            crate::utils::HealthStatus::Degraded => "DEGRADED",
            crate::utils::HealthStatus::Unhealthy => "UNHEALTHY",
        }
    } else {
        "OK"
    }
}

async fn detailed_health_handler(health: Option<Arc<SystemHealth>>) -> Json<serde_json::Value> {
    if let Some(health) = health {
        let components = health.check_all();
        let overall = health.overall_status();
        
        Json(serde_json::json!({
            "status": format!("{:?}", overall),
            "components": components,
        }))
    } else {
        Json(serde_json::json!({
            "status": "Healthy",
            "components": [],
        }))
    }
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
        let trading_stats = position_manager.get_trading_stats().await;
        let recent_trades = position_manager.get_recent_trades(20).await;
        
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
                        let trading_stats = position_manager_send.get_trading_stats().await;
                        let recent_trades = position_manager_send.get_recent_trades(20).await;
                        
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
                    // Валидация размера сообщения (защита от DoS)
                    if text.len() > 10_000 {
                        error!("Message too large: {} bytes", text.len());
                        continue;
                    }
                    
                    // Обрабатываем входящие команды
                    match serde_json::from_str::<ClientCommand>(&text) {
                        Ok(cmd) => {
                            match cmd {
                                ClientCommand::UpdateSettings { settings } => {
                                    // Валидация настроек стратегии
                                    if !validate_strategy_settings(&settings) {
                                        error!("Invalid strategy settings received");
                                        continue;
                                    }
                                    
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
                                    // Валидация настроек капитала
                                    if !validate_capital_settings(capital, position_size_percent, leverage, max_positions) {
                                        error!("Invalid capital settings received");
                                        continue;
                                    }
                                    
                                    position_manager_recv.update_capital_settings(
                                        capital,
                                        position_size_percent,
                                        leverage,
                                        max_positions,
                                    ).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse client command: {}", e);
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

/// Валидация настроек стратегии
fn validate_strategy_settings(settings: &StrategySettings) -> bool {
    // Проверяем что веса в диапазоне [0, 1]
    if settings.momentum_weight < 0.0 || settings.momentum_weight > 1.0 {
        return false;
    }
    if settings.lag_weight < 0.0 || settings.lag_weight > 1.0 {
        return false;
    }
    if settings.price_diff_weight < 0.0 || settings.price_diff_weight > 1.0 {
        return false;
    }
    
    // Проверяем что сумма весов примерно равна 1.0 (с погрешностью)
    let total_weight = settings.momentum_weight + settings.lag_weight + settings.price_diff_weight;
    if (total_weight - 1.0).abs() > 0.01 {
        return false;
    }
    
    // Проверяем momentum_threshold
    if settings.momentum_threshold < 0.0 || settings.momentum_threshold > 1.0 {
        return false;
    }
    
    // Проверяем quick_exit_timeout_ms
    if settings.quick_exit_timeout_ms < 100 || settings.quick_exit_timeout_ms > 60_000 {
        return false;
    }
    
    true
}

/// Валидация настроек капитала
fn validate_capital_settings(capital: f64, position_size_percent: f64, leverage: f64, max_positions: usize) -> bool {
    // Проверяем капитал
    if capital <= 0.0 || capital > 1_000_000.0 {
        return false;
    }
    
    // Проверяем размер позиции
    if position_size_percent <= 0.0 || position_size_percent > 100.0 {
        return false;
    }
    
    // Проверяем плечо
    if leverage < 1.0 || leverage > 200.0 {
        return false;
    }
    
    // Проверяем максимальное количество позиций
    if max_positions == 0 || max_positions > 10 {
        return false;
    }
    
    true
}
