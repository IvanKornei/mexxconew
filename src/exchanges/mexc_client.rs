use async_trait::async_trait;
use governor::{Quota, RateLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};
use hmac::{Hmac, Mac};
use nonzero_ext::*;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::exchanges::client::{
    Balance, ExchangeClient, Order, OrderRequest, OrderSide, OrderStatus, OrderType,
};
use crate::utils::ApiError;

#[allow(dead_code)]
pub type ClientResult<T> = std::result::Result<T, ApiError>;

#[allow(dead_code)]
type HmacSha256 = Hmac<Sha256>;

#[allow(dead_code)]
pub struct MexcClient {
    api_key: String,
    api_secret: String,
    http_client: Client,
    base_url: String,
    // MEXC: 20 requests/second
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl MexcClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        let quota = Quota::per_second(nonzero!(20u32));
        
        Self {
            api_key,
            api_secret,
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            base_url: "https://contract.mexc.com".to_string(),
            rate_limiter: RateLimiter::direct(quota),
        }
    }
    
    async fn check_rate_limit(&self) {
        self.rate_limiter.until_ready().await;
    }
    
    fn sign(&self, query: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
    
    fn timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[async_trait]
impl ExchangeClient for MexcClient {
    async fn place_order(&self, order: OrderRequest) -> ClientResult<Order> {
        self.check_rate_limit().await;
        
        let timestamp = Self::timestamp();
        let side = match order.side {
            OrderSide::Buy => 1,
            OrderSide::Sell => 2,
        };
        let order_type = match order.order_type {
            OrderType::Limit => 1,
            OrderType::Market => 2,
        };
        
        let mut params = vec![
            ("symbol", order.symbol.clone()),
            ("side", side.to_string()),
            ("type", order_type.to_string()),
            ("vol", order.quantity.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        if let Some(price) = order.price {
            params.push(("price", price.to_string()));
        }
        
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        
        let signature = self.sign(&query);
        
        debug!("Placing MEXC order: {}", order.symbol);
        
        let response = self
            .http_client
            .post(format!("{}/api/v1/private/order/submit", self.base_url))
            .header("ApiKey", &self.api_key)
            .header("Request-Time", timestamp.to_string())
            .header("Signature", signature)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "symbol": order.symbol,
                "side": side,
                "type": order_type,
                "vol": order.quantity.to_string(),
                "price": order.price.map(|p| p.to_string()),
            }))
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }
        
        let order_response: MexcOrderResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;
        
        if order_response.code != 0 {
            return Err(ApiError::ExchangeError(format!(
                "MEXC error: {}",
                order_response.msg.unwrap_or_default()
            )));
        }
        
        Ok(Order {
            id: order_response.data.unwrap_or_default(),
            symbol: order.symbol,
            status: OrderStatus::New,
            filled_quantity: Decimal::ZERO,
        })
    }
    
    async fn cancel_order(&self, order_id: &str) -> ClientResult<()> {
        self.check_rate_limit().await;
        
        let timestamp = Self::timestamp();
        let query = format!("orderId={}&timestamp={}", order_id, timestamp);
        let signature = self.sign(&query);
        
        debug!("Cancelling MEXC order: {}", order_id);
        
        let response = self
            .http_client
            .post(format!("{}/api/v1/private/order/cancel", self.base_url))
            .header("ApiKey", &self.api_key)
            .header("Request-Time", timestamp.to_string())
            .header("Signature", signature)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "orderId": order_id,
            }))
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }
        
        Ok(())
    }
    
    async fn get_balance(&self, asset: &str) -> ClientResult<Balance> {
        self.check_rate_limit().await;
        
        let timestamp = Self::timestamp();
        let query = format!("timestamp={}", timestamp);
        let signature = self.sign(&query);
        
        let response = self
            .http_client
            .get(format!("{}/api/v1/private/account/assets", self.base_url))
            .header("ApiKey", &self.api_key)
            .header("Request-Time", timestamp.to_string())
            .header("Signature", signature)
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }
        
        let balance_response: MexcBalanceResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;
        
        if balance_response.code != 0 {
            return Err(ApiError::ExchangeError(format!(
                "MEXC error: {}",
                balance_response.msg.unwrap_or_default()
            )));
        }
        
        let balances = balance_response.data.unwrap_or_default();
        let balance = balances
            .into_iter()
            .find(|b| b.currency == asset)
            .ok_or_else(|| ApiError::ExchangeError(format!("Asset {} not found", asset)))?;
        
        Ok(Balance {
            asset: balance.currency,
            free: balance.available_balance,
            locked: balance.frozen_balance,
        })
    }
}

#[derive(Debug, Deserialize)]
struct MexcOrderResponse {
    code: i32,
    msg: Option<String>,
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MexcBalanceResponse {
    code: i32,
    msg: Option<String>,
    data: Option<Vec<MexcBalance>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MexcBalance {
    currency: String,
    available_balance: Decimal,
    frozen_balance: Decimal,
}
