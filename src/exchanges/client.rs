use async_trait::async_trait;
use rust_decimal::Decimal;
use crate::utils::ApiError;

pub type ClientResult<T> = std::result::Result<T, ApiError>;

/// Trading client trait for exchange interaction
#[async_trait]
pub trait ExchangeClient: Send + Sync {
    async fn place_order(&self, order: OrderRequest) -> ClientResult<Order>;
    async fn cancel_order(&self, order_id: &str) -> ClientResult<()>;
    async fn get_balance(&self, asset: &str) -> ClientResult<Balance>;
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,  // Fixed-point for precision
    pub price: Option<Decimal>,  // Fixed-point for precision
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,  // Fixed-point for precision
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub asset: String,
    pub free: Decimal,  // Fixed-point for precision
    pub locked: Decimal,  // Fixed-point for precision
}

#[derive(Debug, Clone)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
}
