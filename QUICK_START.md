# Быстрый старт - HFT Arbitrage System

## Предварительные требования

- Rust 1.84+ (установка: https://rustup.rs/)
- Node.js 18+ (для фронтенда)
- Git

## Установка и запуск

### 1. Клонирование репозитория (если ещё не сделано)

```bash
git clone <repository-url>
cd arbitrage-system
```

### 2. Настройка конфигурации

Файл `config.toml` уже настроен с оптимальными параметрами:

```toml
[trading]
initial_capital = 100.0        # Начальный капитал ($)
position_size_percent = 10.0   # Размер позиции (%)
leverage = 200.0               # Плечо
max_positions = 2              # Максимум открытых позиций

[api]
host = "0.0.0.0"              # Слушать на всех интерфейсах
port = 3001                    # Порт WebSocket сервера
```

### 3. Запуск бэкенда

```bash
# Сборка в release режиме (с оптимизациями)
cargo build --release

# Запуск
cargo run --release
```

Вы увидите:
```
🚀 Starting HFT Arbitrage System
✅ Configuration loaded successfully
✅ Price feed manager initialized
🌐 Starting WebSocket server on 0.0.0.0:3001
📊 Starting price feeds...
```

### 4. Запуск фронтенда (в отдельном терминале)

```bash
cd frontend
npm install
npm start
```

Откройте браузер: http://localhost:4200

---

## Проверка работоспособности

### 1. Health Check

```bash
# Простая проверка
curl http://localhost:3001/health
# Ответ: OK

# Детальная информация
curl http://localhost:3001/health/detailed
```

Пример ответа:
```json
{
  "status": "Healthy",
  "components": [
    {
      "name": "binance",
      "status": "Healthy",
      "last_check": 1234567890,
      "message": null
    },
    {
      "name": "mexc",
      "status": "Healthy",
      "last_check": 1234567890,
      "message": null
    }
  ]
}
```

### 2. Мониторинг логов

Система автоматически выводит метрики каждые 30 секунд:

```
📊 === Performance Metrics ===
  Market State Processing:
    Total: 15234 | Avg: 12μs
    <10μs: 65% | 10-50μs: 28% | 50-100μs: 5% | >100μs: 2%
  Position Updates:
    Total: 45 | Avg: 8μs
```

### 3. Проверка торговли

В логах вы увидите:
```
🚀 OPENED | pos_1234567890 | Long | Entry: 95234.50 | Size: 0.000210 BTC | Capital: $100.00 | AvgLag: 245ms
💰 CLOSED | pos_1234567890 | Long | Entry: 95234.50 | Exit: 95256.30 | PnL: $0.46 (0.023%) | TakeProfit | 1523ms
💵 Capital: $100.46 | PnL: $0.46 | Fee: $0.00 (0.00%) | Net: $0.46 | Volume: $420 | Return: 0.46%
```

---

## Настройка стратегии через UI

### Параметры стратегии:

1. **Exchange Balance** - Баланс на бирже ($)
2. **Position Size** - Процент капитала на сделку (1-100%)
3. **Leverage** - Плечо (1-200x)
4. **Max Positions** - Максимум открытых позиций (1-5)

### Режимы торговли:

- **Safe** (Консервативный):
  - Max Positions: 1
  - Momentum Threshold: 0.05%
  - Quick Exit: 1.5s
  - Веса: Momentum 40%, Lag 50%, Price 10%

- **Balanced** (Сбалансированный):
  - Max Positions: 2
  - Momentum Threshold: 0.03%
  - Quick Exit: 2s
  - Веса: Momentum 60%, Lag 30%, Price 10%

- **Aggressive** (Агрессивный):
  - Max Positions: 3
  - Momentum Threshold: 0.02%
  - Quick Exit: 3s
  - Веса: Momentum 80%, Lag 15%, Price 5%

### Применение настроек:

1. Измените параметры в UI
2. Нажмите кнопку **"Apply Settings"**
3. Настройки применятся немедленно без перезапуска

---

## Мониторинг производительности

### Метрики в реальном времени:

Система автоматически выводит метрики каждые 30 секунд:

```
📊 === Performance Metrics ===
  Market State Processing:
    Total: 15234 | Avg: 12μs
    <10μs: 65% | 10-50μs: 28% | 50-100μs: 5% | >100μs: 2%
```

### Целевые показатели:

- ✅ **p50 latency**: <10μs
- ✅ **p99 latency**: <100μs
- ✅ **p999 latency**: <1ms
- ✅ **Throughput**: >10,000 msg/sec

### Если метрики хуже целевых:

1. Проверьте CPU load: `top` или `htop`
2. Проверьте сетевую задержку: `ping api.binance.com`
3. Проверьте health status: `curl http://localhost:3001/health/detailed`

---

## Остановка системы

### Graceful shutdown:

```bash
# Нажмите Ctrl+C в терминале с бэкендом
^C
```

Вы увидите:
```
Received Ctrl+C, shutting down gracefully...
Waiting for active operations to complete...
Shutdown complete
```

Система корректно завершит все активные операции за 2 секунды.

---

## Troubleshooting

### Проблема: "Connection refused"

**Решение**: Проверьте интернет-соединение и доступность бирж:
```bash
ping api.binance.com
ping contract.mexc.com
```

### Проблема: "Circuit breaker opened"

**Причина**: Слишком много ошибок подключения к бирже.

**Решение**: 
1. Проверьте интернет
2. Подождите 60 секунд (circuit breaker автоматически закроется)
3. Проверьте health status

### Проблема: Высокая latency (>100μs)

**Причины**:
1. Высокая CPU нагрузка
2. Проблемы с сетью
3. Много открытых позиций

**Решение**:
1. Уменьшите `max_positions`
2. Проверьте CPU: `top`
3. Проверьте сеть: `ping api.binance.com`

### Проблема: "LIQUIDATED! Capital depleted"

**Причина**: Капитал исчерпан из-за убыточных сделок.

**Решение**:
1. Перезапустите систему
2. Уменьшите `leverage`
3. Уменьшите `position_size_percent`
4. Используйте режим "Safe"

---

## Продвинутые настройки

### Изменение уровня логирования:

```bash
# Детальное логирование
export RUST_LOG=debug
cargo run --release

# Только ошибки
export RUST_LOG=error
cargo run --release
```

### JSON логирование для мониторинга:

Измените в `src/main.rs`:
```rust
// Было:
utils::logging::init_logging(Level::INFO)?;

// Стало:
utils::logging::init_json_logging(Level::INFO)?;
```

### Изменение порта WebSocket:

В `config.toml`:
```toml
[api]
port = 8080  # Новый порт
```

---

## Безопасность

### ⚠️ Важно для production:

1. **Не используйте `host = "0.0.0.0"` в production**
   ```toml
   [api]
   host = "127.0.0.1"  # Только localhost
   ```

2. **Добавьте аутентификацию** для WebSocket API

3. **Используйте HTTPS/WSS** в production

4. **Ограничьте rate limiting** для WebSocket клиентов

5. **Мониторьте health checks** и настройте алерты

---

## Дополнительная информация

- **Архитектура**: См. `HFT_CRITICAL_ANALYSIS.md`
- **Метрики**: См. `METRICS_INTEGRATION_GUIDE.md`
- **Улучшения**: См. `IMPROVEMENTS_SUMMARY.md`
- **Конфигурация**: См. `config.toml`

---

## Поддержка

При возникновении проблем:

1. Проверьте логи системы
2. Проверьте health status: `curl http://localhost:3001/health/detailed`
3. Проверьте метрики производительности в логах
4. Создайте issue с описанием проблемы и логами

---

**Удачной торговли! 🚀📈**
