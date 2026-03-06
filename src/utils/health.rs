/// Health check система для мониторинга состояния компонентов
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::Serialize;

/// Статус здоровья компонента
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Информация о здоровье компонента
#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: u64,
    pub message: Option<String>,
}

/// Health checker для отдельного компонента
pub struct HealthChecker {
    name: String,
    is_healthy: Arc<AtomicBool>,
    last_heartbeat: Arc<AtomicU64>,
    timeout: Duration,
}

impl HealthChecker {
    pub fn new(name: String, timeout: Duration) -> Self {
        Self {
            name,
            is_healthy: Arc::new(AtomicBool::new(true)),
            last_heartbeat: Arc::new(AtomicU64::new(
                Instant::now().elapsed().as_secs()
            )),
            timeout,
        }
    }
    
    /// Отправляет heartbeat (компонент жив)
    pub fn heartbeat(&self) {
        let now = Instant::now().elapsed().as_secs();
        self.last_heartbeat.store(now, Ordering::Release);
        self.is_healthy.store(true, Ordering::Release);
    }
    
    /// Помечает компонент как нездоровый
    pub fn mark_unhealthy(&self) {
        self.is_healthy.store(false, Ordering::Release);
    }
    
    /// Проверяет здоровье компонента
    pub fn check(&self) -> ComponentHealth {
        let now = Instant::now().elapsed().as_secs();
        let last_heartbeat = self.last_heartbeat.load(Ordering::Acquire);
        let is_healthy = self.is_healthy.load(Ordering::Acquire);
        
        let elapsed = now.saturating_sub(last_heartbeat);
        
        let status = if !is_healthy {
            HealthStatus::Unhealthy
        } else if elapsed > self.timeout.as_secs() {
            HealthStatus::Unhealthy
        } else if elapsed > self.timeout.as_secs() / 2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        let message = match status {
            HealthStatus::Healthy => None,
            HealthStatus::Degraded => Some(format!("No heartbeat for {}s", elapsed)),
            HealthStatus::Unhealthy => Some(format!("Component unhealthy ({}s since last heartbeat)", elapsed)),
        };
        
        ComponentHealth {
            name: self.name.clone(),
            status,
            last_check: now,
            message,
        }
    }
    
    pub fn clone_checker(&self) -> Self {
        Self {
            name: self.name.clone(),
            is_healthy: self.is_healthy.clone(),
            last_heartbeat: self.last_heartbeat.clone(),
            timeout: self.timeout,
        }
    }
}

/// Агрегатор здоровья всей системы
pub struct SystemHealth {
    components: Vec<HealthChecker>,
}

impl SystemHealth {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }
    
    pub fn add_component(&mut self, checker: HealthChecker) {
        self.components.push(checker);
    }
    
    /// Проверяет здоровье всех компонентов
    pub fn check_all(&self) -> Vec<ComponentHealth> {
        self.components.iter().map(|c| c.check()).collect()
    }
    
    /// Возвращает общий статус системы
    pub fn overall_status(&self) -> HealthStatus {
        let healths = self.check_all();
        
        if healths.iter().any(|h| h.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if healths.iter().any(|h| h.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_health_checker() {
        let checker = HealthChecker::new("test".to_string(), Duration::from_secs(5));
        
        checker.heartbeat();
        let health = checker.check();
        assert_eq!(health.status, HealthStatus::Healthy);
        
        thread::sleep(Duration::from_millis(100));
        checker.mark_unhealthy();
        let health = checker.check();
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }
}
