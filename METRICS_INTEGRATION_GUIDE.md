# Руководство по интеграции метрик производительности

## Быстрый старт

### 1. Добавить метрики в PositionManager

```rust
// src/core/position_manager.rs

use crate::utils::LatencyMetrics;
use std::time::Instant;

pub struct PositionManager {
    // ... существующие поля ...
    
    // Метрики производительности
    market_state_metrics: Arc<LatencyMetrics>,
}

impl PositionManager {
    pub fn new(position_size: f64, max_positions: usize, initial_balance: f64) -> Self {
        // ...
        Self {
            // ...
            market_state_metrics: Arc::new(LatencyMetrics::new()),
        }
    }
    
    pub async fn process_market_state(&self, state: &PriceState) {
        let start = Instant::now();
        
        // ... существующая логика ...
        
        // Записываем метрику в конце
        self.market_state_metrics.record(start);
    }
    
    // Метод для получения метрик
    pub fn get_performance_metrics(&self) -> crate::utils::MetricsSnapshot {
        self.market_state_metrics.snapshot()
    }
}
```

### 2. Добавить периодический вывод метрик

```rust
// src/core/price_feed.rs

// В методе start() добавить новый task:
let position_manager_metrics = position_manager.clone();
let metrics_handle = tokio::spawn(async move {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    loop {
        interval.tick().await;
        
        let snapshot = position_manager_metrics.get_performance_metrics();
        snapshot.print_summary();
        
        // Опционально: сбросить метрики для следующего интервала
        // position_manager_metrics.market_state_metrics.reset();
    }
});

// В tokio::select! добавить:
_ = metrics_handle => info!("Metrics task ended"),
```

### 3. Добавить метрики в WebSocket API

```rust
// src/api/websocket.rs

// Добавить endpoint для получения метрик
pub async fn handle_get_metrics(
    position_manager: Arc<PositionManager>,
) -> impl IntoResponse {
    let snapshot = position_manager.get_performance_metrics();
    
    Json(json!({
        "total_messages": snapshot.total_messages,
        "avg_latency_us": snapshot.avg_latency_us,
        "distribution": {
            "0_10us": snapshot.bucket_0_10us,
            "10_50us": snapshot.bucket_10_50us,
            "50_100us": snapshot.bucket_50_100us,
            "100_500us": snapshot.bucket_100_500us,
            "500_1ms": snapshot.bucket_500_1ms,
            "1ms_plus": snapshot.bucket_1ms_plus,
        }
    }))
}
```

## Интерпретация метрик

### Хорошие показатели (HFT-ready):
```
📊 Performance Metrics:
  Total messages: 10000
  Avg latency: 15μs
  Distribution:
    < 10μs:      6500 (65.0%)  ✅ Отлично
    10-50μs:     2800 (28.0%)  ✅ Хорошо
    50-100μs:     500 (5.0%)   ✅ Приемлемо
    100-500μs:    150 (1.5%)   ⚠️  Редко
    500μs-1ms:     40 (0.4%)   ⚠️  Очень редко
    > 1ms:         10 (0.1%)   ❌ Требует внимания
```

### Проблемные показатели:
```
📊 Performance Metrics:
  Total messages: 10000
  Avg latency: 250μs
  Distribution:
    < 10μs:      1000 (10.0%)  ❌ Мало
    10-50μs:     2000 (20.0%)  ⚠️  Мало
    50-100μs:    2000 (20.0%)  ⚠️  Много
    100-500μs:   3000 (30.0%)  ❌ Слишком много
    500μs-1ms:   1500 (15.0%)  ❌ Критично
    > 1ms:        500 (5.0%)   ❌ Неприемлемо
```

**Действия при проблемных показателях**:
1. Проверить CPU load (должен быть <50%)
2. Проверить lock contention (добавить метрики для locks)
3. Проверить memory allocations (использовать `heaptrack`)
4. Проверить network latency (ping к биржам)

## Дополнительные метрики

### Lock Contention Metrics

```rust
pub struct LockMetrics {
    wait_time_us: AtomicU64,
    acquisitions: AtomicUsize,
}

impl LockMetrics {
    pub async fn with_lock<F, R>(&self, lock: &RwLock<T>, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let start = Instant::now();
        let mut guard = lock.write().await;
        let wait_time = start.elapsed().as_micros() as u64;
        
        self.wait_time_us.fetch_add(wait_time, Ordering::Relaxed);
        self.acquisitions.fetch_add(1, Ordering::Relaxed);
        
        f(&mut guard)
    }
}
```

### Memory Allocation Tracking

```rust
// Использовать jemalloc с профилированием
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// Периодически выводить статистику
jemalloc_ctl::epoch::mib().unwrap().advance().unwrap();
let allocated = jemalloc_ctl::stats::allocated::mib().unwrap().read().unwrap();
let resident = jemalloc_ctl::stats::resident::mib().unwrap().read().unwrap();
```

## Мониторинг в production

### Prometheus metrics (опционально)

```rust
use prometheus::{Histogram, register_histogram};

lazy_static! {
    static ref LATENCY_HISTOGRAM: Histogram = register_histogram!(
        "market_state_processing_duration_seconds",
        "Time spent processing market state"
    ).unwrap();
}

// В hot path:
let timer = LATENCY_HISTOGRAM.start_timer();
// ... обработка ...
timer.observe_duration();
```

### Grafana dashboard

Рекомендуемые панели:
1. Latency percentiles (p50, p95, p99, p999)
2. Throughput (messages/sec)
3. Lock contention time
4. Memory usage
5. Position open/close rate
6. PnL over time

## Troubleshooting

### Высокая latency (>100μs p99)

**Возможные причины**:
1. Lock contention → добавить lock metrics
2. Memory allocations → использовать heaptrack
3. CPU throttling → проверить `cpupower frequency-info`
4. Network issues → проверить ping к биржам

### Jitter (нестабильная latency)

**Возможные причины**:
1. GC паузы → использовать mimalloc (уже используется)
2. Context switching → уменьшить количество threads
3. Interrupt handling → настроить IRQ affinity
4. Disk I/O → переместить SQLite на tmpfs

### Memory leaks

**Диагностика**:
```bash
# Использовать valgrind
valgrind --leak-check=full --show-leak-kinds=all ./target/release/arbitrage

# Или heaptrack
heaptrack ./target/release/arbitrage
heaptrack_gui heaptrack.arbitrage.*.gz
```

## Целевые метрики

### Для тестирования:
- p50: <50μs
- p99: <200μs
- p999: <1ms

### Для production:
- p50: <10μs
- p99: <100μs
- p999: <500μs
- Throughput: >10,000 msg/sec
