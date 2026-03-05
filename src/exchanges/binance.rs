use futures_util::StreamExt;
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use fast_float::parse;

use crate::core::CircuitBreaker;
use crate::utils::{ConnectionError, Result};

#[derive(Debug, Deserialize)]
struct BinanceAggTrade {
    #[serde(rename = "p")]
    price: String,
    #[serde(rename = "T")]
    trade_time: i64,
}

pub struct BinanceFuturesConnector {
    url: String,
    max_reconnect_attempts: u32,
    circuit_breaker: CircuitBreaker,
}

impl BinanceFuturesConnector {
    pub fn new(url: String) -> Self {
        Self {
            url,
            max_reconnect_attempts: 5,
            circuit_breaker: CircuitBreaker::new(Default::default()),
        }
    }
    
    pub async fn connect_and_stream<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(f64, i64) + Send,
    {
        let mut attempt = 0;
        
        loop {
            // Проверяем circuit breaker перед попыткой подключения
            if !self.circuit_breaker.can_proceed() {
                warn!("Binance circuit breaker is open, waiting...");
                sleep(Duration::from_secs(5)).await;
                continue;
            }
            
            match self.try_connect_and_stream(&mut callback).await {
                Ok(_) => {
                    info!("Binance stream ended normally");
                    self.circuit_breaker.record_success();
                    break Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    error!("Binance connection error (attempt {}): {}", attempt, e);
                    
                    self.circuit_breaker.record_failure();
                    
                    if attempt >= self.max_reconnect_attempts {
                        return Err(ConnectionError::ReconnectFailed { attempts: attempt }.into());
                    }
                    
                    // Увеличен максимальный backoff до 10 минут для защиты от reconnect storm
                    let backoff_secs = 2u64.pow(attempt.min(9)); // max 512 секунд
                    let backoff = Duration::from_secs(backoff_secs);
                    warn!("Reconnecting to Binance in {:?}...", backoff);
                    sleep(backoff).await;
                }
            }
        }
    }
    
    async fn try_connect_and_stream<F>(&self, callback: &mut F) -> Result<()>
    where
        F: FnMut(f64, i64),
    {
        info!("Connecting to Binance Futures: {}", self.url);
        
        let (ws_stream, _) = connect_async(&self.url).await?;
        info!("Connected to Binance Futures");
        
        let (mut _write, mut read) = ws_stream.split();
        
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // SIMD-accelerated JSON parsing for HFT
                    let mut bytes = text.into_bytes();
                    if let Ok(trade) = simd_json::from_slice::<BinanceAggTrade>(&mut bytes) {
                        // Fast float parsing - 10x faster than std
                        if let Ok(price) = parse::<f64, _>(&trade.price) {
                            callback(price, trade.trade_time);
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    info!("📦 Binance Binary: {} bytes", data.len());
                }
                Ok(Message::Ping(_)) => {
                    info!("🏓 Binance Ping");
                }
                Ok(Message::Pong(_)) => {
                    info!("🏓 Binance Pong");
                }
                Ok(Message::Close(_)) => {
                    warn!("Binance WebSocket closed");
                    break;
                }
                Err(e) => {
                    error!("Binance WebSocket error: {}", e);
                    return Err(e.into());
                }
                _ => {
                    info!("❓ Binance unknown message type");
                }
            }
        }
        
        Ok(())
    }
}
