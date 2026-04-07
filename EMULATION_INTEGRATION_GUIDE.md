# Руководство по интеграции системы эмуляции

## Быстрый старт

### 1. Настройка переменных окружения

Отредактируйте `.env` файл:

```bash
# MEXC Login Credentials
MEXC_USERNAME=your_email@example.com
MEXC_PASSWORD=your_password

# Encryption Key (сгенерируйте новый!)
# Windows: 
# $bytes = New-Object byte[] 32; (New-Object Security.Cryptography.RNGCryptoServiceProvider).GetBytes($bytes); [BitConverter]::ToString($bytes).Replace('-','').ToLower()
# Linux/Mac:
# openssl rand -hex 32
EMULATION_SESSION_KEY=ваш_сгенерированный_ключ_64_символа
```

### 2. Проверка конфигурации

Убедитесь, что `config.toml` содержит секцию `[emulation]`:

```toml
[emulation]
trading_mode = "Hybrid"  # Или "Aggressive"
browser_profile = "Chrome145Windows64"
session_storage_path = ".kiro/emulation/session.enc"
encryption_key_env = "EMULATION_SESSION_KEY"
```

### 3. Интеграция в код

#### Вариант A: Замена существующего MEXC коннектора (РЕКОМЕНДУЕТСЯ)

В `src/main.rs` или где создается MEXC коннектор:

```rust
// БЫЛО:
// let mexc_connector = MexcFuturesConnector::new(config.exchanges.mexc_ws.clone());

// СТАЛО:
use crate::exchanges::MEXCEmulator;

let mexc_connector = if let Some(emulation_config) = config.emulation {
    info!("🎭 Using MEXC Emulator");
    // Создаем эмулятор (автоматически инициализирует сессию)
    let emulator = MEXCEmulator::new(emulation_config)
        .await
        .expect("Failed to initialize MEXC emulator");
    
    // Используем как обычный коннектор
    Arc::new(emulator) as Arc<dyn ExchangeClient>
} else {
    info!("Using standard MEXC connector");
    Arc::new(MexcFuturesConnector::new(config.exchanges.mexc_ws.clone()))
};
```

#### Вариант B: Прямое использование

```rust
use crate::exchanges::MEXCEmulator;
use crate::emulation::config::EmulationConfig;

// Загрузить конфигурацию
let emulation_config = config.emulation
    .expect("Emulation config not found");

// Создать эмулятор
let mexc = MEXCEmulator::new(emulation_config).await?;

// Использовать для торговли
let order = OrderRequest {
    symbol: "BTC/USDT".to_string(),
    side: OrderSide::Buy,
    order_type: OrderType::Market,
    quantity: Decimal::from_f64_retain(0.001).unwrap(),
    price: None,
};

let response = mexc.place_order(order).await?;
```

### 4. Запуск

```bash
# Сгенерируйте ключ шифрования (если еще не сделали)
openssl rand -hex 32

# Добавьте в .env
# EMULATION_SESSION_KEY=полученный_ключ

# Запустите систему
cargo run --release
```

## Что происходит при запуске

1. **Инициализация сессии (15-30 сек)**
   - Запускается браузерная эмуляция Chrome 145
   - Выполняется логин на MEXC
   - Имитируется навигация к Futures Trading
   - Сохраняются cookies и токены

2. **Фоновая активность**
   - Каждые 10-20 минут (Hybrid) или 30-60 минут (Aggressive)
   - Проверка баланса, просмотр позиций, история
   - Случайный просмотр других торговых пар

3. **Торговля**
   - Hybrid: задержки 20-80мс перед каждым ордером
   - Aggressive: <1мс латентность
   - Автоматическое переключение при блокировке

## Мониторинг

### Dashboard
Откройте http://localhost:4200 и увидите:
- Текущий режим (Hybrid/Aggressive)
- Статус сессии
- Время до истечения сессии
- Количество фоновых действий
- Метрики латентности

### API
```bash
curl http://localhost:3001/api/emulation/status
```

### Логи
Смотрите в консоли:
```
INFO 🎭 Emulation mode: Hybrid
INFO 🎭 Starting session initialization with browser emulation
INFO ✅ Session initialized successfully in 18.5s
INFO 🎬 Periodic activity simulator started in background
```

## Режимы работы

### Hybrid Mode (по умолчанию, безопасный)
```toml
[emulation]
trading_mode = "Hybrid"
```

**Характеристики:**
- Задержки 20-80мс перед ордерами
- Фоновые действия каждые 10-20 минут
- Перерывы каждые 2-3 часа
- Проверка позиций после 15-25 сделок
- **Риск блокировки: НИЗКИЙ**

### Aggressive Mode (быстрый, рискованный)
```toml
[emulation]
trading_mode = "Aggressive"
```

**Характеристики:**
- Латентность <1мс
- Фоновые действия каждые 30-60 минут
- Без перерывов
- Автоматическое переключение на Hybrid при блокировке
- **Риск блокировки: СРЕДНИЙ**

## Troubleshooting

### Проблема: "Failed to create session initializer"
**Причина:** Отсутствует EMULATION_SESSION_KEY  
**Решение:** Добавьте ключ в .env файл

### Проблема: "Session initialization failed"
**Причина:** Неверные учетные данные MEXC  
**Решение:** Проверьте MEXC_USERNAME и MEXC_PASSWORD в .env

### Проблема: "Blocked by anti-bot system"
**Причина:** Слишком агрессивная торговля  
**Решение:** Система автоматически переключится на Hybrid mode

### Проблема: Высокая латентность в Hybrid mode
**Причина:** Это нормально - jitter добавляет задержки  
**Решение:** Переключитесь на Aggressive mode (рискованно)

## Отключение эмуляции

Если хотите вернуться к обычному режиму:

1. Закомментируйте секцию `[emulation]` в `config.toml`
2. Или используйте обычный коннектор в коде

## Безопасность

### ✅ Рекомендации
- Используйте Hybrid mode
- Работайте с домашнего IP
- Мониторьте алерты о блокировках
- Храните EMULATION_SESSION_KEY в секрете

### ❌ Не делайте
- Не используйте Aggressive mode постоянно
- Не запускайте несколько инстансов одновременно
- Не делитесь ключом шифрования
- Не игнорируйте алерты о блокировках

## Продвинутые настройки

### Изменение интервалов фоновой активности

```toml
[emulation.rate_limits]
background_actions_per_hour_hybrid = 6  # Каждые 10 минут
background_actions_per_hour_aggressive = 2  # Каждые 30 минут
```

### Настройка jitter

```toml
[emulation.jitter]
enabled_in_hybrid = true
min_delay_ms = 20
max_delay_ms = 80
mean_delay_ms = 50
std_dev_ms = 15
```

### Использование прокси

```toml
[emulation.proxy]
enabled = true
provider = "brightdata"
sticky_session = true
```

## Метрики производительности

### Ожидаемые значения

**Hybrid Mode:**
- P50 латентность: 40-60мс
- P95 латентность: 70-90мс
- P99 латентность: 80-100мс

**Aggressive Mode:**
- P50 латентность: <1мс
- P95 латентность: 1-2мс
- P99 латентность: 2-5мс

## Поддержка

При проблемах проверьте:
1. Логи в консоли
2. Dashboard на http://localhost:4200
3. API endpoint: http://localhost:3001/api/emulation/status
4. Файл сессии: .kiro/emulation/session.enc

Система автоматически восстанавливается после большинства ошибок!
