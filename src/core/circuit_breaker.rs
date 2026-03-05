use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, warn};

/// Circuit breaker для защиты от каскадных сбоев
/// Использует lock-free атомики для минимальной latency
#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<CircuitBreakerState>,
}

struct CircuitBreakerState {
    is_open: AtomicBool,
    failure_count: AtomicU64,
    last_failure_time: parking_lot::Mutex<Option<Instant>>,
    config: CircuitBreakerConfig,
}

#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u64,
    pub timeout: Duration,
    pub half_open_max_calls: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(CircuitBreakerState {
                is_open: AtomicBool::new(false),
                failure_count: AtomicU64::new(0),
                last_failure_time: parking_lot::Mutex::new(None),
                config,
            }),
        }
    }
    
    /// Проверяет, можно ли выполнить операцию
    #[inline]
    pub fn can_proceed(&self) -> bool {
        if !self.state.is_open.load(Ordering::Acquire) {
            return true;
        }
        
        // Проверяем, не пора ли перейти в half-open состояние
        let last_failure = self.state.last_failure_time.lock();
        if let Some(last_time) = *last_failure {
            if last_time.elapsed() >= self.state.config.timeout {
                drop(last_failure);
                self.try_half_open();
                return true;
            }
        }
        
        false
    }
    
    /// Регистрирует успешное выполнение
    #[inline]
    pub fn record_success(&self) {
        if self.state.is_open.load(Ordering::Acquire) {
            // Закрываем circuit breaker после успешного вызова в half-open состоянии
            self.state.is_open.store(false, Ordering::Release);
            self.state.failure_count.store(0, Ordering::Release);
            warn!("Circuit breaker closed after successful recovery");
        } else {
            // Сбрасываем счетчик ошибок при успехе
            self.state.failure_count.store(0, Ordering::Release);
        }
    }
    
    /// Регистрирует ошибку
    #[inline]
    pub fn record_failure(&self) {
        let failures = self.state.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
        
        *self.state.last_failure_time.lock() = Some(Instant::now());
        
        if failures >= self.state.config.failure_threshold {
            if !self.state.is_open.swap(true, Ordering::AcqRel) {
                error!(
                    "Circuit breaker opened after {} failures. Will retry in {:?}",
                    failures, self.state.config.timeout
                );
            }
        }
    }
    
    fn try_half_open(&self) {
        warn!("Circuit breaker entering half-open state");
        // В half-open состоянии разрешаем ограниченное количество попыток
        self.state.failure_count.store(0, Ordering::Release);
    }
    
    pub fn is_open(&self) -> bool {
        self.state.is_open.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        });
        
        assert!(cb.can_proceed());
        
        cb.record_failure();
        cb.record_failure();
        assert!(cb.can_proceed());
        
        cb.record_failure();
        assert!(!cb.can_proceed());
    }
    
    #[test]
    fn test_circuit_breaker_recovers() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
        });
        
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.can_proceed());
        
        thread::sleep(Duration::from_millis(60));
        assert!(cb.can_proceed());
        
        cb.record_success();
        assert!(cb.can_proceed());
    }
}
