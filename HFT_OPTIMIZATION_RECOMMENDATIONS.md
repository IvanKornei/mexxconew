# HFT System Optimization Recommendations

## Критические исправления (Выполнено)

### 1. Race Condition в PriceState ✅
**Проблема:** `system_timestamp` перезаписывался при каждом обновлении, создавая race condition.
**Решение:** Объединил расчеты в `recalculate_derived()`, убрал дублирование `current_timestamp_ms()`.

### 2. Memory Allocation в WebSocket ✅
**Проблема:** Аллокация String на каждое обновление (~1000+/сек).
**Решение:** Pre-allocated buffer с переиспользованием через `Vec::with_capacity(512)`.

### 3. Stale Checker Overhead ✅
**Проблема:** Проверка каждые 100ms создавала contention на `watch::channel`.
**Решение:** Снижена частота до 500ms + добавлен `MissedTickBehavior::Skip`.

### 4. Reconnect Storm Protection ✅
**Проблема:** Максимальный backoff 32 секунды — недостаточно.
**Решение:** Увеличен до 512 секунд (2^9).

### 5. Circuit Breaker ✅
**Проблема:** Отсутствовал механизм защиты от каскадных сбоев.
**Решение:** Реализован lock-free circuit breaker с атомиками.

---

## Дополнительные рекомендации (TODO)

### 1. Lock-Free State Management (Высокий приоритет)

**Текущая проблема:**
```rust
state_tx.send_modify(|state| {
    state.update_binance(price, timestamp);
});
```
`watch::channel` использует внутренний мьютекс, что создает contention при >1000 updates/sec.

**Решение:**
Использовать `arc-swap` для полностью lock-free обновлений:

```rust
use arc_swap::ArcSwap;

pub struct PriceFeedManager {
    state: Arc<ArcSwap<PriceState>>,
}

// Update без блокировок
self.state.rcu(|old| {
    let mut new = (**old).clone();
    new.update_binance(price, timestamp);
    new
});
```

**Ожидаемый эффект:** Снижение latency на 30-50% в hot path.

---

### 2. SIMD JSON Serialization (Средний приоритет)

**Текущая проблема:**
```rust
serde_json::to_writer(&mut buffer, &state)  // Не использует SIMD
```

**Решение:**
```rust
simd_json::to_writer(&mut buffer, &state)
```

**Ожидаемый эффект:** 2-3x ускорение сериализации.

---

### 3. Zero-Copy WebSocket Messages (Средний приоритет)

**Текущая проблема:**
```rust
String::from_utf8(buffer.clone())  // Копирование данных
```

**Решение:**
```rust
// Использовать Bytes для zero-copy
use bytes::Bytes;
let bytes = Bytes::from(buffer);
sender.send(Message::Binary(bytes)).await
```

---

### 4. CPU Pinning для критичных задач (Высокий приоритет)

**Проблема:** Tokio scheduler может мигрировать задачи между ядрами, вызывая cache misses.

**Решение:**
```rust
use tokio::runtime::Builder;

let runtime = Builder::new_multi_thread()
    .worker_threads(4)
    .thread_name("hft-worker")
    .on_thread_start(|| {
        // Pin thread to specific CPU core
        #[cfg(target_os = "linux")]
        {
            use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
            // Implementation here
        }
    })
    .build()?;
```

---

### 5. Latency Monitoring с Percentiles (Средний приоритет)

**Добавить:**
```rust
use hdrhistogram::Histogram;

pub struct LatencyMonitor {
    histogram: Histogram<u64>,
}

impl LatencyMonitor {
    pub fn record(&mut self, latency_us: u64) {
        self.histogram.record(latency_us).ok();
    }
    
    pub fn report(&self) {
        println!("p50: {}μs", self.histogram.value_at_quantile(0.5));
        println!("p99: {}μs", self.histogram.value_at_quantile(0.99));
        println!("p99.9: {}μs", self.histogram.value_at_quantile(0.999));
    }
}
```

---

### 6. Rate Limiting для API (Критично для production)

**Проблема:** Отсутствует защита от превышения rate limits бирж.

**Решение:**
```rust
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;

pub struct ExchangeConnector {
    rate_limiter: RateLimiter<
        governor::state::direct::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
}

impl ExchangeConnector {
    pub fn new() -> Self {
        // Binance: 1200 requests/minute
        let quota = Quota::per_minute(nonzero!(1200u32));
        Self {
            rate_limiter: RateLimiter::direct(quota),
        }
    }
    
    pub async fn make_request(&self) -> Result<()> {
        self.rate_limiter.until_ready().await;
        // Make request
        Ok(())
    }
}
```

---

### 7. Graceful Shutdown (Выполнено частично)

**Текущее состояние:** Базовый Ctrl+C handler есть.

**Улучшение:**
```rust
use tokio::sync::broadcast;

pub struct ShutdownCoordinator {
    tx: broadcast::Sender<()>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self { tx }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
    
    pub fn shutdown(&self) {
        self.tx.send(()).ok();
    }
}

// В каждой задаче:
tokio::select! {
    _ = shutdown_rx.recv() => {
        info!("Graceful shutdown initiated");
        // Cleanup
    }
    _ = work() => {}
}
```

---

### 8. Order Execution Module (Для будущей торговли)

**Минимальная реализация:**
```rust
pub struct OrderExecutor {
    binance_client: BinanceClient,
    mexc_client: MexcClient,
    risk_manager: RiskManager,
}

pub struct RiskManager {
    max_position_size: f64,
    max_daily_loss: f64,
    current_position: AtomicU64,  // В сатоши для lock-free
    daily_pnl: AtomicI64,
}

impl RiskManager {
    pub fn can_trade(&self, size: f64) -> bool {
        let current = self.current_position.load(Ordering::Acquire);
        let new_position = current + (size * 1e8) as u64;
        
        new_position <= (self.max_position_size * 1e8) as u64
    }
    
    pub fn check_daily_loss(&self) -> bool {
        let pnl = self.daily_pnl.load(Ordering::Acquire);
        pnl > -(self.max_daily_loss * 1e8) as i64
    }
}
```

---

### 9. Benchmarking Infrastructure

**Добавить в `benches/`:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_price_update(c: &mut Criterion) {
    let mut state = PriceState::default();
    
    c.bench_function("price_update", |b| {
        b.iter(|| {
            state.update_binance(black_box(50000.0), black_box(1234567890));
        });
    });
}

criterion_group!(benches, bench_price_update);
criterion_main!(benches);
```

**Запуск:**
```bash
cargo bench
```

---

### 10. Профилирование Production

**Инструменты:**
```bash
# CPU profiling
cargo install flamegraph
cargo flamegraph --bin arbitrage-system

# Memory profiling
cargo install heaptrack
heaptrack ./target/release/arbitrage-system

# Latency tracing
cargo install tokio-console
# Добавить в Cargo.toml:
# tokio = { features = ["tracing"] }
```

---

## Performance Targets

### Текущие метрики (оценка):
- Latency обработки: ~500μs - 1ms
- Throughput: ~500 updates/sec
- Memory allocations: ~1000/sec

### Целевые метрики после оптимизаций:
- Latency обработки: <100μs (p99)
- Throughput: >5000 updates/sec
- Memory allocations: <100/sec (в hot path)

---

## Приоритизация

### Немедленно (перед production):
1. ✅ Race condition fix
2. ✅ Circuit breaker
3. ✅ Memory allocation optimization
4. Rate limiting для API
5. Lock-free state management

### Краткосрочно (1-2 недели):
1. CPU pinning
2. SIMD JSON
3. Latency monitoring
4. Benchmarking suite

### Среднесрочно (1-2 месяца):
1. Order execution module
2. Risk management
3. Production profiling
4. Zero-copy optimizations

---

## Мониторинг в Production

**Ключевые метрики:**
- Latency (p50, p99, p99.9)
- Throughput (updates/sec)
- Circuit breaker state
- WebSocket reconnections
- Memory usage
- CPU usage per core
- GC pauses (если используется)

**Алерты:**
- Latency p99 > 1ms
- Circuit breaker открыт > 5 минут
- Reconnections > 3 за час
- Memory growth > 10% за час
