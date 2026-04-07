/// Session management using browser automation
/// 
/// This module is ONLY for cookie extraction and session refresh.
/// DO NOT use for order execution in HFT scenarios.

use tracing::{debug, info, warn};
use std::time::Duration;

use crate::emulation::{
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::{Cookie, SessionData},
};

use super::{BrowserAutomation, ChromeBrowser};

/// Session manager for cookie-based authentication
pub struct SessionManager {
    browser: Option<ChromeBrowser>,
    config: EmulationConfig,
    last_refresh: Option<std::time::Instant>,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(config: EmulationConfig) -> Self {
        Self {
            browser: None,
            config,
            last_refresh: None,
        }
    }
    
    /// Extract cookies from browser (one-time operation at startup)
    pub async fn extract_cookies(&mut self) -> Result<SessionData> {
        info!("🍪 Extracting cookies from browser");
        
        let mut browser = ChromeBrowser::new(self.config.clone());
        
        // Launch browser
        browser.launch(&self.config).await?;
        
        // Navigate to MEXC login page
        browser.navigate("https://futures.mexc.com").await?;
        
        // Wait for user to login manually
        info!("⏳ Waiting for manual login (60 seconds)...");
        tokio::time::sleep(Duration::from_secs(60)).await;
        
        // Extract cookies
        let cookies = browser.get_cookies().await?;
        
        // Close browser
        browser.close().await?;
        
        info!("✅ Extracted {} cookies", cookies.len());
        
        Ok(SessionData {
            cookies,
            created_at: chrono::Utc::now(),
            expires_at: None,
            source: "browser".to_string(),
        })
    }
    
    /// Refresh existing session cookies
    pub async fn refresh_cookies(&mut self, current_session: &SessionData) -> Result<SessionData> {
        info!("🔄 Refreshing session cookies");
        
        // Check if refresh is needed
        if let Some(last_refresh) = self.last_refresh {
            let elapsed = last_refresh.elapsed();
            let min_interval = Duration::from_secs(self.config.cookies.auto_refresh_interval_minutes * 60);
            
            if elapsed < min_interval {
                debug!("⏭️ Skipping refresh, last refresh was {:?} ago", elapsed);
                return Ok(current_session.clone());
            }
        }
        
        let mut browser = ChromeBrowser::new(self.config.clone());
        
        // Launch browser
        browser.launch(&self.config).await?;
        
        // Navigate to MEXC
        browser.navigate("https://futures.mexc.com").await?;
        
        // Set existing cookies to restore session
        browser.set_cookies(current_session.cookies.clone()).await?;
        
        // Reload to apply cookies
        browser.navigate("https://futures.mexc.com/exchange/BTC_USDT").await?;
        
        // Wait for page load
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Verify session is valid
        self.verify_logged_in(&mut browser).await?;
        
        // Extract refreshed cookies
        let cookies = browser.get_cookies().await?;
        
        // Close browser
        browser.close().await?;
        
        self.last_refresh = Some(std::time::Instant::now());
        
        info!("✅ Session refreshed successfully");
        
        Ok(SessionData {
            cookies,
            created_at: chrono::Utc::now(),
            expires_at: current_session.expires_at,
            source: "browser_refresh".to_string(),
        })
    }
    
    /// Verify user is logged in
    async fn verify_logged_in(&self, browser: &mut ChromeBrowser) -> Result<()> {
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
        
        let result = browser.execute_js(js).await?;
        
        if let Some(logged_in) = result.get("logged_in").and_then(|v| v.as_bool()) {
            if logged_in {
                return Ok(());
            }
        }
        
        Err(EmulationError::AuthError("Not logged in".to_string()))
    }
    
    /// Test if session is valid without browser
    pub async fn test_session(&self, session: &SessionData) -> Result<bool> {
        // Use HTTP client to test cookies
        let client = reqwest::Client::new();
        
        let cookie_header = session.cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        
        let response = client
            .get("https://futures.mexc.com/api/v1/private/account/assets")
            .header("Cookie", cookie_header)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        
        Ok(response.status().is_success())
    }
}
