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
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::core::{PriceState, PositionManager, Position, TradingStats, TradeRecord, SystemManager, TradingMode};
use crate::core::trading_strategy::StrategySettings;
use crate::utils::SystemHealth;

/// Контекст одного торгового символа для WebSocket сервера
#[derive(Clone)]
pub struct SymbolContext {
    pub label: String,
    pub state_rx: watch::Receiver<PriceState>,
    pub position_manager: Arc<PositionManager>,
}

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

/// Данные по одному символу внутри composite сообщения
#[derive(Debug, Clone, Serialize)]
pub struct SymbolPayload {
    #[serde(flatten)]
    pub price_state: PriceState,
    pub positions: Vec<Position>,
    pub total_pnl: f64,
    pub open_positions_count: usize,
    pub trading_stats: Option<TradingStats>,
    pub recent_trades: Vec<TradeRecord>,
}

/// Composite WebSocket сообщение с данными по всем символам
#[derive(Debug, Clone, Serialize)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    pub symbols: BTreeMap<String, SymbolPayload>,
}

pub struct WsServer {
    symbols: Vec<SymbolContext>,
    system_manager: Arc<SystemManager>,
    system_health: Option<Arc<SystemHealth>>,
}

impl WsServer {
    pub fn new(
        symbols: Vec<SymbolContext>,
        system_manager: Arc<SystemManager>,
    ) -> Self {
        Self {
            symbols,
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
        let system_health_for_detailed = self.system_health.clone();

        Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(move || health_handler(health)))
            .route(
                "/health/detailed",
                get(move || detailed_health_handler(system_health_for_detailed)),
            )
            .with_state((self.symbols, self.system_manager))
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
    State((symbols, system_manager)): State<(
        Vec<SymbolContext>,
        Arc<SystemManager>,
    )>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, symbols, system_manager))
}

/// Собирает composite сообщение для всех символов
async fn build_ws_message(symbols: &[SymbolContext]) -> WsMessage {
    let mut map = BTreeMap::new();
    for ctx in symbols {
        let state = ctx.state_rx.borrow().clone();
        let positions = ctx.position_manager.get_open_positions().await;
        let stats = ctx.position_manager.get_stats().await;
        let trading_stats = ctx.position_manager.get_trading_stats().await;
        let recent_trades = ctx.position_manager.get_recent_trades(20).await;

        map.insert(
            ctx.label.clone(),
            SymbolPayload {
                price_state: state,
                positions,
                total_pnl: stats.total_pnl,
                open_positions_count: stats.open_positions,
                trading_stats,
                recent_trades,
            },
        );
    }
    WsMessage {
        msg_type: "marketData",
        symbols: map,
    }
}

async fn handle_socket(
    socket: WebSocket,
    symbols: Vec<SymbolContext>,
    system_manager: Arc<SystemManager>,
) {
    let (mut sender, mut receiver) = socket.split();

    info!("New WebSocket client connected");

    // Канал для отправки дополнительных сообщений (например, ответов на команды)
    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Клонируем для разных задач
    let symbols_send = symbols.clone();
    let system_manager_recv = system_manager.clone();

    // Send initial state with positions AND system state
    let initial_msg = build_ws_message(&symbols).await;

    if let Ok(json) = serde_json::to_string(&initial_msg) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Помечаем все state_rx как "прочитанные" чтобы has_changed() работал корректно дальше
    let mut symbols_for_watch: Vec<SymbolContext> = symbols
        .iter()
        .map(|c| {
            let mut ctx = c.clone();
            let _ = ctx.state_rx.borrow_and_update();
            ctx
        })
        .collect();

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
                    // Проверяем изменения хотя бы одного символа
                    let mut any_changed = false;
                    for ctx in &mut symbols_for_watch {
                        if ctx.state_rx.has_changed().unwrap_or(false) {
                            any_changed = true;
                            // Помечаем как прочитанное
                            let _ = ctx.state_rx.borrow_and_update();
                        }
                    }

                    if any_changed {
                        let ws_msg = build_ws_message(&symbols_send).await;

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

                                    // Получаем текущие настройки и сохраняем через system manager
                                    // (system manager сам применит ко всем position managers)
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
                                    // Отправляем текущие настройки первого символа (они общие для всех)
                                    let pms = system_manager_recv.position_managers();
                                    if let Some(pm) = pms.first() {
                                        let current_settings = pm.get_strategy_settings().await;
                                        if let Ok(json) = serde_json::to_string(&ServerResponse::Settings(current_settings)) {
                                            let _ = response_tx.send(json);
                                        }
                                    }
                                }
                                ClientCommand::UpdateCapital { capital, position_size_percent, leverage, max_positions } => {
                                    // Валидация настроек капитала
                                    if !validate_capital_settings(capital, position_size_percent, leverage, max_positions) {
                                        error!("Invalid capital settings received");
                                        continue;
                                    }

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
