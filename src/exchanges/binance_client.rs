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
pub struct BinanceClient {
    api_key: String,
    api_secret: String,
    http_client: Client,
    base_url: String,
    // Binance Futures: 2400 requests/minute = 40 req/sec
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl BinanceClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        let quota = Quota::per_second(nonzero!(40u32));
        
        Self {
            api_key,
            api_secret,
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            base_url: "https://fapi.binance.com".to_string(),
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
impl ExchangeClient for BinanceClient {
    async fn place_order(&self, order: OrderRequest) -> ClientResult<Order> {
        self.check_rate_limit().await;
        
        let timestamp = Self::timestamp();
        let side = match order.side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        let order_type = match order.order_type {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
        };
        
        let mut query = format!(
            "symbol={}&side={}&type={}&quantity={}&timestamp={}",
            order.symbol, side, order_type, order.quantity, timestamp
        );
        
        if let Some(price) = order.price {
            query.push_str(&format!("&price={}&timeInForce=GTC", price));
        }
        
        let signature = self.sign(&query);
        query.push_str(&format!("&signature={}", signature));
        
        debug!("Placing Binance order: {}", order.symbol);
        
        let response = self
            .http_client
            .post(format!("{}/fapi/v1/order", self.base_url))
            .header("X-MBX-APIKEY", &self.api_key)
            .body(query)
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }
        
        let order_response: BinanceOrderResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;
        
        Ok(Order {
            id: order_response.order_id.to_string(),
            symbol: order_response.symbol,
            status: OrderStatus::New,
            filled_quantity: Decimal::ZERO,
        })
    }
    
    async fn cancel_order(&self, order_id: &str) -> ClientResult<()> {
        self.check_rate_limit().await;
        
        let timestamp = Self::timestamp();
        let query = format!("orderId={}&timestamp={}", order_id, timestamp);
        let signature = self.sign(&query);
        let query = format!("{}&signature={}", query, signature);
        
        debug!("Cancelling Binance order: {}", order_id);
        
        let response = self
            .http_client
            .delete(format!("{}/fapi/v1/order", self.base_url))
            .header("X-MBX-APIKEY", &self.api_key)
            .body(query)
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
        let query = format!("{}&signature={}", query, signature);
        
        let response = self
            .http_client
            .get(format!("{}/fapi/v2/balance?{}", self.base_url, query))
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }
        
        let balances: Vec<BinanceBalance> = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;
        
        let balance = balances
            .into_iter()
            .find(|b| b.asset == asset)
            .ok_or_else(|| ApiError::ExchangeError(format!("Asset {} not found", asset)))?;
        
        Ok(Balance {
            asset: balance.asset,
            free: balance.available_balance,
            locked: Decimal::ZERO,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceOrderResponse {
    order_id: u64,
    symbol: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceBalance {
    asset: String,
    available_balance: Decimal,
}
