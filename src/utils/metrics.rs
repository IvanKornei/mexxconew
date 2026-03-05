/// HFT метрики производительности
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

pub struct LatencyMetrics {
    // Счётчики
    total_messages: AtomicUsize,
    total_latency_us: AtomicU64,
    
    // Latency buckets (микросекунды)
    bucket_0_10us: AtomicUsize,    // < 10μs
    bucket_10_50us: AtomicUsize,   // 10-50μs
    bucket_50_100us: AtomicUsize,  // 50-100μs
    bucket_100_500us: AtomicUsize, // 100-500μs
    bucket_500_1ms: AtomicUsize,   // 500μs-1ms
    bucket_1ms_plus: AtomicUsize,  // > 1ms
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyMetrics {
    pub fn new() -> Self {
        Self {
            total_messages: AtomicUsize::new(0),
            total_latency_us: AtomicU64::new(0),
            bucket_0_10us: AtomicUsize::new(0),
            bucket_10_50us: AtomicUsize::new(0),
            bucket_50_100us: AtomicUsize::new(0),
            bucket_100_500us: AtomicUsize::new(0),
            bucket_500_1ms: AtomicUsize::new(0),
            bucket_1ms_plus: AtomicUsize::new(0),
        }
    }
    
    /// Записывает latency измерение
    pub fn record(&self, start: Instant) {
        let latency_us = start.elapsed().as_micros() as u64;
        
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us.fetch_add(latency_us, Ordering::Relaxed);
        
        // Распределяем по buckets
        if latency_us < 10 {
            self.bucket_0_10us.fetch_add(1, Ordering::Relaxed);
        } else if latency_us < 50 {
            self.bucket_10_50us.fetch_add(1, Ordering::Relaxed);
        } else if latency_us < 100 {
            self.bucket_50_100us.fetch_add(1, Ordering::Relaxed);
        } else if latency_us < 500 {
            self.bucket_100_500us.fetch_add(1, Ordering::Relaxed);
        } else if latency_us < 1000 {
            self.bucket_500_1ms.fetch_add(1, Ordering::Relaxed);
        } else {
            self.bucket_1ms_plus.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Возвращает статистику
    pub fn snapshot(&self) -> MetricsSnapshot {
        let total = self.total_messages.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);
        
        MetricsSnapshot {
            total_messages: total,
            avg_latency_us: if total > 0 { total_latency / total as u64 } else { 0 },
            bucket_0_10us: self.bucket_0_10us.load(Ordering::Relaxed),
            bucket_10_50us: self.bucket_10_50us.load(Ordering::Relaxed),
            bucket_50_100us: self.bucket_50_100us.load(Ordering::Relaxed),
            bucket_100_500us: self.bucket_100_500us.load(Ordering::Relaxed),
            bucket_500_1ms: self.bucket_500_1ms.load(Ordering::Relaxed),
            bucket_1ms_plus: self.bucket_1ms_plus.load(Ordering::Relaxed),
        }
    }
    
    /// Сбрасывает метрики
    pub fn reset(&self) {
        self.total_messages.store(0, Ordering::Relaxed);
        self.total_latency_us.store(0, Ordering::Relaxed);
        self.bucket_0_10us.store(0, Ordering::Relaxed);
        self.bucket_10_50us.store(0, Ordering::Relaxed);
        self.bucket_50_100us.store(0, Ordering::Relaxed);
        self.bucket_100_500us.store(0, Ordering::Relaxed);
        self.bucket_500_1ms.store(0, Ordering::Relaxed);
        self.bucket_1ms_plus.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_messages: usize,
    pub avg_latency_us: u64,
    pub bucket_0_10us: usize,
    pub bucket_10_50us: usize,
    pub bucket_50_100us: usize,
    pub bucket_100_500us: usize,
    pub bucket_500_1ms: usize,
    pub bucket_1ms_plus: usize,
}

impl MetricsSnapshot {
    pub fn print_summary(&self) {
        if self.total_messages == 0 {
            println!("📊 No messages processed yet");
            return;
        }
        
        println!("📊 Performance Metrics:");
        println!("  Total messages: {}", self.total_messages);
        println!("  Avg latency: {}μs", self.avg_latency_us);
        println!("  Distribution:");
        println!("    < 10μs:      {} ({:.1}%)", self.bucket_0_10us, self.percent(self.bucket_0_10us));
        println!("    10-50μs:     {} ({:.1}%)", self.bucket_10_50us, self.percent(self.bucket_10_50us));
        println!("    50-100μs:    {} ({:.1}%)", self.bucket_50_100us, self.percent(self.bucket_50_100us));
        println!("    100-500μs:   {} ({:.1}%)", self.bucket_100_500us, self.percent(self.bucket_100_500us));
        println!("    500μs-1ms:   {} ({:.1}%)", self.bucket_500_1ms, self.percent(self.bucket_500_1ms));
        println!("    > 1ms:       {} ({:.1}%)", self.bucket_1ms_plus, self.percent(self.bucket_1ms_plus));
    }
    
    fn percent(&self, count: usize) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            (count as f64 / self.total_messages as f64) * 100.0
        }
    }
}
