use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use fast_float::parse;

use crate::core::CircuitBreaker;
use crate::utils::{ConnectionError, Result, HealthChecker};

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
    health_checker: Option<HealthChecker>,
}

impl BinanceFuturesConnector {
    pub fn new(url: String) -> Self {
        Self {
            url,
            max_reconnect_attempts: 5,
            circuit_breaker: CircuitBreaker::new(Default::default()),
            health_checker: None,
        }
    }
    
    /// Устанавливает health checker для мониторинга
    pub fn with_health_checker(mut self, checker: HealthChecker) -> Self {
        self.health_checker = Some(checker);
        self
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
        
        // Добавляем таймаут на подключение (10 секунд)
        let connect_timeout = Duration::from_secs(10);
        let (ws_stream, _) = tokio::time::timeout(
            connect_timeout,
            connect_async(&self.url)
        ).await
            .map_err(|_| ConnectionError::Timeout { 
                operation: "connect".to_string(),
                timeout_ms: connect_timeout.as_millis() as u64 
            })??;
        info!("Connected to Binance Futures");
        
        let (write, mut read) = ws_stream.split();
        
        // Shared write access for pong responses
        let write_shared = std::sync::Arc::new(tokio::sync::Mutex::new(write));
        
        // Create bounded channel for pong responses
        let (pong_tx, mut pong_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        let write_pong = write_shared.clone();
        
        // Spawn pong handler task
        let pong_handle = tokio::spawn(async move {
            while let Some(data) = pong_rx.recv().await {
                let mut write = write_pong.lock().await;
                if write.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
        });
        
        // Pre-allocate reusable buffer for JSON parsing (HFT optimization)
        let mut parse_buffer = Vec::with_capacity(4096);
        
        // Main message processing loop
        let result: Result<()> = async {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Валидация размера сообщения (защита от DoS)
                        if text.len() > 10_000 {
                            error!("Message too large: {} bytes", text.len());
                            continue;
                        }
                        
                        // Отправляем heartbeat при получении данных
                        if let Some(ref checker) = self.health_checker {
                            checker.heartbeat();
                        }
                        
                        // Reuse buffer to avoid allocations
                        parse_buffer.clear();
                        parse_buffer.extend_from_slice(text.as_bytes());
                        
                        // SIMD-accelerated JSON parsing for HFT
                        if let Ok(trade) = simd_json::from_slice::<BinanceAggTrade>(&mut parse_buffer) {
                            // Fast float parsing - 10x faster than std
                            if let Ok(price) = parse::<f64, _>(&trade.price) {
                                callback(price, trade.trade_time);
                            }
                        }
                    }
                    Ok(Message::Binary(data)) => {
                        // Binary messages are rare, log at debug level
                        tracing::debug!("Binance Binary: {} bytes", data.len());
                    }
                    Ok(Message::Ping(data)) => {
                        // Send pong via bounded channel to prevent unbounded task spawning
                        // If channel is full, drop the pong (exchange will retry)
                        let _ = pong_tx.try_send(data);
                    }
                    Ok(Message::Pong(_)) => {
                        // Pong received - no action needed (hot path)
                    }
                    Ok(Message::Close(frame)) => {
                        warn!("Binance WebSocket closed: {:?}", frame);
                        break;
                    }
                    Err(e) => {
                        error!("Binance WebSocket error: {}", e);
                        return Err(e.into());
                    }
                    _ => {
                        // Unknown message types are rare
                        tracing::debug!("Binance unknown message type");
                    }
                }
            }
            
            Ok(())
        }.await;
        
        // Always cleanup pong handler task
        pong_handle.abort();
        result
    }
}
