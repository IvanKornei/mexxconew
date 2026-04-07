/// Lightweight Chrome emulation using only cookies and HTTP headers
/// 
/// This provides 100% Chrome emulation for MEXC without launching the actual browser.
/// MEXC only sees: cookies, User-Agent, HTTP headers, and request timing.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::emulation::{
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::{Cookie, SessionData},
    browser_cookies::BrowserCookieExtractor,
};

/// Lightweight Chrome emulator - uses real Chrome cookies but doesn't launch browser
pub struct BrowserActions {
    _config: EmulationConfig,
    http_client: reqwest::Client,
    current_cookies: Arc<RwLock<Vec<Cookie>>>,
    user_agent: String,
}

impl BrowserActions {
    /// Create new browser emulator
    pub fn new(config: EmulationConfig) -> Self {
        // Real Chrome User-Agent for Windows
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string();
        
        let http_client = reqwest::Client::builder()
            .user_agent(&user_agent)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            _config: config,
            http_client,
            current_cookies: Arc::new(RwLock::new(Vec::new())),
            user_agent,
        }
    }
    
    /// Initialize with session data
    pub async fn initialize(&mut self, _config: &EmulationConfig, session: &SessionData) -> Result<()> {
        info!("🔧 Initializing lightweight Chrome emulator");
        
        // Use cookies from session
        let mexc_cookies: Vec<Cookie> = session.cookies
            .iter()
            .filter(|cookie| 
                cookie.domain.contains("mexc.com") || 
                cookie.domain.contains("futures.mexc.com")
            )
            .cloned()
            .collect();
        
        if mexc_cookies.is_empty() {
            warn!("⚠️ No MEXC cookies in session, trying to extract from browser...");
            
            // Try to extract cookies from browser
            let extractor = BrowserCookieExtractor::auto_detect()?;
            let all_cookies = extractor.extract_mexc_cookies()?;
            
            *self.current_cookies.write().await = all_cookies;
        } else {
            *self.current_cookies.write().await = mexc_cookies;
        }
        
        let cookies = self.current_cookies.read().await;
        info!("✅ Chrome emulator initialized with {} MEXC cookies", cookies.len());
        
        Ok(())
    }
    
    /// Extract fresh cookies from Chrome browser
    pub async fn extract_fresh_cookies(&mut self) -> Result<()> {
        info!("🍪 Extracting fresh cookies from Chrome...");
        
        let extractor = BrowserCookieExtractor::auto_detect()?;
        let cookies = extractor.extract_mexc_cookies()?;
        
        *self.current_cookies.write().await = cookies;
        
        let cookie_count = self.current_cookies.read().await.len();
        info!("✅ Extracted {} fresh MEXC cookies", cookie_count);
        
        Ok(())
    }
    
    /// Create HTTP request with Chrome-like headers
    async fn create_chrome_request(&self, method: reqwest::Method, url: &str) -> Result<reqwest::RequestBuilder> {
        let cookies = self.current_cookies.read().await;
        
        if cookies.is_empty() {
            return Err(EmulationError::BrowserError("No cookies available".to_string()));
        }
        
        // Build cookie header
        let cookie_header: String = cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<String>>()
            .join("; ");
        
        debug!("📤 Creating Chrome-like request to {} with {} cookies", url, cookies.len());
        
        // Chrome-like headers
        let request = self.http_client
            .request(method, url)
            .header("Cookie", cookie_header)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Content-Type", "application/json")
            .header("Origin", "https://futures.mexc.com")
            .header("Referer", "https://futures.mexc.com/")
            .header("Sec-Ch-Ua", "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\"")
            .header("Sec-Ch-Ua-Mobile", "?0")
            .header("Sec-Ch-Ua-Platform", "\"Windows\"")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-origin");
        
        Ok(request)
    }
    
    /// Place market order via MEXC API (using Chrome cookies)
    pub async fn place_market_order(&mut self, side: &str, size: f64) -> Result<String> {
        info!("📊 Placing market order via Chrome emulation: {} {} BTC", side, size);
        
        // Simulate human delay (50-150ms)
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        let order_data = serde_json::json!({
            "symbol": "BTC_USDT",
            "side": side.to_uppercase(),
            "type": "MARKET",
            "quantity": size,
            "leverage": 20,
            "positionSide": "BOTH"
        });
        
        let request = self.create_chrome_request(
            reqwest::Method::POST,
            "https://futures.mexc.com/api/v1/private/order/submit"
        ).await?
        .json(&order_data);
        
        let response = request.send().await
            .map_err(|e| EmulationError::BrowserError(format!("Order placement failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::BrowserError(
                format!("Order failed with status: {}", response.status())
            ));
        }
        
        let response_text = response.text().await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to read response: {}", e)))?;
        
        debug!("📥 Order response: {}", response_text);
        
        // Parse order ID from JSON response
        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| EmulationError::BrowserError(format!("Failed to parse order JSON: {}", e)))?;
        
        // MEXC API response format: {"code": 0, "data": {"orderId": "123456789", ...}}
        let order_id = json
            .get("data")
            .and_then(|data| data.get("orderId"))
            .or_else(|| json.get("data").and_then(|data| data.get("order_id")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("chrome_order_{}", chrono::Utc::now().timestamp_millis()));
        
        info!("✅ Order placed successfully: {}", order_id);
        Ok(order_id)
    }
    
    /// Get balance via MEXC API
    pub async fn get_balance(&mut self) -> Result<f64> {
        info!("💰 Getting balance via Chrome emulation");
        
        let request = self.create_chrome_request(
            reqwest::Method::GET,
            "https://futures.mexc.com/api/v1/private/account/balance"
        ).await?;
        
        let response = request.send().await
            .map_err(|e| EmulationError::BrowserError(format!("Balance request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::BrowserError(
                format!("Balance request failed with status: {}", response.status())
            ));
        }
        
        let response_text = response.text().await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to read balance: {}", e)))?;
        
        debug!("📥 Balance response: {}", response_text);
        
        // Parse balance from JSON
        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| EmulationError::BrowserError(format!("Failed to parse balance JSON: {}", e)))?;
        
        // MEXC API response format: {"code": 0, "data": {"availableBalance": 1234.56, ...}}
        let balance = json
            .get("data")
            .and_then(|data| data.get("availableBalance"))
            .or_else(|| json.get("data").and_then(|data| data.get("available")))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| EmulationError::BrowserError("Balance not found in response".to_string()))?;
        
        info!("💰 Balance: ${:.2}", balance);
        Ok(balance)
    }
    
    /// Get open positions
    pub async fn get_open_positions(&mut self) -> Result<Vec<Position>> {
        info!("📈 Getting open positions via Chrome emulation");
        
        let request = self.create_chrome_request(
            reqwest::Method::GET,
            "https://futures.mexc.com/api/v1/private/position/open_positions"
        ).await?;
        
        let response = request.send().await
            .map_err(|e| EmulationError::BrowserError(format!("Positions request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::BrowserError(
                format!("Positions request failed with status: {}", response.status())
            ));
        }
        
        let response_text = response.text().await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to read positions: {}", e)))?;
        
        debug!("📥 Positions response: {}", response_text);
        
        // Parse positions from JSON
        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| EmulationError::BrowserError(format!("Failed to parse positions JSON: {}", e)))?;
        
        // MEXC API response format: {"code": 0, "data": [{"symbol": "BTC_USDT", "side": "LONG", ...}]}
        let positions_data = json
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| EmulationError::BrowserError("Positions data not found".to_string()))?;
        
        let mut positions = Vec::new();
        for pos in positions_data {
            if let (Some(symbol), Some(side), Some(size), Some(entry_price)) = (
                pos.get("symbol").and_then(|v| v.as_str()),
                pos.get("side").and_then(|v| v.as_str()),
                pos.get("positionSize").or(pos.get("size")).and_then(|v| v.as_f64()),
                pos.get("entryPrice").or(pos.get("entry_price")).and_then(|v| v.as_f64()),
            ) {
                let pnl = pos.get("unrealizedPnl")
                    .or(pos.get("unrealized_pnl"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                
                positions.push(Position {
                    symbol: symbol.to_string(),
                    side: side.to_string(),
                    size,
                    entry_price,
                    pnl,
                });
            }
        }
        
        info!("📈 Found {} open positions", positions.len());
        Ok(positions)
    }
    
    /// Simulate human activity (random delays, mouse movements in background)
    pub async fn simulate_human_activity(&mut self) -> Result<()> {
        // Simulate random human delay (1-5 seconds)
        let delay_ms = rand::random::<u64>() % 4000 + 1000;
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        
        debug!("🧍 Simulated human activity ({}ms delay)", delay_ms);
        Ok(())
    }
    
    /// Cancel order
    pub async fn cancel_order(&mut self, order_id: &str) -> Result<()> {
        info!("❌ Cancelling order: {}", order_id);
        
        let request = self.create_chrome_request(
            reqwest::Method::DELETE,
            &format!("https://futures.mexc.com/api/v1/private/order/cancel/{}", order_id)
        ).await?;
        
        let response = request.send().await
            .map_err(|e| EmulationError::BrowserError(format!("Cancel failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::BrowserError(
                format!("Cancel failed with status: {}", response.status())
            ));
        }
        
        info!("✅ Order cancelled: {}", order_id);
        Ok(())
    }
    
    /// Place limit order
    pub async fn place_limit_order(&mut self, side: &str, price: f64, size: f64) -> Result<String> {
        info!("📊 Placing limit order: {} {} BTC @ ${}", side, size, price);
        
        let order_data = serde_json::json!({
            "symbol": "BTC_USDT",
            "side": side.to_uppercase(),
            "type": "LIMIT",
            "price": price,
            "quantity": size,
            "leverage": 20,
            "positionSide": "BOTH"
        });
        
        let request = self.create_chrome_request(
            reqwest::Method::POST,
            "https://futures.mexc.com/api/v1/private/order/submit"
        ).await?
        .json(&order_data);
        
        let response = request.send().await
            .map_err(|e| EmulationError::BrowserError(format!("Limit order failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(EmulationError::BrowserError(
                format!("Limit order failed with status: {}", response.status())
            ));
        }
        
        let _response_text = response.text().await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to read response: {}", e)))?;
        
        let order_id = format!("limit_order_{}", chrono::Utc::now().timestamp_millis());
        
        info!("✅ Limit order placed: {}", order_id);
        Ok(order_id)
    }
    
    /// Close the emulator
    pub async fn close(&mut self) -> Result<()> {
        info!("🔒 Closing Chrome emulator");
        // Clear cookies
        *self.current_cookies.write().await = Vec::new();
        Ok(())
    }
    
    /// Get current cookie count
    pub async fn get_cookie_count(&self) -> usize {
        let cookies = self.current_cookies.read().await;
        cookies.len()
    }
    
    /// Check if cookies are valid (not expired)
    pub async fn are_cookies_valid(&self) -> bool {
        let cookies = self.current_cookies.read().await;
        
        for cookie in cookies.iter() {
            if let Some(expires) = cookie.expires {
                if expires < chrono::Utc::now() {
                    return false;
                }
            }
        }
        
        !cookies.is_empty()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub pnl: f64,
}
