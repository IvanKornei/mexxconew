/// MEXC Cookie-based client - работает только с куками и REST API
/// 
/// Этот клиент не запускает браузер, а использует сохраненные куки для
/// прямого взаимодействия с MEXC REST API.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::emulation::{
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::{Cookie, SessionData, SessionPersistence},
};

/// Клиент для работы с MEXC через куки
pub struct MexcCookieClient {
    _config: EmulationConfig,
    session_persistence: SessionPersistence,
    current_session: Arc<RwLock<Option<SessionData>>>,
    http_client: reqwest::Client,
}

impl MexcCookieClient {
    /// Создать новый клиент
    pub fn new(config: EmulationConfig) -> Result<Self> {
        let session_persistence = SessionPersistence::from_config(&config)?;
        
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| EmulationError::HttpError(e.to_string()))?;
        
        Ok(Self {
            _config: config,
            session_persistence,
            current_session: Arc::new(RwLock::new(None)),
            http_client,
        })
    }
    
    /// Инициализировать клиент с существующей сессией
    pub async fn initialize(&mut self) -> Result<()> {
        info!("🔧 Инициализация MEXC Cookie клиента");
        
        // Попробовать загрузить существующую сессию
        if let Some(session) = self.session_persistence.load_session()? {
            if session.is_valid() {
                info!("✅ Загружена валидная сессия, создана: {}", session.created_at);
                *self.current_session.write().await = Some(session);
                return Ok(());
            } else {
                warn!("⚠️ Сессия истекла, требуется новая аутентификация");
            }
        }
        
        info!("🔐 Сессия не найдена, требуется аутентификация");
        Ok(())
    }
    
    /// Получить текущие куки для MEXC
    pub async fn get_mexc_cookies(&self) -> Result<Vec<Cookie>> {
        let session = self.current_session.read().await;
        
        match &*session {
            Some(session_data) => {
                // Фильтруем куки только для доменов MEXC
                let mexc_cookies: Vec<Cookie> = session_data.cookies
                    .iter()
                    .filter(|cookie| 
                        cookie.domain.contains("mexc.com") || 
                        cookie.domain.contains("futures.mexc.com")
                    )
                    .cloned()
                    .collect();
                
                if mexc_cookies.is_empty() {
                    warn!("⚠️ В сессии не найдены куки MEXC");
                }
                
                Ok(mexc_cookies)
            }
            None => Err(EmulationError::SessionError("Сессия не инициализирована".to_string())),
        }
    }
    
    /// Создать HTTP запрос с куками MEXC
    pub async fn create_request(&self, method: reqwest::Method, url: &str) -> Result<reqwest::RequestBuilder> {
        let cookies = self.get_mexc_cookies().await?;
        
        if cookies.is_empty() {
            return Err(EmulationError::SessionError("Нет куков MEXC для запроса".to_string()));
        }
        
        // Создаем строку куков для заголовка
        let cookie_header: String = cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<String>>()
            .join("; ");
        
        debug!("📤 Создание запроса к {} с {} куками", url, cookies.len());
        
        let request = self.http_client
            .request(method, url)
            .header("Cookie", cookie_header)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Accept", "application/json")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Content-Type", "application/json");
        
        Ok(request)
    }
    
    /// Проверить валидность сессии
    pub async fn check_session_validity(&self) -> Result<bool> {
        let session = self.current_session.read().await;
        
        match &*session {
            Some(session_data) => {
                if !session_data.is_valid() {
                    warn!("⚠️ Сессия истекла: {}", session_data.expires_at);
                    return Ok(false);
                }
                
                // Проверить, что есть куки MEXC
                let has_mexc_cookies = session_data.cookies
                    .iter()
                    .any(|cookie| 
                        cookie.domain.contains("mexc.com") || 
                        cookie.domain.contains("futures.mexc.com")
                    );
                
                if !has_mexc_cookies {
                    warn!("⚠️ В сессии нет куков MEXC");
                    return Ok(false);
                }
                
                Ok(true)
            }
            None => Ok(false),
        }
    }
    
    /// Обновить сессию с новыми куками
    pub async fn update_session_cookies(&mut self, cookies: Vec<Cookie>) -> Result<()> {
        info!("🔄 Обновление сессии с {} куками", cookies.len());
        
        let mut session = self.current_session.write().await;
        
        match &mut *session {
            Some(session_data) => {
                // Обновляем куки в существующей сессии
                session_data.cookies = cookies;
                session_data.expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
                
                // Сохраняем обновленную сессию
                self.session_persistence.save_session(session_data)?;
                info!("✅ Сессия обновлена, истекает: {}", session_data.expires_at);
            }
            None => {
                // Создаем новую сессию
                let new_session = SessionData::new(
                    cookies,
                    "cookie_auth".to_string(), // Токен аутентификации из куков
                    vec![], // TLS сессия не используется
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
                    24, // 24 часа
                );
                
                *session = Some(new_session.clone());
                self.session_persistence.save_session(&new_session)?;
                info!("✅ Новая сессия создана, истекает: {}", new_session.expires_at);
            }
        }
        
        Ok(())
    }
    
    /// Получить баланс через REST API
    pub async fn get_balance(&self) -> Result<f64> {
        info!("💰 Запрос баланса через REST API");
        
        let request = self.create_request(
            reqwest::Method::GET, 
            "https://futures.mexc.com/api/v1/private/account/balance"
        ).await?;
        
        let response = request.send().await
            .map_err(|e| EmulationError::HttpError(format!("Ошибка запроса баланса: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::HttpError(
                format!("Неуспешный статус запроса баланса: {}", response.status())
            ));
        }
        
        let response_text: String = response.text().await
            .map_err(|e| EmulationError::HttpError(format!("Ошибка чтения ответа: {}", e)))?;
        
        debug!("📥 Ответ баланса: {}", response_text);
        
        // Парсим JSON ответ
        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| EmulationError::HttpError(format!("Ошибка парсинга JSON: {}", e)))?;
        
        // MEXC API формат: {"code": 0, "data": {"availableBalance": 1234.56, ...}}
        let balance = json
            .get("data")
            .and_then(|data| data.get("availableBalance"))
            .or_else(|| json.get("data").and_then(|data| data.get("available")))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| EmulationError::HttpError("Баланс не найден в ответе".to_string()))?;
        
        info!("💰 Баланс получен: ${:.2}", balance);
        Ok(balance)
    }
    
    /// Разместить рыночный ордер
    pub async fn place_market_order(&self, side: &str, size: f64) -> Result<String> {
        info!("📊 Размещение рыночного ордера: {} {} BTC", side, size);
        
        let order_data = serde_json::json!({
            "symbol": "BTC_USDT",
            "side": side.to_uppercase(),
            "type": "MARKET",
            "quantity": size,
            "leverage": 20,
            "positionSide": "BOTH"
        });
        
        let request = self.create_request(
            reqwest::Method::POST,
            "https://futures.mexc.com/api/v1/private/order/submit"
        ).await?
        .json(&order_data);
        
        let response = request.send().await
            .map_err(|e| EmulationError::HttpError(format!("Ошибка размещения ордера: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::HttpError(
                format!("Неуспешный статус размещения ордера: {}", response.status())
            ));
        }
        
        let response_text: String = response.text().await
            .map_err(|e| EmulationError::HttpError(format!("Ошибка чтения ответа: {}", e)))?;
        
        debug!("📥 Ответ ордера: {}", response_text);
        
        // Парсим JSON ответ для получения ID ордера
        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| EmulationError::HttpError(format!("Ошибка парсинга JSON: {}", e)))?;
        
        // MEXC API формат: {"code": 0, "data": {"orderId": "123456789", ...}}
        let order_id = json
            .get("data")
            .and_then(|data| data.get("orderId"))
            .or_else(|| json.get("data").and_then(|data| data.get("order_id")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("order_{}", chrono::Utc::now().timestamp()));
        
        info!("✅ Ордер размещен: {}", order_id);
        Ok(order_id)
    }
    
    /// Закрыть клиент
    pub async fn close(&mut self) -> Result<()> {
        info!("🔒 Закрытие MEXC Cookie клиента");
        *self.current_session.write().await = None;
        Ok(())
    }
    
    /// Получить информацию о текущей сессии
    pub async fn get_session_info(&self) -> Result<SessionInfo> {
        let session = self.current_session.read().await;
        
        match &*session {
            Some(session_data) => {
                let mexc_cookies_count = session_data.cookies
                    .iter()
                    .filter(|cookie| 
                        cookie.domain.contains("mexc.com") || 
                        cookie.domain.contains("futures.mexc.com")
                    )
                    .count();
                
                Ok(SessionInfo {
                    is_valid: session_data.is_valid(),
                    created_at: session_data.created_at,
                    expires_at: session_data.expires_at,
                    total_cookies: session_data.cookies.len(),
                    mexc_cookies_count,
                    user_agent: session_data.user_agent.clone(),
                })
            }
            None => Err(EmulationError::SessionError("Сессия не инициализирована".to_string())),
        }
    }
}

/// Информация о сессии
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub is_valid: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub total_cookies: usize,
    pub mexc_cookies_count: usize,
    pub user_agent: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_test_config() -> EmulationConfig {
        let mut config = EmulationConfig::default();
        config.session_storage_path = std::env::temp_dir().join("test_session.enc");
        config.encryption_key_env = "TEST_ENCRYPTION_KEY".to_string();
        
        // Установить тестовый ключ шифрования
        std::env::set_var("TEST_ENCRYPTION_KEY", "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        
        config
    }
    
    #[tokio::test]
    async fn test_client_creation() {
        let config = create_test_config();
        let client = MexcCookieClient::new(config);
        assert!(client.is_ok());
    }
    
    #[tokio::test]
    async fn test_session_management() {
        let config = create_test_config();
        let mut client = MexcCookieClient::new(config).unwrap();
        
        // Инициализация без сессии
        let init_result = client.initialize().await;
        assert!(init_result.is_ok());
        
        // Проверка валидности сессии (должна быть false)
        let validity = client.check_session_validity().await.unwrap();
        assert!(!validity);
    }
}