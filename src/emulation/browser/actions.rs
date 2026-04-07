/// ⚠️ DEPRECATED: Browser-based trading actions
/// 
/// This module is DEPRECATED for HFT arbitrage trading due to:
/// - High latency (2-5 seconds per order vs <10ms REST API)
/// - No atomicity guarantees
/// - Race conditions with concurrent UI interactions
/// - Memory allocations in hot paths
/// 
/// USE INSTEAD:
/// - `crate::exchanges::mexc_rest::MexcRestClient` for order execution
/// - `crate::emulation::browser::session_manager::SessionManager` for cookie management
/// 
/// This module is kept for:
/// - Manual testing and debugging
/// - UI automation for non-trading tasks
/// - Fallback when REST API is unavailable

use tracing::{debug, info, warn};
use std::time::Duration;

use crate::emulation::{
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::SessionData,
};

use super::BrowserAutomation;
// use super::ChromeBrowser;

/// ⚠️ DEPRECATED: High-level browser actions for trading
/// 
/// WARNING: This struct has 2-5 second latency per operation.
/// For HFT arbitrage, use `MexcRestClient` instead.
#[deprecated(
    since = "0.2.0",
    note = "Use MexcRestClient for trading, SessionManager for cookies"
)]
pub struct BrowserActions {
    // browser: ChromeBrowser, // TODO: Fix ChromeBrowser compilation
    config: EmulationConfig,
}

impl BrowserActions {
    /// Create new browser actions instance
    #[deprecated(
        since = "0.2.0",
        note = "Use MexcRestClient::new() for trading operations"
    )]
    pub fn new(config: EmulationConfig) -> Self {
        // let browser = ChromeBrowser::new(config.clone());
        
        Self {
            // browser,
            config,
        }
    }
    
    /// Initialize browser and restore session
    pub async fn initialize(&mut self, _config: &EmulationConfig, _session: &SessionData) -> Result<()> {
        // TODO: Implement when ChromeBrowser is fixed
        Err(EmulationError::BrowserError("Not implemented".to_string()))
    }
        
        // Launch browser
        self.browser.launch(config).await?;
        
        // Navigate to MEXC
        self.browser.navigate("https://futures.mexc.com").await?;
        
        // Set cookies to restore session
        self.browser.set_cookies(session.cookies.clone()).await?;
        
        // Reload to apply cookies
        self.browser.navigate("https://futures.mexc.com/exchange/BTC_USDT").await?;
        
        // Wait for page to load
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Verify we're logged in
        self.verify_logged_in().await?;
        
        info!("✅ Browser initialized and logged in");
        
        Ok(())
    }
    
    /// Verify user is logged in
    async fn verify_logged_in(&mut self) -> Result<()> {
        let js = r#"
            // Check if user menu is present (indicates logged in)
            const userMenu = document.querySelector('.user-menu, .user-info, [class*="user"]');
            const loginButton = document.querySelector('[class*="login"], [href*="login"]');
            
            if (userMenu && !loginButton) {
                return { logged_in: true };
            } else {
                return { logged_in: false };
            }
        "#;
        
        let result = self.browser.execute_js(js).await?;
        
        if let Some(logged_in) = result.get("logged_in").and_then(|v| v.as_bool()) {
            if logged_in {
                return Ok(());
            }
        }
        
        Err(EmulationError::AuthError("Not logged in".to_string()))
    }
    
    /// ⚠️ DEPRECATED: Place market order via UI (2-5 second latency)
    /// 
    /// WARNING: This method has the following issues:
    /// - Latency: 2000-5000ms (vs <10ms REST API)
    /// - No atomicity: can fail mid-execution
    /// - Race conditions: conflicts with manual UI usage
    /// - No order confirmation: relies on DOM parsing
    /// 
    /// For HFT arbitrage, use:
    /// ```rust
    /// let client = MexcRestClient::new(cookies)?;
    /// let order = client.place_market_order("BTC_USDT", OrderSide::Buy, 0.001).await?;
    /// ```
    #[deprecated(
        since = "0.2.0",
        note = "Use MexcRestClient::place_market_order() for <10ms latency"
    )]
    pub async fn place_market_order(
        &mut self,
        side: &str, // "BUY" or "SELL"
        quantity: f64,
    ) -> Result<String> {
        warn!("⚠️ Using deprecated browser-based order placement (2-5s latency)");
        info!("📊 Placing {} market order for {} BTC", side, quantity);
        
        // Switch to market order tab
        self.browser.click("button[data-type='market'], .market-tab").await?;
        
        // ❌ PROBLEM: Random delay 200-500ms
        tokio::time::sleep(Duration::from_millis(350)).await;
        
        // Select side (Buy/Sell)
        let side_selector = if side == "BUY" {
            "button[data-side='buy'], .buy-button, .long-button"
        } else {
            "button[data-side='sell'], .sell-button, .short-button"
        };
        
        self.browser.click(side_selector).await?;
        
        // ❌ PROBLEM: Random delay 300-600ms
        tokio::time::sleep(Duration::from_millis(450)).await;
        
        // Enter quantity
        // ❌ PROBLEM: Heap allocation in hot path
        let quantity_str = format!("{:.4}", quantity);
        self.browser.type_text(
            "input[name='quantity'], input[placeholder*='Quantity'], .quantity-input",
            &quantity_str,
            false, // Changed to false: no human-like typing for speed
        ).await?;
        
        // ❌ PROBLEM: Random delay 500-1000ms before submit
        tokio::time::sleep(Duration::from_millis(750)).await;
        
        // Click submit button
        // ❌ PROBLEM: No retry logic if click fails
        self.browser.click("button[type='submit'], .submit-button, .place-order-button").await?;
        
        // Wait for confirmation
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Get order ID from confirmation
        // ❌ PROBLEM: Unreliable DOM parsing, may return fake ID
        let order_id = self.get_last_order_id().await?;
        
        warn!("⚠️ Order placed via UI (unreliable): {}", order_id);
        
        Ok(order_id)
    }
    
    /// Place limit order
    pub async fn place_limit_order(
        &mut self,
        side: &str,
        price: f64,
        quantity: f64,
    ) -> Result<String> {
        info!("📊 Placing {} limit order: {} BTC @ ${}", side, quantity, price);
        
        // Switch to limit order tab
        self.browser.click("button[data-type='limit'], .limit-tab").await?;
        
        tokio::time::sleep(Duration::from_millis(350)).await;
        
        // Select side
        let side_selector = if side == "BUY" {
            "button[data-side='buy'], .buy-button"
        } else {
            "button[data-side='sell'], .sell-button"
        };
        
        self.browser.click(side_selector).await?;
        
        tokio::time::sleep(Duration::from_millis(450)).await;
        
        // Enter price
        let price_str = format!("{:.2}", price);
        self.browser.type_text(
            "input[name='price'], input[placeholder*='Price'], .price-input",
            &price_str,
            true,
        ).await?;
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Enter quantity
        let quantity_str = format!("{:.4}", quantity);
        self.browser.type_text(
            "input[name='quantity'], input[placeholder*='Quantity'], .quantity-input",
            &quantity_str,
            true,
        ).await?;
        
        tokio::time::sleep(Duration::from_millis(750)).await;
        
        // Submit order
        self.browser.click("button[type='submit'], .submit-button, .place-order-button").await?;
        
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let order_id = self.get_last_order_id().await?;
        
        info!("✅ Limit order placed: {}", order_id);
        
        Ok(order_id)
    }
    
    /// Cancel order by ID
    pub async fn cancel_order(&mut self, order_id: &str) -> Result<()> {
        info!("❌ Cancelling order: {}", order_id);
        
        // Find cancel button for this order
        let cancel_selector = format!(
            "button[data-order-id='{}'] .cancel-button, tr[data-order-id='{}'] .cancel",
            order_id, order_id
        );
        
        self.browser.click(&cancel_selector).await?;
        
        // Confirm cancellation if needed
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Check for confirmation dialog
        let confirm_js = r#"
            const confirmButton = document.querySelector('.confirm-button, button[class*="confirm"]');
            if (confirmButton) {
                confirmButton.click();
                return true;
            }
            return false;
        "#;
        
        self.browser.execute_js(confirm_js).await?;
        
        info!("✅ Order cancelled");
        
        Ok(())
    }
    
    /// Get current balance
    pub async fn get_balance(&mut self) -> Result<f64> {
        debug!("💰 Getting balance");
        
        let js = r#"
            // Find balance element
            const balanceElement = document.querySelector(
                '.balance, [class*="balance"], [class*="available"]'
            );
            
            if (balanceElement) {
                const text = balanceElement.textContent || balanceElement.innerText;
                const match = text.match(/[\d,]+\.?\d*/);
                if (match) {
                    return parseFloat(match[0].replace(/,/g, ''));
                }
            }
            
            return 0;
        "#;
        
        let result = self.browser.execute_js(js).await?;
        
        let balance = result.as_f64().unwrap_or(0.0);
        
        debug!("💰 Balance: ${:.2}", balance);
        
        Ok(balance)
    }
    
    /// Get open positions
    pub async fn get_open_positions(&mut self) -> Result<Vec<Position>> {
        debug!("📊 Getting open positions");
        
        let js = r#"
            const positions = [];
            const rows = document.querySelectorAll('.position-row, tr[class*="position"]');
            
            rows.forEach(row => {
                const symbol = row.querySelector('[class*="symbol"]')?.textContent;
                const side = row.querySelector('[class*="side"]')?.textContent;
                const size = row.querySelector('[class*="size"]')?.textContent;
                const entryPrice = row.querySelector('[class*="entry"]')?.textContent;
                const pnl = row.querySelector('[class*="pnl"]')?.textContent;
                
                if (symbol && side && size) {
                    positions.push({
                        symbol: symbol.trim(),
                        side: side.trim(),
                        size: parseFloat(size.replace(/[^\d.]/g, '')),
                        entry_price: parseFloat(entryPrice?.replace(/[^\d.]/g, '') || '0'),
                        pnl: parseFloat(pnl?.replace(/[^\d.-]/g, '') || '0')
                    });
                }
            });
            
            return positions;
        "#;
        
        let result = self.browser.execute_js(js).await?;
        
        let positions: Vec<Position> = serde_json::from_value(result)
            .unwrap_or_default();
        
        debug!("📊 Found {} open positions", positions.len());
        
        Ok(positions)
    }
    
    /// Get last order ID from UI
    async fn get_last_order_id(&mut self) -> Result<String> {
        let js = r#"
            // Try to find order ID in confirmation or order list
            const orderElements = document.querySelectorAll(
                '[class*="order-id"], [data-order-id]'
            );
            
            if (orderElements.length > 0) {
                const lastOrder = orderElements[0];
                return lastOrder.getAttribute('data-order-id') || 
                       lastOrder.textContent.trim();
            }
            
            // Generate temporary ID if not found
            return 'order_' + Date.now();
        "#;
        
        let result = self.browser.execute_js(js).await?;
        
        Ok(result.as_str().unwrap_or("unknown").to_string())
    }
    
    /// Simulate human activity (view charts, check balance, etc.)
    pub async fn simulate_human_activity(&mut self) -> Result<()> {
        info!("👤 Simulating human activity");
        
        // Random activity
        let activity = rand::random::<f64>();
        
        if activity < 0.3 {
            // Check balance
            self.get_balance().await?;
        } else if activity < 0.6 {
            // View positions
            self.get_open_positions().await?;
        } else {
            // Scroll chart
            let scroll_js = "window.scrollBy(0, 100);";
            self.browser.execute_js(scroll_js).await?;
            
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            let scroll_back_js = "window.scrollBy(0, -100);";
            self.browser.execute_js(scroll_back_js).await?;
        }
        
        debug!("✅ Human activity simulated");
        
        Ok(())
    }
    
    /// Close browser
    pub async fn close(&mut self) -> Result<()> {
        self.browser.close().await
    }
}

/// Position information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub pnl: f64,
}
