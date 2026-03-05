use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use fast_float::parse;

use crate::core::CircuitBreaker;
use crate::utils::{ConnectionError, Result};

// Typed structures for zero-copy deserialization (HFT optimization)
#[derive(Debug, Deserialize)]
struct MexcTradeMessage {
    channel: String,
    data: Vec<MexcTrade>,
    #[serde(rename = "ts")]
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
struct MexcTrade {
    #[serde(rename = "p")]
    price: MexcPrice,
    #[serde(rename = "t")]
    trade_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MexcPrice {
    Float(f64),
    String(String),
}

#[derive(Debug, Deserialize)]
struct MexcResponse {
    msg: Option<String>,
}

pub struct MexcFuturesConnector {
    url: String,
    max_reconnect_attempts: u32,
    circuit_breaker: CircuitBreaker,
}

impl MexcFuturesConnector {
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
                warn!("MEXC circuit breaker is open, waiting...");
                sleep(Duration::from_secs(5)).await;
                continue;
            }
            
            match self.try_connect_and_stream(&mut callback).await {
                Ok(_) => {
                    info!("MEXC stream ended normally");
                    self.circuit_breaker.record_success();
                    break Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    error!("MEXC connection error (attempt {}): {}", attempt, e);
                    
                    self.circuit_breaker.record_failure();
                    
                    if attempt >= self.max_reconnect_attempts {
                        return Err(ConnectionError::ReconnectFailed { attempts: attempt }.into());
                    }
                    
                    // Увеличен максимальный backoff до 10 минут для защиты от reconnect storm
                    let backoff_secs = 2u64.pow(attempt.min(9)); // max 512 секунд
                    let backoff = Duration::from_secs(backoff_secs);
                    warn!("Reconnecting to MEXC in {:?}...", backoff);
                    sleep(backoff).await;
                }
            }
        }
    }
    
    async fn try_connect_and_stream<F>(&self, callback: &mut F) -> Result<()>
    where
        F: FnMut(f64, i64),
    {
        info!("Connecting to MEXC: {}", self.url);
        
        let (ws_stream, _) = connect_async(&self.url).await?;
        info!("Connected to MEXC");
        
        let (write, mut read) = ws_stream.split();
        
        // Pre-allocate messages to avoid repeated allocations (HFT optimization)
        let ping_msg = Message::Text(r#"{"method":"ping"}"#.to_string());
        let subscribe_msg = Message::Text(r#"{"method":"sub.deal","param":{"symbol":"BTC_USDT"}}"#.to_string());
        
        // Send subscription
        info!("Sending MEXC subscription");
        let mut write_guard = write;
        write_guard.send(subscribe_msg).await?;
        info!("Subscribed to MEXC Futures BTC_USDT trades");
        
        // Use Arc<Mutex> for safe shared write access between tasks
        let write_shared = std::sync::Arc::new(tokio::sync::Mutex::new(write_guard));
        let write_heartbeat = write_shared.clone();
        
        // Create bounded channel for pong responses (prevent unbounded task spawning)
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
        
        // Spawn heartbeat task with proper cleanup
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            // Skip missed ticks to avoid burst sends after blocking
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                interval.tick().await;
                // Try to acquire lock with timeout to avoid blocking forever
                // Reduced timeout for HFT: 10ms max latency for heartbeat
                match tokio::time::timeout(Duration::from_millis(10), write_heartbeat.lock()).await {
                    Ok(mut write) => {
                        if write.send(ping_msg.clone()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // Lock timeout - skip this ping to avoid blocking
                        // Exchange will handle missed pings gracefully
                        debug!("Heartbeat lock timeout, skipping ping");
                    }
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
                        // Reuse buffer to avoid allocations
                        parse_buffer.clear();
                        parse_buffer.extend_from_slice(text.as_bytes());
                        
                        // Try typed deserialization first (fast path)
                        if let Ok(trade_msg) = simd_json::from_slice::<MexcTradeMessage>(&mut parse_buffer) {
                            if trade_msg.channel == "push.deal" {
                                for trade in trade_msg.data {
                                    let price = match trade.price {
                                        MexcPrice::Float(p) => p,
                                        MexcPrice::String(ref s) => {
                                            match parse::<f64, _>(s) {
                                                Ok(p) => p,
                                                Err(_) => continue,
                                            }
                                        }
                                    };
                                    callback(price, trade.trade_time);
                                }
                            } else {
                                debug!("Unknown MEXC channel: {}", trade_msg.channel);
                            }
                        } else {
                            // Fallback for subscription confirmations - reuse buffer
                            if let Ok(response) = simd_json::from_slice::<MexcResponse>(&mut parse_buffer) {
                                if let Some(msg_type) = response.msg {
                                    if msg_type == "success" {
                                        info!("MEXC subscription confirmed");
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Binary(data)) => {
                        info!("📦 MEXC Binary: {} bytes", data.len());
                    }
                    Ok(Message::Ping(data)) => {
                        debug!("🏓 MEXC Ping");
                        // Send pong via bounded channel to prevent unbounded task spawning
                        // If channel is full, drop the pong (exchange will retry)
                        let _ = pong_tx.try_send(data);
                    }
                    Ok(Message::Pong(_)) => {
                        // Pong received - no action needed (hot path)
                        // Не логируем для минимизации latency
                    }
                    Ok(Message::Close(frame)) => {
                        warn!("MEXC WebSocket closed: {:?}", frame);
                        break;
                    }
                    Err(e) => {
                        error!("MEXC WebSocket error: {}", e);
                        return Err(e.into());
                    }
                    _ => {
                        info!("❓ MEXC unknown message type");
                    }
                }
            }
            
            Ok(())
        }.await;
        
        // Always cleanup heartbeat task regardless of result
        heartbeat_handle.abort();
        pong_handle.abort();
        result
    }
}
