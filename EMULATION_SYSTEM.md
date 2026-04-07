# Система эмуляции пользователя для MEXC

## Обзор

Гибридная система эмуляции для работы с MEXC Futures, сочетающая браузерную автоматизацию для инициализации сессии с высокоскоростным WebSocket API для торговли.

## Архитектура

### Компоненты

1. **SessionInitializer** - Браузерная эмуляция для получения легитимной сессии
2. **TradingExecutor** - Высокоскоростное выполнение сделок через WebSocket
3. **PeriodicActivitySimulator** - Фоновая имитация человеческой активности
4. **ModeManager** - Управление режимами Hybrid/Aggressive
5. **RecoveryHandler** - Восстановление после ошибок
6. **MEXCEmulator** - Адаптер ExchangeClient для интеграции

### Режимы работы

#### Hybrid Mode (по умолчанию)
- Добавляет задержки 20-80мс перед каждым ордером
- Фоновые действия каждые 10-20 минут
- Имитация перерывов каждые 2-3 часа
- Проверка позиций после 15-25 сделок

#### Aggressive Mode
- Минимальные задержки (<1мс)
- Фоновые действия каждые 30-60 минут
- Без перерывов
- Автоматическое переключение на Hybrid при блокировке

## Конфигурация

### config.toml

```toml
[emulation]
trading_mode = "Hybrid"  # "Hybrid" или "Aggressive"
browser_profile = "Chrome145Windows64"
session_storage_path = ".kiro/emulation/session.enc"
encryption_key_env = "EMULATION_SESSION_KEY"

[emulation.rate_limits]
max_orders_per_minute = 60
background_actions_per_hour_hybrid = 6
background_actions_per_hour_aggressive = 2

[emulation.jitter]
enabled_in_hybrid = true
min_delay_ms = 20
max_delay_ms = 80
mean_delay_ms = 50
std_dev_ms = 15
```

### Переменные окружения

```bash
# Ключ шифрования для сессий (32 байта в hex)
EMULATION_SESSION_KEY=0000000000000000000000000000000000000000000000000000000000000000

# Опционально: учетные данные MEXC для автоматического логина
MEXC_USERNAME=your_username
MEXC_PASSWORD=your_password
```

## Использование

### Базовое использование

```rust
use crate::exchanges::MEXCEmulator;
use crate::emulation::config::EmulationConfig;

// Загрузить конфигурацию
let config = EmulationConfig::default();

// Создать эмулятор (автоматически инициализирует сессию)
let mexc = MEXCEmulator::new(config).await?;

// Использовать как обычный ExchangeClient
let order = OrderRequest {
    symbol: "BTC/USDT".to_string(),
    side: OrderSide::Buy,
    order_type: OrderType::Market,
    quantity: Decimal::from_f64_retain(0.001).unwrap(),
    price: None,
};

let response = mexc.place_order(order).await?;
```

### Интеграция в существующий код

Замените обычный MEXC коннектор на MEXCEmulator:

```rust
// Было:
// let mexc = MexcFuturesConnector::new(config);

// Стало:
let emulation_config = config.emulation.unwrap_or_default();
let mexc = MEXCEmulator::new(emulation_config).await?;
```

## Мониторинг

### Dashboard

Откройте `http://localhost:4200` для просмотра:
- Текущий режим торговли
- Статус сессии и время до истечения
- Количество фоновых действий
- Метрики латентности (P50/P95/P99)

### API Endpoint

```bash
curl http://localhost:3001/api/emulation/status
```

Ответ:
```json
{
  "trading_mode": "Hybrid",
  "session_valid": true,
  "session_created_at": "2026-03-06T04:00:00Z",
  "session_expires_at": "2026-03-07T04:00:00Z",
  "background_actions_count": 12,
  "latency_p50_ms": 0.8,
  "latency_p95_ms": 2.1,
  "latency_p99_ms": 5.3,
  "last_activity": "2026-03-06T05:30:00Z"
}
```

## Компоненты

### TLS Fingerprinting
- Эмуляция Chrome 145 (64-bit) Windows
- JA3/JA4 fingerprints
- HTTP/2 SETTINGS frames
- Упорядоченные заголовки

### Behavior Simulation
- Движение мыши по кривой Безье
- Ввод текста с задержками 100-300мс
- Клики с паузами 50-200мс
- Прокрутка с ускорением/замедлением

### Session Management
- Шифрование AES-256-GCM
- Автоматическая проверка валидности
- Переиспользование между перезапусками
- Автоматическое обновление

### Error Recovery
- AuthError/SessionExpired → Переинициализация сессии
- RateLimitError → Экспоненциальный backoff
- BlockedError → Переключение на Hybrid + переинициализация
- WebSocketError → Переподключение с backoff

## Производительность

### Латентность
- Aggressive mode: <1мс (без jitter)
- Hybrid mode: 20-80мс (с jitter)
- Инициализация сессии: 15-30 секунд

### Ресурсы
- CPU: <1% для фоновой активности
- Memory: ~50MB для эмуляции
- Network: Минимальный overhead

## Безопасность

### Рекомендации
1. Используйте Hybrid mode по умолчанию
2. Работайте с домашнего IP (без прокси)
3. Ограничьте частоту фоновых действий до 10/час
4. Мониторьте алерты о блокировках
5. Храните ключ шифрования в безопасности

### Rate Limiting
- Home IP: 10 запросов/час для фоновых действий
- С прокси: без ограничений
- Автоматическое переключение на Hybrid при 429

## Troubleshooting

### Проблема: Session initialization failed
**Решение**: Проверьте учетные данные MEXC в .env файле

### Проблема: Blocked by anti-bot system
**Решение**: Система автоматически переключится на Hybrid mode и переинициализирует сессию

### Проблема: High latency in Hybrid mode
**Решение**: Это нормально - jitter добавляет 20-80мс для имитации человека

### Проблема: Session expired
**Решение**: Система автоматически переинициализирует сессию

## Разработка

### Тестирование

```bash
# Unit тесты
cargo test --package arbitrage-system --lib emulation

# Integration тесты
cargo test --package arbitrage-system --test emulation_integration

# Benchmarks
cargo bench --package arbitrage-system --bench emulation_bench
```

### Добавление нового режима

1. Добавьте вариант в `TradingMode` enum
2. Обновите логику в `TradingExecutor`
3. Обновите интервалы в `PeriodicActivitySimulator`
4. Добавьте конфигурацию в `config.toml`

## Лицензия

Proprietary - для внутреннего использования
