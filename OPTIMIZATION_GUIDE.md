# Руководство по оптимизации HFT системы

## 1. Сетевая оптимизация

### Колокация серверов
- **AWS Tokyo** для Binance (серверы в Токио)
- **AWS Singapore** для MEXC (серверы в Сингапуре)
- Задержка: 1-5ms вместо 50-200ms

### Множественные соединения
```rust
// Подключаемся к нескольким WebSocket endpoint'ам
let binance_ws1 = "wss://fstream.binance.com/ws/btcusdt@aggTrade";
let binance_ws2 = "wss://fstream-auth.binance.com/ws/btcusdt@aggTrade";
let binance_ws3 = "wss://fstream.binance.com/stream?streams=btcusdt@aggTrade";

// Используем самый быстрый
```

### TCP оптимизация
```bash
# Linux kernel tuning
sudo sysctl -w net.ipv4.tcp_fastopen=3
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
```

## 2. Оптимизация кода

### Используй lock-free структуры
```rust
use crossbeam::queue::ArrayQueue;
use arc_swap::ArcSwap;

// Вместо RwLock
let state = ArcSwap::from_pointee(PriceState::default());

// Обновление без блокировки
state.store(Arc::new(new_state));
```

### SIMD для вычислений
```rust
use std::simd::f64x4;

// Векторизованные вычисления
let prices = f64x4::from_array([price1, price2, price3, price4]);
let spreads = (prices - base_price) / base_price * 100.0;
```

### Zero-copy парсинг
```rust
// Используем simd-json (уже есть)
let mut bytes = text.into_bytes();
let trade = simd_json::from_slice::<Trade>(&mut bytes)?;
```

## 3. Реальное исполнение ордеров

### Быстрое размещение ордеров
```rust
// Параллельное размещение на обеих биржах
let (binance_order, mexc_order) = tokio::join!(
    binance_client.place_order(sell_order),
    mexc_client.place_order(buy_order)
);
```

### Post-only ордера (maker)
```rust
// Избегаем комиссий taker
OrderRequest {
    order_type: OrderType::Limit,
    time_in_force: TimeInForce::PostOnly, // Только maker
    price: Some(best_bid + 0.01), // Чуть лучше рынка
    ..
}
```

### Отмена и замена (amend)
```rust
// Быстрее чем cancel + new
client.amend_order(order_id, new_price).await?;
```

## 4. Управление рисками

### Максимальная просадка
```rust
struct RiskManager {
    max_drawdown_percent: f64,  // 5%
    daily_loss_limit: f64,      // $1000
    max_position_size: f64,     // 1 BTC
}

impl RiskManager {
    fn can_open_position(&self, current_loss: f64) -> bool {
        current_loss < self.daily_loss_limit
    }
}
```

### Circuit breaker
```rust
// Уже реализован в src/core/circuit_breaker.rs
if circuit_breaker.is_open() {
    // Остановить торговлю на 60 секунд
    return;
}
```

## 5. Мониторинг и алерты

### Prometheus метрики
```rust
use metrics::{counter, gauge, histogram};

counter!("trades_total").increment(1);
gauge!("open_positions").set(positions.len() as f64);
histogram!("order_latency_ms").record(latency);
```

### Telegram уведомления
```rust
async fn send_telegram_alert(message: &str) {
    let bot_token = env::var("TELEGRAM_BOT_TOKEN")?;
    let chat_id = env::var("TELEGRAM_CHAT_ID")?;
    
    reqwest::Client::new()
        .post(format!("https://api.telegram.org/bot{}/sendMessage", bot_token))
        .json(&json!({
            "chat_id": chat_id,
            "text": message
        }))
        .send()
        .await?;
}
```

## 6. Бэктестинг

### Исторические данные
```rust
struct Backtester {
    historical_data: Vec<PriceState>,
    strategy: ImpulseStrategy,
}

impl Backtester {
    async fn run(&self) -> BacktestResults {
        let mut positions = Vec::new();
        
        for state in &self.historical_data {
            // Симулируем торговлю
            if let Some(signal) = self.strategy.check_entry_signal(...) {
                positions.push(self.strategy.create_position(...));
            }
        }
        
        BacktestResults {
            total_trades: positions.len(),
            win_rate: calculate_win_rate(&positions),
            sharpe_ratio: calculate_sharpe(&positions),
            max_drawdown: calculate_max_drawdown(&positions),
        }
    }
}
```

## 7. Сравнение скорости

### WebSocket (текущий)
- Задержка: **10-50ms**
- Частота обновлений: **100-1000 msg/sec**
- Подходит для: HFT, скальпинг

### REST API
- Задержка: **100-500ms**
- Rate limit: **1200 req/min** (Binance)
- Подходит для: размещение ордеров, проверка баланса

### FIX Protocol (профессиональный)
- Задержка: **1-10ms**
- Требует: специальный доступ, дорого
- Используют: институциональные трейдеры

## 8. Реальные цифры

### Latency breakdown
```
Биржа → Интернет → Твой сервер → Обработка → Ордер → Биржа
10ms  + 20ms     + 5ms          + 1ms      + 50ms  + 10ms = 96ms
```

### Оптимизированная версия (колокация)
```
Биржа → Локальная сеть → Обработка → Ордер → Биржа
1ms   + 1ms            + 0.1ms     + 5ms   + 1ms = 8.1ms
```

## 9. Следующие шаги

1. **Тестирование на testnet** (Binance/MEXC testnet)
2. **Малые суммы** ($10-100) для проверки
3. **Мониторинг 24/7** перед увеличением капитала
4. **Постепенное масштабирование**

## 10. Важные предупреждения

⚠️ **Риски**:
- Slippage может съесть всю прибыль
- Задержка сети непредсказуема
- Биржи могут менять API
- Регуляторные риски

⚠️ **Не делай**:
- Не торгуй на заёмные средства сразу
- Не используй весь капитал
- Не игнорируй risk management
- Не запускай без тестирования

✅ **Делай**:
- Начни с малого
- Тестируй на testnet
- Логируй всё
- Мониторь метрики
- Имей план выхода
