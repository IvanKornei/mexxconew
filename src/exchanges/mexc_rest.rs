/// MEXC Futures REST API client for HFT trading
/// 
/// This module provides low-latency order execution via REST API.
/// Target latency: <10ms for order placement.

use reqwest::{Client, header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT}};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use std::time::Duration;

use crate::emulation::persistence::Cookie;
use crate::utils::{Result, ConnectionError};

/// Order side
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType {
    Market,
    Limit,
}

/// Order request
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub quantity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<String>,
}

/// Order response
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub status: String,
    pub filled_qty: f64,
    pub avg_price: Option<f64>,
}

/// Account balance
#[derive(Debug, Clone, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub available: f64,
    pub frozen: f64,
}

/// Position information
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub unrealized_pnl: f64,
}

/// MEXC REST API client with cookie-based authentication
pub struct MexcRestClient {
    client: Client,
    cookies: Arc<RwLock<Vec<Cookie>>>,
    base_url: String,
}

impl MexcRestClient {
    /// Create new REST client
    pub fn new(cookies: Vec<Cookie>) -> Result<Self> {
        // Pre-configure HTTP client for minimal latency
        let client = Client::builder()
            .timeout(Duration::from_millis(100))  // 100ms timeout for HFT
            .pool_max_idle_per_host(10)           // Connection pooling
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_nodelay(true)                    // Disable Nagle's algorithm
            .http2_prior_knowledge()              // Use HTTP/2 for multiplexing
            .build()
            .map_err(|e| ConnectionError::Other(format!("Failed to build HTTP client: {}", e)))?;
        
        Ok(Self {
            client,
            cookies: Arc::new(RwLock::new(cookies)),
            base_url: "https://futures.mexc.com/api/v1".to_string(),
        })
    }
    
    /// Update cookies (called by session manager)
    pub async fn update_cookies(&self, cookies: Vec<Cookie>) {
        let mut guard = self.cookies.write().await;
        *guard = cookies;
        info!("✅ Updated {} cookies", guard.len());
    }
    
    /// Build headers with cookies
    async fn build_headers(&self) -> Result<HeaderMap> {
        let cookies = self.cookies.read().await;
        
        let cookie_header = cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_str(&cookie_header)
            .map_err(|e| ConnectionError::Other(format!("Invalid cookie header: {}", e)))?);
        headers.insert(USER_AGENT, HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"
        ));
        
        Ok(headers)
    }
    
    /// Place market order (optimized for HFT)
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: OrderSide,
        quantity: f64,
    ) -> Result<OrderResponse> {
        let order = OrderRequest {
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
            time_in_force: None,
        };
        
        self.place_order(order).await
    }
    
    /// Place limit order
    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: OrderSide,
        price: f64,
        quantity: f64,
    ) -> Result<OrderResponse> {
        let order = OrderRequest {
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            time_in_force: Some("GTC".to_string()),
        };
        
        self.place_order(order).await
    }
    
    /// Place order (generic)
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        let url = format!("{}/private/order/submit", self.base_url);
        let headers = self.build_headers().await?;
        
        debug!("📤 Placing order: {:?}", order);
        
        let response = self.client
            .post(&url)
            .headers(headers)
            .json(&order)
            .send()
            .await
            .map_err(|e| ConnectionError::Other(format!("Order request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ConnectionError::Other(
                format!("Order failed with status {}: {}", status, body)
            ).into());
        }
        
        let order_response: OrderResponse = response.json().await
            .map_err(|e| ConnectionError::Other(format!("Failed to parse order response: {}", e)))?;
        
        info!("✅ Order placed: {}", order_response.order_id);
        
        Ok(order_response)
    }
    
    /// Cancel order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/private/order/cancel", self.base_url);
        let headers = self.build_headers().await?;
        
        #[derive(Serialize)]
        struct CancelRequest {
            order_id: String,
        }
        
        let response = self.client
            .post(&url)
            .headers(headers)
            .json(&CancelRequest {
                order_id: order_id.to_string(),
            })
            .send()
            .await
            .map_err(|e| ConnectionError::Other(format!("Cancel request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ConnectionError::Other(
                format!("Cancel failed with status {}: {}", status, body)
            ).into());
        }
        
        info!("✅ Order cancelled: {}", order_id);
        
        Ok(())
    }
    
    /// Get account balance
    pub async fn get_balance(&self) -> Result<Vec<Balance>> {
        let url = format!("{}/private/account/assets", self.base_url);
        let headers = self.build_headers().await?;
        
        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ConnectionError::Other(format!("Balance request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ConnectionError::Other(
                format!("Balance request failed with status {}", response.status())
            ).into());
        }
        
        let balances: Vec<Balance> = response.json().await
            .map_err(|e| ConnectionError::Other(format!("Failed to parse balance: {}", e)))?;
        
        Ok(balances)
    }
    
    /// Get open positions
    pub async fn get_positions(&self) -> Result<Vec<Position>> {
        let url = format!("{}/private/position/list", self.base_url);
        let headers = self.build_headers().await?;
        
        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ConnectionError::Other(format!("Position request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ConnectionError::Other(
                format!("Position request failed with status {}", response.status())
            ).into());
        }
        
        let positions: Vec<Position> = response.json().await
            .map_err(|e| ConnectionError::Other(format!("Failed to parse positions: {}", e)))?;
        
        Ok(positions)
    }
    
    /// Test if session is valid
    pub async fn test_session(&self) -> Result<bool> {
        match self.get_balance().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_order_serialization() {
        let order = OrderRequest {
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 0.001,
            price: None,
            time_in_force: None,
        };
        
        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("BTC_USDT"));
        assert!(json.contains("BUY"));
        assert!(json.contains("MARKET"));
    }
}
