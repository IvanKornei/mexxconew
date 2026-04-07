/// TLS profile optimization strategies for different use cases
/// 
/// This module provides optimized TLS configurations for different phases
/// of the emulation system to balance stealth and performance.

use super::{TLSEmulator, Http2Settings};
use crate::emulation::config::BrowserProfile;
use std::sync::Arc;

/// TLS profile for session initialization (slow path)
/// 
/// Full browser emulation with all headers and fingerprints.
/// Used during SessionInitializer phase (15-30 seconds).
pub struct InitializationProfile {
    emulator: TLSEmulator,
}

impl InitializationProfile {
    pub fn new(profile: BrowserProfile) -> Self {
        Self {
            emulator: TLSEmulator::new(profile),
        }
    }

    /// Get full ordered headers for browser emulation
    pub fn get_full_headers(&self) -> &[(&'static str, &'static str)] {
        self.emulator.get_ordered_headers()
    }

    /// Get all browser-specific headers
    pub fn get_user_agent(&self) -> &'static str {
        self.emulator.get_user_agent()
    }

    pub fn get_sec_ch_ua(&self) -> &'static str {
        self.emulator.get_sec_ch_ua()
    }

    pub fn get_sec_ch_ua_platform(&self) -> &'static str {
        self.emulator.get_sec_ch_ua_platform()
    }

    pub fn get_accept_language(&self) -> &'static str {
        self.emulator.get_accept_language()
    }

    pub fn get_accept_encoding(&self) -> &'static str {
        self.emulator.get_accept_encoding()
    }

    pub fn get_ja3_fingerprint(&self) -> &str {
        self.emulator.generate_ja3_fingerprint()
    }

    pub fn get_ja4_fingerprint(&self) -> &str {
        self.emulator.generate_ja4_fingerprint()
    }

    pub fn get_http2_settings(&self) -> Arc<Http2Settings> {
        self.emulator.configure_http2_settings()
    }
}

/// TLS profile for high-speed trading (fast path)
/// 
/// Minimal emulation overhead for <1ms latency requirement.
/// Used during TradingExecutor phase for WebSocket connections.
pub struct TradingProfile {
    emulator: TLSEmulator,
}

impl TradingProfile {
    pub fn new(profile: BrowserProfile) -> Self {
        Self {
            emulator: TLSEmulator::new(profile),
        }
    }

    /// Get minimal headers for WebSocket upgrade
    /// 
    /// Only essential headers to maintain fingerprint consistency
    /// while minimizing serialization overhead.
    pub fn get_minimal_headers(&self) -> &[(&'static str, &'static str)] {
        // Only pseudo-headers and critical fingerprint headers
        match self.emulator.profile {
            BrowserProfile::Chrome145Windows64 => &[
                (":method", "GET"),
                (":authority", ""),  // Filled by caller
                (":scheme", "https"),
                (":path", "/ws"),
                ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"),
                ("upgrade", "websocket"),
                ("connection", "Upgrade"),
            ],
        }
    }

    /// Get cached JA3 fingerprint (zero-cost)
    pub fn get_ja3_fingerprint(&self) -> &str {
        self.emulator.generate_ja3_fingerprint()
    }

    /// Get cached HTTP/2 settings (Arc clone, no allocation)
    pub fn get_http2_settings(&self) -> Arc<Http2Settings> {
        self.emulator.configure_http2_settings()
    }

    /// Get User-Agent only (most critical header)
    pub fn get_user_agent(&self) -> &'static str {
        self.emulator.get_user_agent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_profile_has_full_headers() {
        let profile = InitializationProfile::new(BrowserProfile::Chrome145Windows64);
        let headers = profile.get_full_headers();
        
        // Should have all headers for full emulation
        assert!(headers.len() > 10);
        assert!(headers.iter().any(|(k, _)| *k == "sec-ch-ua"));
        assert!(headers.iter().any(|(k, _)| *k == "accept-language"));
    }

    #[test]
    fn test_trading_profile_has_minimal_headers() {
        let profile = TradingProfile::new(BrowserProfile::Chrome145Windows64);
        let headers = profile.get_minimal_headers();
        
        // Should have minimal headers for speed
        assert!(headers.len() <= 7);
        assert!(headers.iter().any(|(k, _)| *k == "user-agent"));
        assert!(headers.iter().any(|(k, _)| *k == "upgrade"));
        
        // Should NOT have non-essential headers
        assert!(!headers.iter().any(|(k, _)| *k == "accept-language"));
        assert!(!headers.iter().any(|(k, _)| *k == "sec-ch-ua"));
    }

    #[test]
    fn test_profiles_share_fingerprints() {
        let init_profile = InitializationProfile::new(BrowserProfile::Chrome145Windows64);
        let trading_profile = TradingProfile::new(BrowserProfile::Chrome145Windows64);
        
        // Both should return same JA3 (consistency)
        assert_eq!(init_profile.get_ja3_fingerprint(), trading_profile.get_ja3_fingerprint());
        
        // Both should return same User-Agent
        assert_eq!(init_profile.get_user_agent(), trading_profile.get_user_agent());
    }

    #[test]
    fn test_http2_settings_are_shared() {
        let profile1 = TradingProfile::new(BrowserProfile::Chrome145Windows64);
        let profile2 = TradingProfile::new(BrowserProfile::Chrome145Windows64);
        
        let settings1 = profile1.get_http2_settings();
        let settings2 = profile2.get_http2_settings();
        
        // Should be Arc-cloned, pointing to same data
        assert_eq!(Arc::strong_count(&settings1), Arc::strong_count(&settings2));
    }
}
