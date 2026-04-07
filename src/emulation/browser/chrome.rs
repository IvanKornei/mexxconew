use chromiumoxide::{
    Browser as ChromiumBrowser, BrowserConfig,
    cdp::browser_protocol::network::CookieParam,
    handler::viewport::Viewport,
};
use futures_util::StreamExt;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::emulation::{
    behavior::BehaviorSimulator,
    config::EmulationConfig,
    errors::{EmulationError, Result},
    persistence::Cookie,
};

use super::BrowserAutomation;

/// Chrome browser automation using CDP
pub struct ChromeBrowser {
    browser: Option<ChromiumBrowser>,
    behavior: BehaviorSimulator,
    config: EmulationConfig,
}

impl ChromeBrowser {
    /// Create new Chrome browser instance
    pub fn new(config: EmulationConfig) -> Self {
        let behavior = BehaviorSimulator::new(config.behavior.clone());
        
        Self {
            browser: None,
            behavior,
            config,
        }
    }
    
    /// Get browser config with full emulation
    fn get_browser_config(&self) -> BrowserConfig {
        let mut config = BrowserConfig::builder()
            .window_size(1920, 1080)
            .viewport(Viewport {
                width: 1920,
                height: 1080,
                device_scale_factor: Some(1.0),
                emulating_mobile: false,
                is_landscape: true,
                has_touch: false,
            });
        
        // Add Chrome arguments for stealth mode
        let args = vec![
            // Disable automation flags
            "--disable-blink-features=AutomationControlled",
            
            // User agent and platform
            "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36",
            
            // Language
            "--lang=ru-RU,ru",
            
            // Disable features that reveal automation
            "--disable-dev-shm-usage",
            "--disable-setuid-sandbox",
            "--no-sandbox",
            
            // Enable features for realism
            "--enable-features=NetworkService,NetworkServiceInProcess",
            
            // WebGL and Canvas
            "--enable-webgl",
            "--enable-accelerated-2d-canvas",
            
            // Audio
            "--autoplay-policy=no-user-gesture-required",
            
            // Timezone
            "--timezone-id=Europe/Moscow",
        ];
        
        for arg in args {
            config = config.arg(arg);
        }
        
        config.build().expect("Failed to build browser config")
    }
    
    /// Inject stealth scripts to hide automation
    async fn inject_stealth_scripts(&mut self) -> Result<()> {
        let stealth_js = r#"
            // Override navigator.webdriver
            Object.defineProperty(navigator, 'webdriver', {
                get: () => undefined
            });
            
            // Override chrome runtime
            window.chrome = {
                runtime: {}
            };
            
            // Override permissions
            const originalQuery = window.navigator.permissions.query;
            window.navigator.permissions.query = (parameters) => (
                parameters.name === 'notifications' ?
                    Promise.resolve({ state: Notification.permission }) :
                    originalQuery(parameters)
            );
            
            // Override plugins
            Object.defineProperty(navigator, 'plugins', {
                get: () => [
                    {
                        0: {type: "application/x-google-chrome-pdf", suffixes: "pdf", description: "Portable Document Format"},
                        description: "Portable Document Format",
                        filename: "internal-pdf-viewer",
                        length: 1,
                        name: "Chrome PDF Plugin"
                    },
                    {
                        0: {type: "application/pdf", suffixes: "pdf", description: "Portable Document Format"},
                        description: "Portable Document Format",
                        filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai",
                        length: 1,
                        name: "Chrome PDF Viewer"
                    },
                    {
                        0: {type: "application/x-nacl", suffixes: "", description: "Native Client Executable"},
                        1: {type: "application/x-pnacl", suffixes: "", description: "Portable Native Client Executable"},
                        description: "",
                        filename: "internal-nacl-plugin",
                        length: 2,
                        name: "Native Client"
                    }
                ]
            });
            
            // Override languages
            Object.defineProperty(navigator, 'languages', {
                get: () => ['ru-RU', 'ru', 'en-US', 'en']
            });
            
            // Override platform
            Object.defineProperty(navigator, 'platform', {
                get: () => 'Win32'
            });
            
            // Override vendor
            Object.defineProperty(navigator, 'vendor', {
                get: () => 'Google Inc.'
            });
            
            // Override hardwareConcurrency
            Object.defineProperty(navigator, 'hardwareConcurrency', {
                get: () => 8
            });
            
            // Override deviceMemory
            Object.defineProperty(navigator, 'deviceMemory', {
                get: () => 8
            });
            
            // Override screen
            Object.defineProperty(screen, 'width', {
                get: () => 1920
            });
            Object.defineProperty(screen, 'height', {
                get: () => 1080
            });
            Object.defineProperty(screen, 'availWidth', {
                get: () => 1920
            });
            Object.defineProperty(screen, 'availHeight', {
                get: () => 1040
            });
            Object.defineProperty(screen, 'colorDepth', {
                get: () => 24
            });
            Object.defineProperty(screen, 'pixelDepth', {
                get: () => 24
            });
            
            console.log('🎭 Stealth mode activated');
        "#;
        
        self.execute_js(stealth_js).await?;
        debug!("✅ Stealth scripts injected");
        
        Ok(())
    }
    
    /// Get current page
    fn get_page(&self) -> Result<&chromiumoxide::Page> {
        self.browser
            .as_ref()
            .and_then(|b| b.pages().first())
            .ok_or_else(|| EmulationError::BrowserError("No page available".to_string()))
    }
}

#[async_trait::async_trait]
impl BrowserAutomation for ChromeBrowser {
    async fn launch(&mut self, config: &EmulationConfig) -> Result<()> {
        info!("🚀 Launching Chrome browser with full emulation");
        
        let browser_config = self.get_browser_config();
        
        let (browser, mut handler) = ChromiumBrowser::launch(browser_config)
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to launch browser: {}", e)))?;
        
        // Spawn handler task
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(e) = event {
                    warn!("Browser handler error: {}", e);
                }
            }
        });
        
        self.browser = Some(browser);
        
        // Create initial page
        let page = self.browser.as_ref().unwrap().new_page("about:blank")
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to create page: {}", e)))?;
        
        // Inject stealth scripts
        self.inject_stealth_scripts().await?;
        
        info!("✅ Chrome browser launched successfully");
        
        Ok(())
    }
    
    async fn navigate(&mut self, url: &str) -> Result<()> {
        info!("🌐 Navigating to: {}", url);
        
        let page = self.get_page()?;
        
        page.goto(url)
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Navigation failed: {}", e)))?;
        
        // Wait for page load
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Re-inject stealth scripts after navigation
        self.inject_stealth_scripts().await?;
        
        debug!("✅ Navigation complete");
        
        Ok(())
    }
    
    async fn execute_js(&mut self, script: &str) -> Result<serde_json::Value> {
        let page = self.get_page()?;
        
        let result = page.evaluate(script)
            .await
            .map_err(|e| EmulationError::BrowserError(format!("JS execution failed: {}", e)))?;
        
        Ok(result.into_value()
            .map_err(|e| EmulationError::BrowserError(format!("Failed to parse JS result: {}", e)))?)
    }
    
    async fn set_cookies(&mut self, cookies: Vec<Cookie>) -> Result<()> {
        let page = self.get_page()?;
        
        for cookie in cookies {
            let cookie_param = CookieParam {
                name: cookie.name,
                value: cookie.value,
                domain: Some(cookie.domain),
                path: Some(cookie.path),
                secure: Some(cookie.secure),
                http_only: Some(cookie.http_only),
                expires: cookie.expires.map(|dt| dt.timestamp() as f64),
                ..Default::default()
            };
            
            page.set_cookie(cookie_param)
                .await
                .map_err(|e| EmulationError::BrowserError(format!("Failed to set cookie: {}", e)))?;
        }
        
        debug!("✅ Cookies set successfully");
        
        Ok(())
    }
    
    async fn get_cookies(&mut self) -> Result<Vec<Cookie>> {
        let page = self.get_page()?;
        
        let cdp_cookies = page.get_cookies()
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to get cookies: {}", e)))?;
        
        let cookies = cdp_cookies
            .into_iter()
            .map(|c| Cookie {
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                secure: c.secure,
                http_only: c.http_only,
                expires: c.expires.and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0)),
            })
            .collect();
        
        Ok(cookies)
    }
    
    async fn click(&mut self, selector: &str) -> Result<()> {
        debug!("🖱️ Clicking: {}", selector);
        
        let page = self.get_page()?;
        
        // Wait for element
        self.wait_for_element(selector, 5000).await?;
        
        // Simulate human-like delay before click
        let delay = self.behavior.simulate_click_delay();
        tokio::time::sleep(delay).await;
        
        // Find element
        let element = page.find_element(selector)
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Element not found: {}", e)))?;
        
        // Click
        element.click()
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Click failed: {}", e)))?;
        
        debug!("✅ Click successful");
        
        Ok(())
    }
    
    async fn type_text(&mut self, selector: &str, text: &str, human_like: bool) -> Result<()> {
        debug!("⌨️ Typing into: {}", selector);
        
        let page = self.get_page()?;
        
        // Wait for element
        self.wait_for_element(selector, 5000).await?;
        
        // Find element
        let element = page.find_element(selector)
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Element not found: {}", e)))?;
        
        // Click to focus
        element.click().await
            .map_err(|e| EmulationError::BrowserError(format!("Failed to focus: {}", e)))?;
        
        if human_like {
            // Type with human-like delays
            let typing_events = self.behavior.simulate_typing(text);
            
            for (char, delay) in typing_events {
                element.type_str(&char.to_string()).await
                    .map_err(|e| EmulationError::BrowserError(format!("Typing failed: {}", e)))?;
                tokio::time::sleep(delay).await;
            }
        } else {
            // Type instantly
            element.type_str(text).await
                .map_err(|e| EmulationError::BrowserError(format!("Typing failed: {}", e)))?;
        }
        
        debug!("✅ Typing complete");
        
        Ok(())
    }
    
    async fn wait_for_element(&mut self, selector: &str, timeout_ms: u64) -> Result<()> {
        let page = self.get_page()?;
        
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() > timeout {
                return Err(EmulationError::BrowserError(
                    format!("Timeout waiting for element: {}", selector)
                ));
            }
            
            match page.find_element(selector).await {
                Ok(_) => {
                    debug!("✅ Element found: {}", selector);
                    return Ok(());
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    
    async fn screenshot(&mut self, path: &str) -> Result<()> {
        let page = self.get_page()?;
        
        let screenshot = page.screenshot()
            .await
            .map_err(|e| EmulationError::BrowserError(format!("Screenshot failed: {}", e)))?;
        
        std::fs::write(path, screenshot)
            .map_err(|e| EmulationError::IoError(e))?;
        
        info!("📸 Screenshot saved: {}", path);
        
        Ok(())
    }
    
    async fn close(&mut self) -> Result<()> {
        if let Some(browser) = self.browser.take() {
            browser.close()
                .await
                .map_err(|e| EmulationError::BrowserError(format!("Failed to close browser: {}", e)))?;
            
            info!("✅ Browser closed");
        }
        
        Ok(())
    }
}

impl Drop for ChromeBrowser {
    fn drop(&mut self) {
        if self.browser.is_some() {
            warn!("⚠️ Browser not properly closed, forcing cleanup");
        }
    }
}
