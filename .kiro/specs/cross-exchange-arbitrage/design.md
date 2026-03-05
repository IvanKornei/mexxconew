# Design Document

## Overview

Система мониторинга и торговли арбитражем построена на высокопроизводительной архитектуре с Rust backend и Angular 21 frontend. Система фокусируется на паре BTC/USDT между Binance Futures и MEXC Futures, обеспечивая ультра-низкую латентность (<1ms) для обработки данных.

Ключевые принципы дизайна:
- Ультра-низкая латентность (<1ms) через оптимизированный парсинг JSON и lock-free структуры данных
- Отказоустойчивость через автоматическое переподключение с exponential backoff
- Модульность следуя принципам SOLID
- Реактивность через Angular Signals для эффективного управления состоянием
- Готовность к торговле через trait-based архитектуру для exchange clients

## Architecture

```mermaid
graph TB
    subgraph "Exchange Layer"
        BinanceWS[Binance Futures WS<br/>wss://fstream.binance.com/ws/btcusdt@aggTrade]
        MEXCWS[MEXC Futures WS<br/>wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT]
    end
    
    subgraph "Core Engine (Rust + Tokio)"
        PriceFeed[Price Feed Manager<br/>tungstenite/tokio-tungstenite]
        Normalizer[Symbol Normalizer<br/>BTC_USDT ↔ BTCUSDT]
        PriceState[Price State<br/>Arc<RwLock> / tokio::sync::watch]
        SpreadCalc[Spread Calculator<br/>Formula: ((MEXC-Binance)/Binance)*100]
        ExchangeTrait[ExchangeClient Trait<br/>place_order, cancel_order, get_balance]
        MEXCClient[MEXC Client Boilerplate<br/>Hmac-Sha256 Auth]
    end
    
    subgraph "API Layer"
        WebSocketServer[Axum WebSocket Server<br/>Broadcasts prices & spread]
    end
    
    subgraph "Frontend (Angular 21)"
        MarketDataService[MarketDataService<br/>WebSocket Client]
        PriceCard[PriceCardComponent<br/>Displays BTC/USDT price]
        SpreadViz[SpreadVisualizerComponent<br/>Green/Red indicator]
        Signals[Angular Signals<br/>State Management]
    end
    
    BinanceWS -->|aggTrade JSON| PriceFeed
    MEXCWS -->|deal JSON| PriceFeed
    PriceFeed -->|Raw Data| Normalizer
    Normalizer -->|Normalized Prices| PriceState
    PriceState -->|Read| SpreadCalc
    SpreadCalc -->|Spread %| WebSocketServer
    PriceState -->|Prices| WebSocketServer
    WebSocketServer -->|JSON Messages| MarketDataService
    MarketDataService -->|Update| Signals
    Signals -->|Reactive Data| PriceCard
    Signals -->|Reactive Data| SpreadViz
    ExchangeTrait -.->|Future Implementation| MEXCClient
    MEXCClient -.->|Load from .env| ConfigEnv[.env file]
```

Архитектура разделена на несколько слоев:

1. **Exchange Layer**: WebSocket подключения к Binance Futures и MEXC Futures для BTC/USDT
2. **Core Engine**: Rust backend с Tokio runtime для асинхронной обработки данных
3. **API Layer**: Axum-based WebSocket сервер для real-time коммуникации с фронтендом
4. **Frontend**: Angular 21 с Signals для реактивного UI и Tailwind CSS для стилизации

## Components and Interfaces

### 1. Price Feed Manager

**Responsibility**: Управление WebSocket подключениями к Binance Futures и MEXC Futures для получения данных о сделках BTC/USDT

**Key Interfaces**:
```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use serde::{Deserialize, Serialize};

// Binance aggTrade message
#[derive(Deserialize)]
struct BinanceAggTrade {
    #[serde(rename = "p")]
    price: String,
    #[serde(rename = "T")]
    trade_time: i64,
}

// MEXC deal message
#[derive(Deserialize)]
struct MexcDeal {
    #[serde(rename = "p")]
    price: String,
    #[serde(rename = "t")]
    trade_time: i64,
}

pub trait ExchangeConnector: Send + Sync {
    async fn connect(&mut self) -> Result<(), ConnectionError>;
    async fn receive(&mut self) -> Result<PriceUpdate, ReceiveError>;
    async fn reconnect_with_backoff(&mut self) -> Result<(), ConnectionError>;
}

pub struct BinanceFuturesConnector {
    url: String,
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    backoff: ExponentialBackoff,
}

pub struct MexcFuturesConnector {
    url: String,
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    backoff: ExponentialBackoff,
}

pub struct PriceFeedManager {
    binance: BinanceFuturesConnector,
    mexc: MexcFuturesConnector,
    price_tx: watch::Sender<PriceState>,
}

impl PriceFeedManager {
    pub async fn start(&mut self) -> Result<(), Error>;
    pub async fn run_binance_feed(&mut self) -> Result<(), Error>;
    pub async fn run_mexc_feed(&mut self) -> Result<(), Error>;
}
```

**Design Decisions**:
- Использование `tokio-tungstenite` для async WebSocket клиентов
- Отдельные async tasks для каждой биржи для параллельной обработки
- Exponential backoff для reconnection (Requirements 1.4)
- Оптимизированный парсинг JSON с serde атрибутами для минимальной латентности
- Конкретные endpoints: Binance `wss://fstream.binance.com/ws/btcusdt@aggTrade` и MEXC `wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT`

### 2. Symbol Normalizer

**Responsibility**: Нормализация символов торговых пар между биржами

**Key Interfaces**:
```rust
pub struct SymbolNormalizer;

impl SymbolNormalizer {
    pub fn normalize_binance(symbol: &str) -> String {
        // BTCUSDT -> BTC/USDT
        symbol.replace("USDT", "/USDT")
    }
    
    pub fn normalize_mexc(symbol: &str) -> String {
        // BTC_USDT -> BTC/USDT
        symbol.replace("_", "/")
    }
    
    pub fn to_canonical(symbol: &str) -> String {
        // Any format -> BTC/USDT
        symbol.to_uppercase().replace("_", "/")
    }
}
```

**Design Decisions**:
- Простая утилита для приведения символов к единому формату (Requirements 2.2)
- Canonical format: `BTC/USDT`
- Zero-allocation где возможно

### 3. Price State

**Responsibility**: Shared state для передачи цен между async tasks

**Key Interfaces**:
```rust
use tokio::sync::{RwLock, watch};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PriceState {
    pub binance_price: f64,
    pub mexc_price: f64,
    pub binance_timestamp: i64,
    pub mexc_timestamp: i64,
    pub spread_percent: f64,
}

// Option 1: Using Arc<RwLock>
pub type PriceStateShared = Arc<RwLock<PriceState>>;

// Option 2: Using tokio::sync::watch (preferred for broadcast)
pub type PriceStateWatch = watch::Receiver<PriceState>;
```

**Design Decisions**:
- `tokio::sync::watch` предпочтительнее для broadcast паттерна (Requirements 2.4)
- Lock-free reads для множественных consumers
- Atomic updates для consistency
- Microsecond precision timestamps (Requirements 1.5)

### 4. Spread Calculator

**Responsibility**: Расчет процентного спреда между ценами бирж

**Key Interfaces**:
```rust
pub struct SpreadCalculator {
    binance_fee: f64,  // e.g., 0.0004 (0.04%)
    mexc_fee: f64,     // e.g., 0.0002 (0.02%)
}

impl SpreadCalculator {
    pub fn calculate_raw_spread(&self, mexc_price: f64, binance_price: f64) -> f64 {
        ((mexc_price - binance_price) / binance_price) * 100.0
    }
    
    pub fn calculate_net_spread(&self, mexc_price: f64, binance_price: f64) -> f64 {
        let raw_spread = self.calculate_raw_spread(mexc_price, binance_price);
        let total_fees = (self.binance_fee + self.mexc_fee) * 100.0;
        raw_spread - total_fees
    }
    
    pub fn is_opportunity(&self, spread: f64, threshold: f64) -> bool {
        spread.abs() > threshold
    }
}
```

**Design Decisions**:
- Формула: `((PriceMEXC - PriceBinance) / PriceBinance) * 100` (Requirements 2.1)
- Учет комиссий для net spread (Requirements 2.3)
- Threshold 0.3% для фильтрации возможностей
- Inline функции для минимальной латентности

### 5. ExchangeClient Trait

**Responsibility**: Абстракция для взаимодействия с биржами (подготовка к торговле)

**Key Interfaces**:
```rust
use async_trait::async_trait;

#[async_trait]
pub trait ExchangeClient: Send + Sync {
    async fn place_order(&self, order: OrderRequest) -> Result<Order, ApiError>;
    async fn cancel_order(&self, order_id: &str) -> Result<(), ApiError>;
    async fn get_balance(&self, asset: &str) -> Result<Balance, ApiError>;
}

pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
}

pub struct Order {
    pub id: String,
    pub symbol: String,
    pub status: OrderStatus,
    pub filled_quantity: f64,
}

pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
}

#[derive(Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
}
```

**Design Decisions**:
- Trait-based design для поддержки множественных бирж (Requirements 3.1)
- Async methods с `async_trait` crate
- Типизированные enums для безопасности
- Подготовка инфраструктуры для будущей автоматизации

### 6. MEXC Client Boilerplate

**Responsibility**: Реализация ExchangeClient для MEXC с аутентификацией

**Key Interfaces**:
```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use reqwest::Client;

pub struct MexcClient {
    api_key: String,
    api_secret: String,
    http_client: Client,
    base_url: String,
}

impl MexcClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            http_client: Client::new(),
            base_url: "https://api.mexc.com".to_string(),
        }
    }
    
    pub fn from_env() -> Result<Self, EnvError> {
        let api_key = std::env::var("MEXC_API_KEY")?;
        let api_secret = std::env::var("MEXC_API_SECRET")?;
        Ok(Self::new(api_key, api_secret))
    }
    
    fn sign_request(&self, params: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(params.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

#[async_trait]
impl ExchangeClient for MexcClient {
    async fn place_order(&self, order: OrderRequest) -> Result<Order, ApiError> {
        // Boilerplate implementation
        todo!("Implement MEXC order placement with signed request")
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ApiError> {
        // Boilerplate implementation
        todo!("Implement MEXC order cancellation")
    }
    
    async fn get_balance(&self, asset: &str) -> Result<Balance, ApiError> {
        // Boilerplate implementation
        todo!("Implement MEXC balance query")
    }
}
```

**Design Decisions**:
- Hmac-Sha256 для подписи запросов (Requirements 3.2)
- Загрузка ключей из .env (Requirements 3.3)
- Boilerplate структура для будущей реализации (Requirements 3.4, 3.5)
- Использование `reqwest` для HTTP клиента

### 7. WebSocket API Server

**Responsibility**: Предоставление real-time данных фронтенду через WebSocket

**Key Interfaces**:
```rust
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct MarketDataMessage {
    pub binance_price: f64,
    pub mexc_price: f64,
    pub spread_percent: f64,
    pub timestamp: i64,
}

pub struct WsServer {
    price_rx: watch::Receiver<PriceState>,
}

impl WsServer {
    pub fn new(price_rx: watch::Receiver<PriceState>) -> Self {
        Self { price_rx }
    }
    
    pub fn router() -> Router {
        Router::new()
            .route("/ws", get(ws_handler))
    }
    
    async fn ws_handler(ws: WebSocketUpgrade) -> Response {
        ws.on_upgrade(handle_socket)
    }
    
    async fn handle_socket(mut socket: WebSocket) {
        // Broadcast price updates to connected client
    }
}
```

**Design Decisions**:
- Axum framework для высокой производительности (Requirements 9.1)
- JSON формат для сообщений (Requirements 9.4)
- Broadcast паттерн для множественных клиентов (Requirements 9.5)
- Lightweight сервер без REST endpoints (фокус на WebSocket)

### 8. Angular Frontend Components

**Responsibility**: Визуализация данных в реальном времени

**Key Components**:

```typescript
// market-data.service.ts
@Injectable({ providedIn: 'root' })
export class MarketDataService {
  private ws: WebSocket;
  private priceSignal = signal<MarketData | null>(null);
  
  constructor() {
    this.connect();
  }
  
  connect() {
    this.ws = new WebSocket('ws://localhost:8080/ws');
    this.ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      this.priceSignal.set(data);
    };
  }
  
  get price() {
    return this.priceSignal.asReadonly();
  }
}

// price-card.component.ts
@Component({
  selector: 'app-price-card',
  template: `
    <div class="bg-gray-800 rounded-lg p-6">
      <h3 class="text-gray-400 text-sm">{{ exchange }}</h3>
      <p class="text-white text-3xl font-bold">{{ price() | number:'1.2-2' }}</p>
      <p class="text-gray-500 text-xs">BTC/USDT</p>
    </div>
  `
})
export class PriceCardComponent {
  @Input() exchange: string;
  @Input() price: Signal<number>;
}

// spread-visualizer.component.ts
@Component({
  selector: 'app-spread-visualizer',
  template: `
    <div class="bg-gray-800 rounded-lg p-6">
      <h3 class="text-gray-400 text-sm">Spread</h3>
      <p [class]="spreadClass()" class="text-3xl font-bold">
        {{ spread() | number:'1.2-2' }}%
      </p>
    </div>
  `
})
export class SpreadVisualizerComponent {
  @Input() spread: Signal<number>;
  
  spreadClass = computed(() => {
    return this.spread() > 0 ? 'text-green-500' : 'text-red-500';
  });
}
```

**Design Decisions**:
- Angular 21 Signals для реактивного state management (Requirements 6.1)
- Tailwind CSS с темной темой (Requirements 6.4)
- Минимизация Zone.js через OnPush strategy и Signals (Requirements 10.4)
- WebSocket client в service для централизованного управления (Requirements 6.5)
- Компонентная архитектура (Requirements 6.2, 6.3)

## Data Models

### Core Types

```rust
// Exchange identification
pub enum ExchangeId {
    Binance,
    MEXC,
}

// Price and volume with high precision
pub type Price = f64;
pub type Volume = f64;

// Timestamp with microsecond precision
pub type Timestamp = i64;  // Unix timestamp in microseconds

// Price update from exchange
pub struct PriceUpdate {
    pub exchange: ExchangeId,
    pub price: Price,
    pub timestamp: Timestamp,
}
```

### Frontend Types

```typescript
interface MarketData {
  binance_price: number;
  mexc_price: number;
  spread_percent: number;
  timestamp: number;
}

interface PriceCardData {
  exchange: string;
  price: number;
}

interface SpreadData {
  spread: number;
  isPositive: boolean;
}
```

## Error Handling

### Error Types Hierarchy

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Failed to connect to {exchange}: {reason}")]
    ConnectFailed { exchange: ExchangeId, reason: String },
    
    #[error("Connection lost to {exchange}")]
    Disconnected { exchange: ExchangeId },
    
    #[error("Reconnection failed after {attempts} attempts")]
    ReconnectFailed { attempts: u32 },
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),
    
    #[error("Invalid API credentials")]
    InvalidCredentials,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}
```

### Error Handling Strategy

1. **Connection Errors**: Автоматический retry с exponential backoff, максимум 5 попыток
2. **API Errors**: Логирование и notification, retry для transient errors (rate limits, timeouts)
3. **Parse Errors**: Логирование и skip invalid messages
4. **Critical Errors**: Graceful shutdown с сохранением состояния

### Logging Strategy

```rust
// Structured logging with tracing crate
use tracing::{info, warn, error, debug};

// Example usage
info!(
    exchange = ?ExchangeId::Binance,
    price = %price,
    "Price update received"
);

error!(
    exchange = ?ExchangeId::MEXC,
    error = %e,
    "Failed to connect"
);
```

**Log Levels**:
- `ERROR`: Connection failures, API errors, critical errors
- `WARN`: Reconnection attempts, parse errors
- `INFO`: Price updates, spread calculations, system state changes
- `DEBUG`: WebSocket messages, internal state

**Log Rotation**: (Requirements 8.4, 8.5)
- Max file size: 100 MB
- Retention: 30 days
- Format: JSON для structured logging

## Testing Strategy

### Unit Testing

**Scope**: Тестирование отдельных компонентов в изоляции

**Key Areas**:
1. **Spread Calculator**:
   - Raw spread calculation
   - Net spread с комиссиями
   - Opportunity detection

2. **Symbol Normalizer**:
   - Binance format normalization
   - MEXC format normalization
   - Canonical format conversion

3. **Price State**:
   - Concurrent reads/writes
   - Timestamp accuracy

**Tools**: 
- `cargo test` для unit tests
- Mock implementations для WebSocket connections

### Integration Testing

**Scope**: Тестирование взаимодействия между компонентами

**Key Scenarios**:
1. **End-to-End Data Flow**:
   - WebSocket → Price Feed → Normalizer → Price State → Spread Calculator → WebSocket Server
   - Verify latency < 1ms

2. **Error Recovery**:
   - WebSocket disconnect → Reconnect with backoff
   - Invalid JSON → Skip and continue

3. **Frontend Integration**:
   - WebSocket connection from Angular
   - Real-time updates in UI

**Tools**:
- Integration test suite с `tokio::test`
- Mock WebSocket servers
- Angular testing utilities

### Performance Testing

**Scope**: Валидация latency requirements

**Key Metrics**:
- JSON parsing latency < 100μs
- Spread calculation latency < 10μs
- End-to-end latency < 1ms (Requirements 1.3)

**Tools**:
- `criterion` для benchmarking
- `flamegraph` для profiling

## Deployment Considerations

### System Requirements

- **CPU**: 2+ cores
- **RAM**: 2GB+
- **Network**: Low-latency connection к биржам (<50ms ping)
- **OS**: Linux/Windows/macOS

### Configuration

```toml
# config.toml
[system]
log_level = "info"
log_dir = "./logs"

[exchanges]
binance_ws = "wss://fstream.binance.com/ws/btcusdt@aggTrade"
mexc_ws = "wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT"

[trading]
min_spread_percent = 0.3
binance_fee = 0.0004
mexc_fee = 0.0002

[api]
host = "0.0.0.0"
port = 8080
```

```.env
# .env file
MEXC_API_KEY=your_api_key_here
MEXC_API_SECRET=your_api_secret_here
```

### Monitoring

- Health check endpoint: `GET /health`
- Structured logs для debugging
- WebSocket connection status monitoring
