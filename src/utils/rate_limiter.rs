/// Rate limiter для защиты от перегрузки
use std::sync::Arc;
use std::time::Duration;
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter as GovernorRateLimiter, state::{InMemoryState, NotKeyed}, clock::DefaultClock};

/// Rate limiter для WebSocket соединений
pub struct RateLimiter {
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl RateLimiter {
    /// Создаёт новый rate limiter
    /// max_requests - максимальное количество запросов
    /// per_duration - за какой период времени
    pub fn new(max_requests: u32, per_duration: Duration) -> Self {
        let burst = NonZeroU32::new(max_requests.max(1)).unwrap();
        let quota = Quota::with_period(per_duration)
            .unwrap()
            .allow_burst(burst);
        
        let limiter = Arc::new(GovernorRateLimiter::direct(quota));
        
        Self { limiter }
    }
    
    /// Проверяет, можно ли выполнить запрос
    /// Возвращает true если запрос разрешён, false если превышен лимит
    pub fn check(&self) -> bool {
        self.limiter.check().is_ok()
    }
    
    /// Ожидает пока не станет доступен слот для запроса
    pub async fn wait(&self) {
        self.limiter.until_ready().await;
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            limiter: self.limiter.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_secs(1));
        
        // Первые 2 запроса должны пройти
        assert!(limiter.check());
        assert!(limiter.check());
        
        // Третий запрос должен быть заблокирован
        assert!(!limiter.check());
    }
}
