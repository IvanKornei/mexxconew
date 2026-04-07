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

use crate::core::{PriceState, PositionManager, Position, TradingStats, TradeRecord, SystemManager, TradingMode};
use crate::core::trading_strategy::StrategySettings;
use crate::utils::{SystemHealth};
use crate::api::emulation_status::{EmulationStatus, get_emulation_status};

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
    #[serde(rename = "startTrading")]
    StartTrading,
    #[serde(rename = "stopTrading")]
    StopTrading,
    #[serde(rename = "switchMode")]
    SwitchMode { mode: String },
    #[serde(rename = "getSystemState")]
    GetSystemState,
}

/// Ответы сервера
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ServerResponse {
    #[serde(rename = "settings")]
    Settings(StrategySettings),
    #[serde(rename = "systemState")]
    SystemState {
        is_running: bool,
        mode: String,
        last_updated: i64,
        trading_settings: crate::core::TradingSettings,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "confirmationRequired")]
    ConfirmationRequired { action: String, message: String },
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
    system_manager: Arc<SystemManager>,
    system_health: Option<Arc<SystemHealth>>,
}

impl WsServer {
    pub fn new(
        state_rx: watch::Receiver<PriceState>, 
        position_manager: Arc<PositionManager>,
        system_manager: Arc<SystemManager>,
    ) -> Self {
        Self { 
            state_rx,
            position_manager,
            system_manager,
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
        
        // Create emulation status state
        let emulation_status = Arc::new(tokio::sync::RwLock::new(
            EmulationStatus::default()
        ));
        
        // Create emulation status router with its own state
        let emulation_router = Router::new()
            .route("/api/emulation/status", get(get_emulation_status))
            .with_state(emulation_status);
        
        // Main router
        let main_router = Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(move || health_handler(health)))
            .route("/health/detailed", get(move || detailed_health_handler(self.system_health.clone())))
            .with_state((self.state_rx, self.position_manager, self.system_manager));
        
        // Merge routers
        main_router
            .merge(emulation_router)
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
    State((state_rx, position_manager, system_manager)): State<(
        watch::Receiver<PriceState>, 
        Arc<PositionManager>,
        Arc<SystemManager>,
    )>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state_rx, position_manager, system_manager))
}

async fn handle_socket(
    socket: WebSocket, 
    mut state_rx: watch::Receiver<PriceState>,
    position_manager: Arc<PositionManager>,
    system_manager: Arc<SystemManager>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    info!("New WebSocket client connected");
    
    // Канал для отправки дополнительных сообщений (например, ответов на команды)
    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    
    // Клонируем для разных задач
    let position_manager_send = position_manager.clone();
    let position_manager_recv = position_manager.clone();
    let system_manager_recv = system_manager.clone();
    
    // Send initial state with positions AND system state
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
    
    // Send initial system state
    let system_state = system_manager.get_state().await;
    let system_state_msg = ServerResponse::SystemState {
        is_running: system_state.is_running,
        mode: format!("{:?}", system_state.mode),
        last_updated: system_state.last_updated,
        trading_settings: system_state.trading_settings,
    };
    if let Ok(json) = serde_json::to_string(&system_state_msg) {
        let _ = sender.send(Message::Text(json)).await;
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
                                    
                                    // Обновляем настройки в position manager
                                    position_manager_recv.update_strategy_settings(
                                        settings.momentum_weight,
                                        settings.lag_weight,
                                        settings.price_diff_weight,
                                        settings.momentum_threshold,
                                        settings.quick_exit_timeout_ms,
                                    ).await;
                                    
                                    // Получаем текущие настройки и сохраняем через system manager
                                    let current_state = system_manager_recv.get_state().await;
                                    let mut updated_settings = current_state.trading_settings;
                                    updated_settings.momentum_weight = settings.momentum_weight;
                                    updated_settings.lag_weight = settings.lag_weight;
                                    updated_settings.price_diff_weight = settings.price_diff_weight;
                                    updated_settings.momentum_threshold = settings.momentum_threshold;
                                    updated_settings.quick_exit_timeout = settings.quick_exit_timeout_ms;
                                    
                                    if let Err(e) = system_manager_recv.update_settings(updated_settings).await {
                                        error!("Failed to save settings: {}", e);
                                    }
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
                                    
                                    // Обновляем настройки в position manager
                                    position_manager_recv.update_capital_settings(
                                        capital,
                                        position_size_percent,
                                        leverage,
                                        max_positions,
                                    ).await;
                                    
                                    // Получаем текущие настройки и сохраняем через system manager
                                    let current_state = system_manager_recv.get_state().await;
                                    let mut updated_settings = current_state.trading_settings;
                                    updated_settings.capital = capital;
                                    updated_settings.position_size_percent = position_size_percent;
                                    updated_settings.leverage = leverage;
                                    updated_settings.max_positions = max_positions;
                                    
                                    if let Err(e) = system_manager_recv.update_settings(updated_settings).await {
                                        error!("Failed to save settings: {}", e);
                                    }
                                }
                                ClientCommand::StartTrading => {
                                    match system_manager_recv.start_trading().await {
                                        Ok(_) => {
                                            let state = system_manager_recv.get_state().await;
                                            let msg = ServerResponse::SystemState {
                                                is_running: state.is_running,
                                                mode: format!("{:?}", state.mode),
                                                last_updated: state.last_updated,
                                                trading_settings: state.trading_settings,
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                        Err(e) => {
                                            let msg = ServerResponse::Error {
                                                message: format!("Failed to start trading: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                    }
                                }
                                ClientCommand::StopTrading => {
                                    match system_manager_recv.stop_trading().await {
                                        Ok(_) => {
                                            let state = system_manager_recv.get_state().await;
                                            let msg = ServerResponse::SystemState {
                                                is_running: state.is_running,
                                                mode: format!("{:?}", state.mode),
                                                last_updated: state.last_updated,
                                                trading_settings: state.trading_settings,
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                        Err(e) => {
                                            let msg = ServerResponse::Error {
                                                message: format!("Failed to stop trading: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                    }
                                }
                                ClientCommand::SwitchMode { mode } => {
                                    let trading_mode = match mode.as_str() {
                                        "emulation" | "Emulation" => TradingMode::Emulation,
                                        "live" | "Live" => TradingMode::Live,
                                        _ => {
                                            let msg = ServerResponse::Error {
                                                message: format!("Invalid mode: {}", mode),
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                            continue;
                                        }
                                    };
                                    
                                    match system_manager_recv.switch_mode(trading_mode).await {
                                        Ok(_) => {
                                            let state = system_manager_recv.get_state().await;
                                            let msg = ServerResponse::SystemState {
                                                is_running: state.is_running,
                                                mode: format!("{:?}", state.mode),
                                                last_updated: state.last_updated,
                                                trading_settings: state.trading_settings,
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                        Err(e) => {
                                            let msg = ServerResponse::Error {
                                                message: format!("{}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                let _ = response_tx.send(json);
                                            }
                                        }
                                    }
                                }
                                ClientCommand::GetSystemState => {
                                    let state = system_manager_recv.get_state().await;
                                    let msg = ServerResponse::SystemState {
                                        is_running: state.is_running,
                                        mode: format!("{:?}", state.mode),
                                        last_updated: state.last_updated,
                                        trading_settings: state.trading_settings,
                                    };
                                    if let Ok(json) = serde_json::to_string(&msg) {
                                        let _ = response_tx.send(json);
                                    }
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
