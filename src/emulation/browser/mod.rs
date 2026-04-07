/// Full browser automation module
/// 
/// This module provides complete Chrome browser emulation using Chrome DevTools Protocol (CDP).
/// The browser is indistinguishable from a real user session.

// TODO: Fix compilation errors in chrome.rs
// pub mod chrome;
// #[allow(deprecated)]
// pub mod actions;
pub mod actions_stub;
// pub mod fingerprint;
// pub mod session_manager;

use crate::emulation::{
    config::EmulationConfig,
    errors::Result,
};

// pub use chrome::ChromeBrowser;
pub use actions_stub::BrowserActions;
// pub use fingerprint::BrowserFingerprint;
// pub use session_manager::SessionManager;

/// Browser automation interface
#[async_trait::async_trait]
pub trait BrowserAutomation: Send + Sync {
    /// Launch browser with specified configuration
    async fn launch(&mut self, config: &EmulationConfig) -> Result<()>;
    
    /// Navigate to URL
    async fn navigate(&mut self, url: &str) -> Result<()>;
    
    /// Execute JavaScript in page context
    async fn execute_js(&mut self, script: &str) -> Result<serde_json::Value>;
    
    /// Set cookies
    async fn set_cookies(&mut self, cookies: Vec<crate::emulation::persistence::Cookie>) -> Result<()>;
    
    /// Get cookies
    async fn get_cookies(&mut self) -> Result<Vec<crate::emulation::persistence::Cookie>>;
    
    /// Click element by selector
    async fn click(&mut self, selector: &str) -> Result<()>;
    
    /// Type text into element
    async fn type_text(&mut self, selector: &str, text: &str, human_like: bool) -> Result<()>;
    
    /// Wait for element
    async fn wait_for_element(&mut self, selector: &str, timeout_ms: u64) -> Result<()>;
    
    /// Take screenshot
    async fn screenshot(&mut self, path: &str) -> Result<()>;
    
    /// Close browser
    async fn close(&mut self) -> Result<()>;
}
