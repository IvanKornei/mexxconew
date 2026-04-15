use async_trait::async_trait;
use governor::{Quota, RateLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};
use hmac::{Hmac, Mac};
use nonzero_ext::*;
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use crate::exchanges::client::{
    Balance, ExchangeClient, Order, OrderRequest, OrderSide, OrderStatus, OrderType,
};
use crate::utils::ApiError;

#[allow(dead_code)]
pub type ClientResult<T> = std::result::Result<T, ApiError>;

#[allow(dead_code)]
type HmacSha256 = Hmac<Sha256>;

/// Коды направления ордера по MEXC Contract API v1.
///
/// Docs (`order/submit`): `side` — одно целое число:
///   1 = open long
///   2 = close short
///   3 = open short
///   4 = close long
///
/// Публичное API оставляем абстрактным (`OrderSide::Buy | Sell`) и
/// транслируем здесь с учётом явного флага `reduce_only` для закрытий.
fn mexc_side(side: OrderSide, reduce_only: bool) -> i32 {
    match (side, reduce_only) {
        (OrderSide::Buy, false) => 1,  // open long
        (OrderSide::Sell, true) => 4,  // close long (выход из long через sell)
        (OrderSide::Sell, false) => 3, // open short
        (OrderSide::Buy, true) => 2,   // close short (выход из short через buy)
    }
}

/// Коды типа ордера по MEXC Contract API v1.
///   1 = Limit, 2 = Post Only, 3 = IOC, 4 = FOK, 5 = Market, 6 = Convert
fn mexc_order_type(t: OrderType) -> i32 {
    match t {
        OrderType::Limit => 1,
        OrderType::Market => 5,
    }
}

/// Спецификация контракта MEXC Futures, полученная из
/// `GET /api/v1/contract/detail`. Нужна для корректной конвертации
/// quantity (в базовой валюте, например BTC) → `vol` (целые контракты),
/// который реально принимает биржа.
#[derive(Debug, Clone)]
pub struct ContractDetail {
    pub symbol: String,
    /// Размер одного контракта в базовой валюте (например, 0.0001 BTC).
    pub contract_size: Decimal,
    /// Минимальный размер ордера в контрактах.
    pub min_vol: u64,
    /// Максимальный размер ордера в контрактах (0 если не указано).
    pub max_vol: u64,
}

#[allow(dead_code)]
pub struct MexcClient {
    api_key: String,
    api_secret: String,
    http_client: Client,
    base_url: String,
    // MEXC: 20 requests/second
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    // Плечо по умолчанию при открытии. Можно переопределить позже при
    // необходимости. По умолчанию держим консервативное значение.
    default_leverage: u32,
    // openType: 1 = isolated margin, 2 = cross margin. Isolated безопаснее.
    open_type: u8,
    /// Кэш характеристик контрактов, подтягиваемых с биржи на старте.
    /// Используется для конвертации `quantity` в `vol` (кол-во контрактов).
    contracts: RwLock<HashMap<String, ContractDetail>>,
}

impl MexcClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self::with_leverage(api_key, api_secret, 20, 1)
    }

    pub fn with_leverage(
        api_key: String,
        api_secret: String,
        default_leverage: u32,
        open_type: u8,
    ) -> Self {
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
            default_leverage: default_leverage.max(1),
            open_type: if open_type == 2 { 2 } else { 1 },
            contracts: RwLock::new(HashMap::new()),
        }
    }

    /// Загружает характеристики контракта с MEXC и сохраняет их в кэш.
    /// Публичный endpoint — не требует подписи.
    pub async fn fetch_contract_detail(&self, symbol: &str) -> ClientResult<ContractDetail> {
        self.check_rate_limit().await;

        let url = format!("{}/api/v1/contract/detail?symbol={}", self.base_url, symbol);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::ExchangeError(error_text));
        }

        let parsed: MexcContractDetailResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if !parsed.success.unwrap_or(parsed.code == 0) {
            return Err(ApiError::ExchangeError(format!(
                "MEXC contract/detail [{}]: {}",
                parsed.code,
                parsed.msg.unwrap_or_default()
            )));
        }

        // Endpoint без параметра symbol возвращает массив; с параметром — обычно
        // один элемент, но MEXC иногда всё равно присылает массив, поэтому
        // разбираем оба варианта.
        let raw = parsed
            .data
            .into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| {
                ApiError::ExchangeError(format!("MEXC contract {} not found", symbol))
            })?;

        let contract_size = Decimal::from_f64_retain(raw.contract_size).ok_or_else(|| {
            ApiError::ParseError(format!(
                "Invalid contractSize {} for {}",
                raw.contract_size, symbol
            ))
        })?;

        if contract_size.is_zero() {
            return Err(ApiError::ParseError(format!(
                "contractSize is zero for {}",
                symbol
            )));
        }

        let detail = ContractDetail {
            symbol: raw.symbol,
            contract_size,
            min_vol: raw.min_vol.unwrap_or(1),
            max_vol: raw.max_vol.unwrap_or(0),
        };

        self.contracts
            .write()
            .expect("contracts RwLock poisoned")
            .insert(symbol.to_string(), detail.clone());

        info!(
            "📐 MEXC contract loaded | {} | contract_size={} | min_vol={} | max_vol={}",
            detail.symbol, detail.contract_size, detail.min_vol, detail.max_vol
        );

        Ok(detail)
    }

    /// Возвращает копию кэшированной характеристики контракта.
    pub fn get_cached_contract(&self, symbol: &str) -> Option<ContractDetail> {
        self.contracts
            .read()
            .expect("contracts RwLock poisoned")
            .get(symbol)
            .cloned()
    }

    /// Конвертирует объём в базовой валюте (например, BTC) в целое число
    /// контрактов MEXC. Округление — «обычное» (half-away-from-zero через
    /// `round_dp(0)`), затем validate `min_vol/max_vol`. Возвращает
    /// осмысленную ошибку, если размер меньше шага или больше лимита —
    /// вызывающему коду следует прервать попытку открытия, а не отправлять
    /// заведомо плохой ордер.
    pub fn quantity_to_contracts(
        &self,
        symbol: &str,
        quantity: Decimal,
    ) -> ClientResult<u64> {
        let detail = self.get_cached_contract(symbol).ok_or_else(|| {
            ApiError::ExchangeError(format!(
                "Contract spec for {} not loaded — call fetch_contract_detail at startup",
                symbol
            ))
        })?;

        if quantity <= Decimal::ZERO {
            return Err(ApiError::ExchangeError(format!(
                "Non-positive quantity for {}: {}",
                symbol, quantity
            )));
        }

        let contracts_decimal = (quantity / detail.contract_size).round();
        let contracts = contracts_decimal.to_u64().ok_or_else(|| {
            ApiError::ExchangeError(format!(
                "Quantity {} for {} does not fit into u64 contracts ({})",
                quantity, symbol, contracts_decimal
            ))
        })?;

        if contracts < detail.min_vol {
            return Err(ApiError::ExchangeError(format!(
                "Quantity {} {} → {} contracts is below MEXC min_vol {}",
                quantity, symbol, contracts, detail.min_vol
            )));
        }
        if detail.max_vol > 0 && contracts > detail.max_vol {
            return Err(ApiError::ExchangeError(format!(
                "Quantity {} {} → {} contracts exceeds MEXC max_vol {}",
                quantity, symbol, contracts, detail.max_vol
            )));
        }

        Ok(contracts)
    }

    /// Вставляет заранее известную спецификацию контракта в кэш. Используется
    /// в тестах и при холодной инициализации без сети.
    #[allow(dead_code)]
    pub fn insert_contract(&self, detail: ContractDetail) {
        self.contracts
            .write()
            .expect("contracts RwLock poisoned")
            .insert(detail.symbol.clone(), detail);
    }

    async fn check_rate_limit(&self) {
        self.rate_limiter.until_ready().await;
    }

    /// Подписывает payload через HMAC-SHA256 по правилам MEXC Contract API v1.
    ///
    /// Для POST-запросов payload = `api_key + timestamp + body_json_string`.
    /// Для GET-запросов payload = `api_key + timestamp + sorted_query_string`.
    /// Ключом HMAC выступает api_secret. Результат — lowercase hex.
    fn sign(&self, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Генерирует уникальный client-order-id для идемпотентности. MEXC
    /// отклоняет повторные `externalOid` в коротком окне, что защищает
    /// от случайного двойного сабмита при ретраях.
    fn generate_external_oid() -> String {
        let ts = Self::timestamp();
        let rand: u32 = rand::random();
        format!("hft-{ts}-{rand:08x}")
    }

    /// Экспорт sign для тестов.
    #[cfg(test)]
    pub(crate) fn sign_for_test(&self, payload: &str) -> String {
        self.sign(payload)
    }

    /// Закрывает позицию (reduce-only) на MEXC обратным ордером.
    ///
    /// `reduce_only=true` передаётся в `side`-mapping, так что MEXC
    /// обрабатывает как "close" и не открывает встречную позицию.
    pub async fn place_close_order(&self, order: OrderRequest) -> ClientResult<Order> {
        self.place_order_inner(order, true).await
    }

    async fn place_order_inner(
        &self,
        order: OrderRequest,
        reduce_only: bool,
    ) -> ClientResult<Order> {
        self.check_rate_limit().await;

        let timestamp = Self::timestamp();
        let side_code = mexc_side(order.side.clone(), reduce_only);
        let type_code = mexc_order_type(order.order_type.clone());
        let external_oid = Self::generate_external_oid();

        // На MEXC futures `vol` — количество контрактов (целое число).
        // Конвертируем базовый объём (BTC/ETH/SOL) по сохранённому
        // contract_size. Если деталь контракта не загружена (например,
        // забыли вызвать fetch_contract_detail на старте) — возвращаем
        // ошибку вместо того, чтобы слать заведомо неверный ордер.
        let vol_contracts = self.quantity_to_contracts(&order.symbol, order.quantity)?;

        let mut body = serde_json::json!({
            "symbol": order.symbol,
            "side": side_code,
            "type": type_code,
            "vol": vol_contracts,
            "leverage": self.default_leverage,
            "openType": self.open_type,
            "externalOid": external_oid,
        });
        if let Some(price) = order.price {
            body["price"] = serde_json::Value::String(price.to_string());
        }

        let body_str = serde_json::to_string(&body)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        // Signature = HMAC-SHA256(secret, api_key + timestamp + body)
        let payload = format!("{}{}{}", self.api_key, timestamp, body_str);
        let signature = self.sign(&payload);

        debug!(
            "Placing MEXC order: symbol={} side={} type={} vol={} reduce_only={} oid={}",
            order.symbol, side_code, type_code, vol_contracts, reduce_only, external_oid
        );

        let response = self
            .http_client
            .post(format!("{}/api/v1/private/order/submit", self.base_url))
            .header("ApiKey", &self.api_key)
            .header("Request-Time", timestamp.to_string())
            .header("Signature", signature)
            .header("Content-Type", "application/json")
            .body(body_str)
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

        if !order_response.success.unwrap_or(order_response.code == 0) {
            return Err(ApiError::ExchangeError(format!(
                "MEXC error [{}]: {}",
                order_response.code,
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
}

#[async_trait]
impl ExchangeClient for MexcClient {
    async fn place_order(&self, order: OrderRequest) -> ClientResult<Order> {
        self.place_order_inner(order, false).await
    }

    async fn cancel_order(&self, order_id: &str) -> ClientResult<()> {
        self.check_rate_limit().await;

        let timestamp = Self::timestamp();
        // MEXC cancel ожидает массив id в JSON body.
        let body = serde_json::json!([order_id]);
        let body_str = serde_json::to_string(&body)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        let payload = format!("{}{}{}", self.api_key, timestamp, body_str);
        let signature = self.sign(&payload);

        debug!("Cancelling MEXC order: {}", order_id);

        let response = self
            .http_client
            .post(format!("{}/api/v1/private/order/cancel", self.base_url))
            .header("ApiKey", &self.api_key)
            .header("Request-Time", timestamp.to_string())
            .header("Signature", signature)
            .header("Content-Type", "application/json")
            .body(body_str)
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
        // GET подписывается как api_key + timestamp + sorted_query_string.
        // Для этого эндпоинта query пустой.
        let payload = format!("{}{}", self.api_key, timestamp);
        let signature = self.sign(&payload);

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

        if !balance_response.success.unwrap_or(balance_response.code == 0) {
            return Err(ApiError::ExchangeError(format!(
                "MEXC error [{}]: {}",
                balance_response.code,
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
    success: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MexcBalanceResponse {
    code: i32,
    msg: Option<String>,
    data: Option<Vec<MexcBalance>>,
    success: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MexcBalance {
    currency: String,
    available_balance: Decimal,
    frozen_balance: Decimal,
}

/// Ответ `GET /api/v1/contract/detail`. MEXC возвращает объект с массивом
/// `data`, даже когда запрошен конкретный символ.
#[derive(Debug, Deserialize)]
struct MexcContractDetailResponse {
    code: i32,
    msg: Option<String>,
    #[serde(default)]
    data: Vec<MexcContractRaw>,
    success: Option<bool>,
}

/// Сырой элемент ответа `contract/detail`. Поля из официальной документации
/// MEXC; часть из них (priceScale/volScale) пока не используется, но
/// сохраняем на будущее.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct MexcContractRaw {
    symbol: String,
    /// MEXC отдаёт как число (например 0.0001). Парсим в f64 и затем в Decimal.
    contract_size: f64,
    min_vol: Option<u64>,
    max_vol: Option<u64>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mexc_side_mapping_open_vs_close() {
        // open long / open short
        assert_eq!(mexc_side(OrderSide::Buy, false), 1);
        assert_eq!(mexc_side(OrderSide::Sell, false), 3);
        // close long (sell) / close short (buy)
        assert_eq!(mexc_side(OrderSide::Sell, true), 4);
        assert_eq!(mexc_side(OrderSide::Buy, true), 2);
    }

    #[test]
    fn test_mexc_order_type_market_is_5() {
        // MEXC contract использует 5 для market (не 2).
        assert_eq!(mexc_order_type(OrderType::Market), 5);
        assert_eq!(mexc_order_type(OrderType::Limit), 1);
    }

    #[test]
    fn test_signature_is_deterministic_hex() {
        let c = MexcClient::new("K".into(), "S".into());
        let sig = c.sign_for_test("payload");
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(sig, c.sign_for_test("payload"));
    }

    #[test]
    fn test_external_oid_is_unique() {
        let a = MexcClient::generate_external_oid();
        let b = MexcClient::generate_external_oid();
        assert_ne!(a, b, "externalOid must be unique per call");
        assert!(a.starts_with("hft-"));
    }

    fn client_with_contract(symbol: &str, size: &str, min_vol: u64, max_vol: u64) -> MexcClient {
        let c = MexcClient::new("K".into(), "S".into());
        c.insert_contract(ContractDetail {
            symbol: symbol.into(),
            contract_size: Decimal::from_str_exact(size).unwrap(),
            min_vol,
            max_vol,
        });
        c
    }

    #[test]
    fn test_quantity_to_contracts_basic_conversion() {
        // 0.001 BTC при contract_size 0.0001 → 10 контрактов
        let c = client_with_contract("BTC_USDT", "0.0001", 1, 0);
        let qty = Decimal::from_str_exact("0.001").unwrap();
        assert_eq!(c.quantity_to_contracts("BTC_USDT", qty).unwrap(), 10);
    }

    #[test]
    fn test_quantity_to_contracts_rounds_half_away_from_zero() {
        // 0.00015 / 0.0001 = 1.5 → 2 (half-away-from-zero Decimal::round)
        let c = client_with_contract("BTC_USDT", "0.0001", 1, 0);
        let qty = Decimal::from_str_exact("0.00015").unwrap();
        assert_eq!(c.quantity_to_contracts("BTC_USDT", qty).unwrap(), 2);
    }

    #[test]
    fn test_quantity_below_min_vol_is_rejected() {
        // 0.00005 BTC / 0.0001 = 0.5 → 1 контракт, но min_vol = 5 → ошибка
        let c = client_with_contract("BTC_USDT", "0.0001", 5, 0);
        let qty = Decimal::from_str_exact("0.00005").unwrap();
        let err = c.quantity_to_contracts("BTC_USDT", qty).unwrap_err();
        assert!(
            format!("{}", err).contains("below MEXC min_vol"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_quantity_above_max_vol_is_rejected() {
        let c = client_with_contract("BTC_USDT", "0.0001", 1, 10);
        let qty = Decimal::from_str_exact("0.002").unwrap(); // → 20 контрактов
        let err = c.quantity_to_contracts("BTC_USDT", qty).unwrap_err();
        assert!(
            format!("{}", err).contains("exceeds MEXC max_vol"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_quantity_without_loaded_contract_errors_clearly() {
        let c = MexcClient::new("K".into(), "S".into());
        let qty = Decimal::from_str_exact("0.001").unwrap();
        let err = c.quantity_to_contracts("BTC_USDT", qty).unwrap_err();
        assert!(
            format!("{}", err).contains("Contract spec for BTC_USDT not loaded"),
            "unexpected error: {}",
            err
        );
    }
}
