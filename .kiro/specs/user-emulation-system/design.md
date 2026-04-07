# Design Document

## Overview

Система эмуляции пользовательского поведения для MEXC Futures представляет собой многоуровневую архитектуру, сочетающую браузерную автоматизацию для инициализации сессии с высокоскоростным WebSocket API для торговли. Ключевая идея - разделение на "медленную" фазу подготовки (15-30 сек) и "быструю" фазу торговли (<1мс латентность).

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    EmulationSystem                          │
│  ┌──────────────────┐         ┌─────────────────────────┐  │
│  │ SessionInitializer│────────▶│  SessionPersistence     │  │
│  │  (eoka browser)   │         │  (encrypted storage)    │  │
│  └──────────────────┘         └─────────────────────────┘  │
│           │                              │                   │
│           ▼                              ▼                   │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              TradingExecutor                          │  │
│  │  ┌────────────┐  ┌──────────────┐  ┌─────────────┐  │  │
│  │  │ WebSocket  │  │ HumanJitter  │  │ RateLimiter │  │  │
│  │  │ (wreq)     │  │ (optional)   │  │             │  │  │
│  │  └────────────┘  └──────────────┘  └─────────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
│           │                              │                   │
│           ▼                              ▼                   │
│  ┌──────────────────┐         ┌─────────────────────────┐  │
│  │PeriodicActivity  │         │   ParameterManager      │  │
│  │   Simulator      │         │                         │  │
│  └──────────────────┘         └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
         │                                      │
         ▼                                      ▼
    ┌─────────┐                          ┌──────────┐
    │  MEXC   │                          │ Binance  │
    │ Futures │                          │ (monitor)│
    └─────────┘                          └──────────┘
```

### Component Interaction Flow

**Phase 1: Session Initialization (15-30 seconds, once at startup)**
1. SessionInitializer запускает eoka browser с Chrome 145 профилем
2. BehaviorSimulator эмулирует логин, навигацию, настройку параметров
3. SessionPersistence сохраняет cookies, токены, TLS session tickets
4. ParameterManager сохраняет торговые параметры (размер позиции, leverage)

**Phase 2: High-Speed Trading (continuous, <1ms latency)**
1. TradingExecutor подключается к Binance WebSocket (мониторинг)
2. TradingExecutor подключается к MEXC WebSocket (торговля) с сохраненными токенами
3. При сигнале арбитража: отправка ордера на MEXC через WebSocket
4. HumanJitter добавляет задержку 20-80мс (только в Hybrid режиме)

**Phase 3: Background Activity (periodic, every 10-60 minutes)**
1. PeriodicActivitySimulator выполняет фоновые UI-действия в отдельном потоке
2. Проверка баланса, просмотр позиций, случайный просмотр других пар
3. Не блокирует TradingExecutor



## Components and Interfaces

### 1. SessionInitializer

**Responsibility:** Выполнение полной браузерной эмуляции для получения легитимной сессии

**Technology:** eoka (Rust browser automation library)

**Key Methods:**
```rust
pub struct SessionInitializer {
    browser: EokaBrowser,
    behavior_simulator: BehaviorSimulator,
    tls_emulator: TLSEmulator,
}

impl SessionInitializer {
    pub async fn initialize_session(&mut self) -> Result<SessionData, EmulationError> {
        // 1. Launch Chrome 145 with TLS fingerprints
        // 2. Navigate to MEXC login page
        // 3. Simulate login with typing delays
        // 4. Navigate to Futures Trading
        // 5. Simulate chart viewing and parameter setup
        // 6. Extract cookies and tokens
    }
}
```

**Configuration:**
- Browser profile: Chrome 145 (64-bit) Windows
- Viewport: 1920x1080
- User-Agent: синхронизирован с TLS fingerprint
- Headless: false (для обхода детектирования)

### 2. TLSEmulator

**Responsibility:** Генерация браузерных TLS и HTTP/2 отпечатков

**Technology:** wreq + BoringSSL

**Key Methods:**
```rust
pub struct TLSEmulator {
    profile: BrowserProfile,
}

pub enum BrowserProfile {
    Chrome145Windows64,
    // Возможность расширения для других браузеров
}

impl TLSEmulator {
    pub fn generate_ja3_fingerprint(&self) -> String {
        // Генерация JA3 hash для Chrome 145
    }
    
    pub fn generate_ja4_fingerprint(&self) -> String {
        // Генерация JA4 hash с алфавитной сортировкой
    }
    
    pub fn configure_http2_settings(&self) -> Http2Settings {
        // SETTINGS frames, WINDOW_UPDATE, PRIORITY
    }
}
```

**Chrome 145 TLS Parameters:**
- Cipher Suites: TLS_AES_128_GCM_SHA256, TLS_AES_256_GCM_SHA384, TLS_CHACHA20_POLY1305_SHA256
- Extensions: server_name, supported_groups, ec_point_formats, signature_algorithms, ALPN
- Supported Groups: x25519, secp256r1, secp384r1
- ALPN: h2, http/1.1

### 3. BehaviorSimulator

**Responsibility:** Имитация человеческого поведения (движения мыши, клики, ввод текста)

**Key Methods:**
```rust
pub struct BehaviorSimulator {
    rng: ThreadRng,
}

impl BehaviorSimulator {
    pub async fn simulate_mouse_movement(&self, from: Point, to: Point) -> Vec<Point> {
        // Генерация траектории по кривой Безье
        // B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
    }
    
    pub async fn simulate_typing(&self, text: &str, delay_range: (u64, u64)) {
        // Ввод текста с случайными задержками между символами
    }
    
    pub async fn simulate_click(&self, element: Element) {
        // Движение мыши к элементу + клик с задержкой
    }
    
    pub async fn simulate_scroll(&self, distance: i32) {
        // Прокрутка с ускорением/замедлением
    }
}
```

**Bezier Curve Implementation:**
```rust
fn bezier_curve(p0: Point, p3: Point, t: f64) -> Point {
    // Генерация случайных контрольных точек P1, P2
    let p1 = generate_control_point(p0, p3, 0.3);
    let p2 = generate_control_point(p0, p3, 0.7);
    
    let x = (1.0 - t).powi(3) * p0.x 
          + 3.0 * (1.0 - t).powi(2) * t * p1.x
          + 3.0 * (1.0 - t) * t.powi(2) * p2.x
          + t.powi(3) * p3.x;
    
    let y = // аналогично для y
    
    Point { x, y }
}
```



### 4. TradingExecutor

**Responsibility:** Высокоскоростное выполнение сделок через WebSocket API

**Technology:** wreq + tokio-tungstenite

**Key Methods:**
```rust
pub struct TradingExecutor {
    mexc_ws: WreqWebSocket,
    binance_ws: WebSocket,
    session_data: Arc<RwLock<SessionData>>,
    human_jitter: Option<HumanJitter>,
    mode: TradingMode,
}

impl TradingExecutor {
    pub async fn connect_to_mexc(&mut self) -> Result<(), EmulationError> {
        // Подключение к MEXC WebSocket с сохраненными токенами
        // Использование wreq для сохранения TLS fingerprints
    }
    
    pub async fn connect_to_binance(&mut self) -> Result<(), EmulationError> {
        // Подключение к Binance WebSocket (чистое API, без эмуляции)
    }
    
    pub async fn execute_order(&mut self, order: Order) -> Result<OrderResponse, EmulationError> {
        // В Hybrid режиме: добавить HumanJitter задержку
        // Отправить ордер через MEXC WebSocket
        // Латентность: <1мс (без jitter) или 20-80мс (с jitter)
    }
    
    pub async fn monitor_arbitrage(&mut self) {
        // Мониторинг цен Binance и MEXC
        // При обнаружении отставания: вызов execute_order
    }
}
```

**WebSocket Message Format (MEXC):**
```json
{
  "method": "order.place",
  "params": {
    "symbol": "BTC_USDT",
    "side": "BUY",
    "type": "LIMIT",
    "quantity": "0.001",
    "price": "50000",
    "timestamp": 1234567890
  },
  "id": "unique_request_id"
}
```

### 5. HumanJitter

**Responsibility:** Добавление случайных задержек для имитации человеческой реакции

**Key Methods:**
```rust
pub struct HumanJitter {
    rng: ThreadRng,
    distribution: Normal<f64>,
}

impl HumanJitter {
    pub fn new() -> Self {
        // Нормальное распределение: mean=50мс, std_dev=15мс
        let distribution = Normal::new(50.0, 15.0).unwrap();
        Self { rng: thread_rng(), distribution }
    }
    
    pub async fn apply_jitter(&mut self) {
        let delay_ms = self.distribution.sample(&mut self.rng).clamp(20.0, 80.0);
        tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
    }
}
```

### 6. SessionPersistence

**Responsibility:** Сохранение и восстановление состояния сессии

**Key Methods:**
```rust
pub struct SessionPersistence {
    storage_path: PathBuf,
    encryption_key: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    pub cookies: Vec<Cookie>,
    pub auth_token: String,
    pub tls_session_ticket: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SessionPersistence {
    pub async fn save_session(&self, data: &SessionData) -> Result<(), EmulationError> {
        // Сериализация в JSON
        // Шифрование AES-256-GCM
        // Запись в файл .kiro/emulation/session.enc
    }
    
    pub async fn load_session(&self) -> Result<Option<SessionData>, EmulationError> {
        // Чтение из файла
        // Расшифровка
        // Проверка валидности (expires_at)
    }
    
    pub fn is_session_valid(&self, data: &SessionData) -> bool {
        Utc::now() < data.expires_at
    }
}
```



### 7. PeriodicActivitySimulator

**Responsibility:** Фоновая имитация человеческой активности

**Key Methods:**
```rust
pub struct PeriodicActivitySimulator {
    browser: Arc<Mutex<EokaBrowser>>,
    behavior_simulator: BehaviorSimulator,
    mode: TradingMode,
    last_activity: Instant,
}

impl PeriodicActivitySimulator {
    pub async fn run_background_loop(&mut self) {
        loop {
            let interval = self.calculate_next_interval();
            tokio::time::sleep(interval).await;
            
            self.perform_random_activity().await;
        }
    }
    
    async fn perform_random_activity(&mut self) {
        let activities = vec![
            Activity::CheckBalance,
            Activity::ViewOpenPositions,
            Activity::ViewTradeHistory,
            Activity::ViewOtherPair("ETH_USDT"),
        ];
        
        let activity = activities.choose(&mut thread_rng()).unwrap();
        self.execute_activity(activity).await;
    }
    
    fn calculate_next_interval(&self) -> Duration {
        match self.mode {
            TradingMode::Hybrid => Duration::from_secs(thread_rng().gen_range(600..1200)), // 10-20 min
            TradingMode::Aggressive => Duration::from_secs(thread_rng().gen_range(1800..3600)), // 30-60 min
        }
    }
}
```

### 8. ParameterManager

**Responsibility:** Управление торговыми параметрами

**Key Methods:**
```rust
pub struct ParameterManager {
    position_size: f64,
    leverage: u8,
    stop_loss_percent: f64,
    take_profit_percent: f64,
    last_update: Instant,
}

impl ParameterManager {
    pub async fn initialize_parameters(&mut self, browser: &mut EokaBrowser) {
        // Эмуляция ввода параметров через UI
        // Сохранение значений для переиспользования
    }
    
    pub async fn update_parameters_if_needed(&mut self, browser: &mut EokaBrowser) {
        if self.last_update.elapsed() > Duration::from_secs(3600) {
            // Обновление параметров раз в час через UI
            self.initialize_parameters(browser).await;
        }
    }
    
    pub fn get_order_parameters(&self) -> OrderParameters {
        OrderParameters {
            size: self.position_size,
            leverage: self.leverage,
            stop_loss: self.stop_loss_percent,
            take_profit: self.take_profit_percent,
        }
    }
}
```

## Data Models

### SessionData
```rust
pub struct SessionData {
    pub cookies: Vec<Cookie>,
    pub auth_token: String,
    pub refresh_token: Option<String>,
    pub tls_session_ticket: Vec<u8>,
    pub user_agent: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
```

### TradingMode
```rust
pub enum TradingMode {
    Hybrid,      // По умолчанию: задержки + фоновая активность
    Aggressive,  // Опциональный: минимальные задержки
}
```

### EmulationConfig
```rust
#[derive(Deserialize)]
pub struct EmulationConfig {
    pub trading_mode: TradingMode,
    pub browser_profile: BrowserProfile,
    pub session_storage_path: PathBuf,
    pub proxy: Option<ProxyConfig>,
    pub rate_limits: RateLimitConfig,
}

#[derive(Deserialize)]
pub struct RateLimitConfig {
    pub max_orders_per_minute: u32,
    pub background_actions_per_hour: u32,
}
```

### Order
```rust
pub struct Order {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub timestamp: i64,
}

pub enum OrderSide {
    Buy,
    Sell,
}

pub enum OrderType {
    Market,
    Limit,
}
```



## Error Handling

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum EmulationError {
    #[error("Session initialization failed: {0}")]
    SessionInitFailed(String),
    
    #[error("Browser automation error: {0}")]
    BrowserError(String),
    
    #[error("WebSocket connection error: {0}")]
    WebSocketError(String),
    
    #[error("Authentication failed: {0}")]
    AuthError(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),
    
    #[error("Session expired")]
    SessionExpired,
    
    #[error("Blocked by anti-bot system: {0}")]
    BlockedError(String),
}
```

### Error Recovery Strategy

**SessionInitFailed / BrowserError:**
- Retry с экспоненциальной задержкой (1s, 2s, 4s, 8s)
- Максимум 3 попытки
- После 3 неудач: логирование и остановка системы

**WebSocketError:**
- Автоматическое переподключение с экспоненциальной задержкой
- Сохранение состояния сессии
- Восстановление подписок на рыночные данные

**AuthError / SessionExpired:**
- Автоматический запуск SessionInitializer
- Полная переинициализация сессии
- Продолжение работы с новой сессией

**RateLimitError:**
- Экспоненциальное увеличение задержек между запросами
- Логирование для анализа паттернов
- Временная пауза в торговле (1-5 минут)

**BlockedError (429, 403, Cloudflare):**
- Если Aggressive режим: переключение на Hybrid
- Полная переинициализация сессии через SessionInitializer
- Увеличение интервалов фоновой активности
- Логирование деталей блокировки для анализа

## Testing Strategy

### Unit Tests

**TLSEmulator:**
- Проверка корректности JA3/JA4 fingerprints для Chrome 145
- Валидация HTTP/2 SETTINGS frames
- Проверка порядка псевдо-заголовков

**BehaviorSimulator:**
- Проверка генерации кривых Безье
- Валидация диапазонов задержек
- Проверка стохастического распределения

**HumanJitter:**
- Проверка нормального распределения задержек
- Валидация диапазона 20-80мс
- Проверка отсутствия фиксированных паттернов

**SessionPersistence:**
- Проверка шифрования/расшифровки
- Валидация проверки срока действия сессии
- Проверка обработки поврежденных файлов

### Integration Tests

**Session Initialization Flow:**
- Полный цикл инициализации сессии
- Проверка сохранения cookies и токенов
- Валидация TLS fingerprints в WebSocket соединении

**Trading Execution Flow:**
- Подключение к MEXC WebSocket с сохраненной сессией
- Отправка тестового ордера
- Проверка латентности (<1мс без jitter)

**Mode Switching:**
- Переключение Aggressive → Hybrid при блокировке
- Проверка изменения интервалов фоновой активности
- Валидация добавления HumanJitter задержек

### Performance Tests

**Latency Benchmarks:**
- Измерение латентности отправки ордера: target <1мс (Aggressive), <100мс (Hybrid)
- Измерение времени инициализации сессии: target 15-30 секунд
- Измерение overhead фоновой активности: должен быть <1% CPU

**Load Tests:**
- Симуляция 100 ордеров в минуту
- Проверка стабильности WebSocket соединения
- Мониторинг использования памяти

### Manual Testing Checklist

- [ ] Визуальная проверка браузерной эмуляции (движения мыши выглядят естественно)
- [ ] Проверка на реальном MEXC аккаунте (testnet)
- [ ] Мониторинг логов MEXC на предмет предупреждений о боте
- [ ] Проверка работы в Aggressive режиме до получения блокировки
- [ ] Проверка автоматического переключения на Hybrid после блокировки



## Configuration

### config.toml Example

```toml
[emulation]
trading_mode = "Hybrid"  # или "Aggressive"
browser_profile = "Chrome145Windows64"
session_storage_path = ".kiro/emulation/session.enc"
encryption_key_env = "EMULATION_SESSION_KEY"

[emulation.rate_limits]
max_orders_per_minute = 60
background_actions_per_hour_hybrid = 6      # каждые 10 минут
background_actions_per_hour_aggressive = 2  # каждые 30 минут

[emulation.jitter]
enabled_in_hybrid = true
min_delay_ms = 20
max_delay_ms = 80
mean_delay_ms = 50
std_dev_ms = 15

[emulation.session]
max_session_duration_hours = 24
auto_refresh_before_expiry_minutes = 30

[emulation.proxy]
enabled = false
# Опционально: residential proxy для масштабирования
# provider = "brightdata"
# sticky_session = true

[emulation.behavior]
typing_delay_range_ms = [100, 300]
mouse_movement_steps = 50
scroll_acceleration = true

[emulation.periodic_activity]
check_balance_probability = 0.3
view_positions_probability = 0.4
view_history_probability = 0.2
view_other_pair_probability = 0.1
break_duration_minutes = [2, 5]
break_interval_hours = [2, 3]

[mexc]
futures_url = "https://futures.mexc.com"
websocket_url = "wss://contract.mexc.com/ws"
api_key_env = "MEXC_API_KEY"
api_secret_env = "MEXC_API_SECRET"

[binance]
websocket_url = "wss://fstream.binance.com/ws"
# Binance используется только для мониторинга, без эмуляции
```

## Dependencies

### Rust Crates

```toml
[dependencies]
# Browser automation
eoka = "0.1"  # Скрытая браузерная автоматизация

# HTTP/WebSocket с TLS эмуляцией
wreq = "0.12"  # Fork reqwest с BoringSSL
wreq-util = "0.1"  # Профили браузеров
boring = "4.0"  # BoringSSL bindings
tokio-tungstenite = "0.21"

# Fingerprinting
spider_fingerprint = "0.1"  # JavaScript fingerprint генерация

# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Cryptography
aes-gcm = "0.10"  # Шифрование сессий
rand = "0.8"

# Math для Bezier curves
rand_distr = "0.4"  # Нормальное распределение для jitter

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Time
chrono = "0.4"
```

## Security Considerations

### Session Storage
- Cookies и токены шифруются AES-256-GCM
- Ключ шифрования хранится в переменной окружения
- Файл сессии имеет права доступа 600 (только владелец)

### Credential Management
- API ключи MEXC загружаются из .env файла
- Никогда не логируются в plaintext
- Используются только для HMAC подписи запросов

### Browser Fingerprinting
- Консистентность между User-Agent, WebGL, Canvas
- Избегание детектируемых артефактов (navigator.webdriver)
- Ротация fingerprints при переинициализации сессии

### Rate Limiting
- Жесткие лимиты на частоту запросов
- Экспоненциальный backoff при ошибках
- Мониторинг паттернов для предотвращения блокировок

## Performance Optimization

### Memory Management
- Использование Arc<RwLock> для shared state
- Минимизация клонирования больших структур
- Переиспользование WebSocket соединений

### Latency Optimization
- Zero-copy десериализация WebSocket сообщений
- Прямая отправка ордеров без промежуточных буферов
- Использование tokio::spawn для параллельной обработки

### CPU Optimization
- Фоновая активность в отдельном потоке с низким приоритетом
- Ленивая инициализация браузера (только при необходимости)
- Кэширование TLS fingerprints

## Monitoring and Observability

### Metrics
- Латентность отправки ордеров (p50, p95, p99)
- Количество успешных/неудачных инициализаций сессии
- Частота фоновых действий
- Количество переключений режимов (Aggressive → Hybrid)

### Logging
- Структурированное логирование через tracing
- Уровни: ERROR, WARN, INFO, DEBUG, TRACE
- Ротация логов: 100MB, хранение 30 дней

### Alerts
- Блокировка MEXC (403, 429, Cloudflare)
- Превышение rate limits
- Ошибки инициализации сессии
- Аномальная латентность (>100мс в Aggressive режиме)
