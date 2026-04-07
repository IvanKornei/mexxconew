use std::path::PathBuf;
use directories::UserDirs;
use rusqlite::Connection;
use tracing::{debug, info, warn};

use crate::emulation::{
    errors::{EmulationError, Result},
    persistence::Cookie,
};

/// Supported browsers for cookie extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Chrome,
    Edge,
    Firefox,
}

impl Browser {
    /// Get the cookie database path for this browser on Windows
    fn cookie_db_path(&self) -> Result<PathBuf> {
        let user_dirs = UserDirs::new()
            .ok_or_else(|| EmulationError::BrowserCookieError("Cannot find user directories".to_string()))?;
        
        let local_app_data = user_dirs.home_dir()
            .join("AppData")
            .join("Local");
        
        let path = match self {
            Browser::Chrome => local_app_data
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Default")
                .join("Network")
                .join("Cookies"),
            
            Browser::Edge => local_app_data
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Default")
                .join("Network")
                .join("Cookies"),
            
            Browser::Firefox => {
                // Firefox uses profiles, need to find the default profile
                let roaming_app_data = user_dirs.home_dir()
                    .join("AppData")
                    .join("Roaming");
                
                let profiles_dir = roaming_app_data
                    .join("Mozilla")
                    .join("Firefox")
                    .join("Profiles");
                
                // Find first .default-release profile
                if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                            
                            if name.contains(".default-release") || name.contains(".default") {
                                return Ok(path.join("cookies.sqlite"));
                            }
                        }
                    }
                }
                
                return Err(EmulationError::BrowserCookieError(
                    "Cannot find Firefox default profile".to_string()
                ));
            }
        };
        
        if !path.exists() {
            return Err(EmulationError::BrowserCookieError(
                format!("Cookie database not found at: {}", path.display())
            ));
        }
        
        Ok(path)
    }
    
    /// Get browser name for logging
    fn name(&self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome",
            Browser::Edge => "Edge",
            Browser::Firefox => "Firefox",
        }
    }
}

/// Browser cookie extractor
pub struct BrowserCookieExtractor {
    browser: Browser,
}

impl BrowserCookieExtractor {
    /// Create a new cookie extractor for the specified browser
    pub fn new(browser: Browser) -> Self {
        Self { browser }
    }
    
    /// Try to detect and use the first available browser
    pub fn auto_detect() -> Result<Self> {
        // Try browsers in order of preference
        for browser in [Browser::Chrome, Browser::Edge, Browser::Firefox] {
            if browser.cookie_db_path().is_ok() {
                info!("🔍 Auto-detected browser: {}", browser.name());
                return Ok(Self::new(browser));
            }
        }
        
        Err(EmulationError::BrowserCookieError(
            "No supported browser found with accessible cookies".to_string()
        ))
    }
    
    /// Extract cookies for MEXC domains
    pub fn extract_mexc_cookies(&self) -> Result<Vec<Cookie>> {
        info!("🍪 Extracting cookies from {} browser", self.browser.name());
        
        let db_path = self.browser.cookie_db_path()?;
        debug!("Cookie database path: {}", db_path.display());
        
        // Copy the database to a temporary location to avoid locking issues
        let temp_db = self.copy_to_temp(&db_path)?;
        
        // Extract cookies from the temporary database
        let cookies = self.extract_from_db(&temp_db)?;
        
        // Clean up temporary file
        let _ = std::fs::remove_file(temp_db);
        
        info!("✅ Extracted {} cookies from {}", cookies.len(), self.browser.name());
        Ok(cookies)
    }
    
    /// Copy database to temporary location
    fn copy_to_temp(&self, db_path: &PathBuf) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let temp_db = temp_dir.join(format!("mexc_cookies_{}.db", chrono::Utc::now().timestamp()));
        
        std::fs::copy(db_path, &temp_db)
            .map_err(|e| EmulationError::BrowserCookieError(
                format!("Failed to copy cookie database: {}", e)
            ))?;
        
        Ok(temp_db)
    }
    
    /// Extract cookies from database
    fn extract_from_db(&self, db_path: &PathBuf) -> Result<Vec<Cookie>> {
        let conn = Connection::open(db_path)
            .map_err(|e| EmulationError::BrowserCookieError(
                format!("Failed to open cookie database: {}", e)
            ))?;
        
        let mut cookies = Vec::new();
        
        // Query depends on browser type
        match self.browser {
            Browser::Chrome | Browser::Edge => {
                cookies.extend(self.extract_chromium_cookies(&conn)?);
            }
            Browser::Firefox => {
                cookies.extend(self.extract_firefox_cookies(&conn)?);
            }
        }
        
        Ok(cookies)
    }
    
    /// Extract cookies from Chromium-based browsers (Chrome, Edge)
    fn extract_chromium_cookies(&self, conn: &Connection) -> Result<Vec<Cookie>> {
        let mut stmt = conn.prepare(
            "SELECT name, value, host_key, path, is_secure, is_httponly, expires_utc 
             FROM cookies 
             WHERE host_key LIKE '%mexc.com%' OR host_key LIKE '%futures.mexc.com%'"
        ).map_err(|e| EmulationError::BrowserCookieError(
            format!("Failed to prepare query: {}", e)
        ))?;
        
        let cookie_iter = stmt.query_map([], |row| {
            Ok(Cookie {
                name: row.get(0)?,
                value: row.get(1)?,
                domain: row.get(2)?,
                path: row.get(3)?,
                secure: row.get::<_, i32>(4)? != 0,
                http_only: row.get::<_, i32>(5)? != 0,
                expires: row.get::<_, Option<i64>>(6).ok()
                    .flatten()
                    .and_then(|ts| {
                        // Chrome stores time as microseconds since Windows epoch (1601-01-01)
                        // Convert to Unix timestamp
                        if ts > 0 {
                            let unix_ts = (ts - 11644473600000000) / 1000000;
                            chrono::DateTime::from_timestamp(unix_ts, 0)
                        } else {
                            None
                        }
                    }),
            })
        }).map_err(|e| EmulationError::BrowserCookieError(
            format!("Failed to query cookies: {}", e)
        ))?;
        
        let mut cookies = Vec::new();
        for cookie_result in cookie_iter {
            match cookie_result {
                Ok(cookie) => {
                    debug!("Found cookie: {} = {} (domain: {})", 
                        cookie.name, 
                        if cookie.value.len() > 20 { 
                            format!("{}...", &cookie.value[..20]) 
                        } else { 
                            cookie.value.clone() 
                        },
                        cookie.domain
                    );
                    cookies.push(cookie);
                }
                Err(e) => {
                    warn!("Failed to parse cookie: {}", e);
                }
            }
        }
        
        Ok(cookies)
    }
    
    /// Extract cookies from Firefox
    fn extract_firefox_cookies(&self, conn: &Connection) -> Result<Vec<Cookie>> {
        let mut stmt = conn.prepare(
            "SELECT name, value, host, path, isSecure, isHttpOnly, expiry 
             FROM moz_cookies 
             WHERE host LIKE '%mexc.com%' OR host LIKE '%futures.mexc.com%'"
        ).map_err(|e| EmulationError::BrowserCookieError(
            format!("Failed to prepare query: {}", e)
        ))?;
        
        let cookie_iter = stmt.query_map([], |row| {
            Ok(Cookie {
                name: row.get(0)?,
                value: row.get(1)?,
                domain: row.get(2)?,
                path: row.get(3)?,
                secure: row.get::<_, i32>(4)? != 0,
                http_only: row.get::<_, i32>(5)? != 0,
                expires: row.get::<_, Option<i64>>(6).ok()
                    .flatten()
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            })
        }).map_err(|e| EmulationError::BrowserCookieError(
            format!("Failed to query cookies: {}", e)
        ))?;
        
        let mut cookies = Vec::new();
        for cookie_result in cookie_iter {
            match cookie_result {
                Ok(cookie) => {
                    debug!("Found cookie: {} = {} (domain: {})", 
                        cookie.name, 
                        if cookie.value.len() > 20 { 
                            format!("{}...", &cookie.value[..20]) 
                        } else { 
                            cookie.value.clone() 
                        },
                        cookie.domain
                    );
                    cookies.push(cookie);
                }
                Err(e) => {
                    warn!("Failed to parse cookie: {}", e);
                }
            }
        }
        
        Ok(cookies)
    }
}

/// Manual cookie input helper
pub struct ManualCookieInput;

impl ManualCookieInput {
    /// Parse cookies from browser DevTools format
    /// 
    /// Format: "name1=value1; name2=value2; name3=value3"
    pub fn parse_from_string(cookie_string: &str, domain: &str) -> Vec<Cookie> {
        cookie_string
            .split(';')
            .filter_map(|pair| {
                let pair = pair.trim();
                if let Some((name, value)) = pair.split_once('=') {
                    Some(Cookie {
                        name: name.trim().to_string(),
                        value: value.trim().to_string(),
                        domain: domain.to_string(),
                        path: "/".to_string(),
                        secure: true,
                        http_only: true,
                        expires: None,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Parse cookies from JSON format
    /// 
    /// Format: [{"name": "...", "value": "...", "domain": "..."}, ...]
    pub fn parse_from_json(json_string: &str) -> Result<Vec<Cookie>> {
        serde_json::from_str(json_string)
            .map_err(|e| EmulationError::BrowserCookieError(
                format!("Failed to parse JSON cookies: {}", e)
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_cookie_string() {
        let cookie_str = "session_id=abc123; auth_token=xyz789; user_id=12345";
        let cookies = ManualCookieInput::parse_from_string(cookie_str, "mexc.com");
        
        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0].name, "session_id");
        assert_eq!(cookies[0].value, "abc123");
        assert_eq!(cookies[1].name, "auth_token");
        assert_eq!(cookies[1].value, "xyz789");
    }
    
    #[test]
    fn test_parse_cookie_json() {
        let json = r#"[
            {"name": "session_id", "value": "abc123", "domain": "mexc.com", "path": "/", "secure": true, "http_only": true, "expires": null}
        ]"#;
        
        let cookies = ManualCookieInput::parse_from_json(json).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "session_id");
    }
}
