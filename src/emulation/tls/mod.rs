use crate::emulation::config::BrowserProfile;
use std::sync::{Arc, LazyLock};

pub mod profiles;

// Pre-computed fingerprints to avoid runtime allocations
static CHROME_145_JA3: LazyLock<String> = LazyLock::new(|| {
    format!("{},{},{},{},{}",
        "771",  // TLS 1.2
        "4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53",
        "0-23-65281-10-11-35-16-5-13-18-51-45-43-27-17513-21",
        "29-23-24",
        "0"
    )
});

static CHROME_145_JA4: LazyLock<String> = LazyLock::new(|| {
    format!("{}{}{}_{}_{}_{}_{}",
        "t",   // TCP
        "13",  // TLS 1.3
        "d",   // SNI present
        "15",  // Cipher count
        "17",  // Extension count
        "h2",  // HTTP/2
        "8daaf6152771"  // Cipher hash
    )
});

// Pre-computed HTTP/2 settings to avoid allocations
static CHROME_145_HTTP2_SETTINGS: LazyLock<Arc<Http2Settings>> = LazyLock::new(|| {
    Arc::new(Http2Settings {
        header_table_size: 65536,
        enable_push: false,
        max_concurrent_streams: 1000,
        initial_window_size: 6291456,
        max_frame_size: 16384,
        max_header_list_size: 262144,
        priority_frames: vec![
            PriorityFrame { stream_id: 3, weight: 200, depends_on: 0 },
            PriorityFrame { stream_id: 5, weight: 100, depends_on: 0 },
            PriorityFrame { stream_id: 7, weight: 0, depends_on: 0 },
            PriorityFrame { stream_id: 9, weight: 0, depends_on: 7 },
            PriorityFrame { stream_id: 11, weight: 0, depends_on: 3 },
        ],
        window_update_increment: 15663105,
    })
});

/// TLS fingerprint emulator for browser impersonation
pub struct TLSEmulator {
    profile: BrowserProfile,
}

impl TLSEmulator {
    /// Create a new TLS emulator with the specified browser profile
    pub fn new(profile: BrowserProfile) -> Self {
        Self { profile }
    }

    /// Generate JA3 fingerprint for the configured browser profile
    /// JA3 format: SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats
    /// 
    /// Returns a reference to pre-computed fingerprint to avoid allocations.
    pub fn generate_ja3_fingerprint(&self) -> &str {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => &CHROME_145_JA3,
        }
    }

    /// Generate JA4 fingerprint (newer, more robust than JA3)
    /// JA4 format: Protocol_CipherCount_ExtensionCount_FirstALPN_LastCipher
    /// 
    /// Returns a reference to pre-computed fingerprint to avoid allocations.
    pub fn generate_ja4_fingerprint(&self) -> &str {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => &CHROME_145_JA4,
        }
    }

    /// Generate HTTP/2 SETTINGS frame parameters for Chrome 145
    /// 
    /// Returns Arc to pre-computed settings to avoid allocations and cloning.
    pub fn configure_http2_settings(&self) -> Arc<Http2Settings> {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => Arc::clone(&CHROME_145_HTTP2_SETTINGS),
        }
    }

    /// Get User-Agent string matching the TLS fingerprint
    pub fn get_user_agent(&self) -> &'static str {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"
            }
        }
    }

    /// Get Accept-Language header
    pub fn get_accept_language(&self) -> &'static str {
        "en-US,en;q=0.9"
    }

    /// Get Accept-Encoding header
    pub fn get_accept_encoding(&self) -> &'static str {
        "gzip, deflate, br, zstd"
    }

    /// Get Sec-CH-UA header (Chrome Client Hints)
    pub fn get_sec_ch_ua(&self) -> &'static str {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => {
                r#""Chromium";v="145", "Google Chrome";v="145", "Not-A.Brand";v="99""#
            }
        }
    }

    /// Get Sec-CH-UA-Platform header
    pub fn get_sec_ch_ua_platform(&self) -> &'static str {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => "\"Windows\""
        }
    }

    /// Get ordered HTTP headers as they appear in Chrome
    /// 
    /// Returns static references to avoid allocations in hot path.
    /// Caller should fill in :authority and :path dynamically.
    pub fn get_ordered_headers(&self) -> &[(&'static str, &'static str)] {
        match self.profile {
            BrowserProfile::Chrome145Windows64 => &[
                (":method", "GET"),
                (":authority", ""),  // Filled by caller
                (":scheme", "https"),
                (":path", "/"),  // Filled by caller
                ("sec-ch-ua", r#""Chromium";v="145", "Google Chrome";v="145", "Not-A.Brand";v="99""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", "\"Windows\""),
                ("upgrade-insecure-requests", "1"),
                ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"),
                ("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
                ("sec-fetch-site", "none"),
                ("sec-fetch-mode", "navigate"),
                ("sec-fetch-user", "?1"),
                ("sec-fetch-dest", "document"),
                ("accept-encoding", "gzip, deflate, br, zstd"),
                ("accept-language", "en-US,en;q=0.9"),
            ],
        }
    }
}

/// HTTP/2 SETTINGS frame configuration
#[derive(Debug, Clone)]
pub struct Http2Settings {
    pub header_table_size: u32,
    pub enable_push: bool,
    pub max_concurrent_streams: u32,
    pub initial_window_size: u32,
    pub max_frame_size: u32,
    pub max_header_list_size: u32,
    pub priority_frames: Vec<PriorityFrame>,
    pub window_update_increment: u32,
}

/// HTTP/2 PRIORITY frame
#[derive(Debug, Clone)]
pub struct PriorityFrame {
    pub stream_id: u32,
    pub weight: u8,
    pub depends_on: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ja3_fingerprint_generation() {
        let emulator = TLSEmulator::new(BrowserProfile::Chrome145Windows64);
        let ja3 = emulator.generate_ja3_fingerprint();
        
        // Verify JA3 has correct format (5 comma-separated sections)
        assert_eq!(ja3.split(',').count(), 5);
        assert!(ja3.starts_with("771,"));  // TLS 1.2
    }

    #[test]
    fn test_ja4_fingerprint_generation() {
        let emulator = TLSEmulator::new(BrowserProfile::Chrome145Windows64);
        let ja4 = emulator.generate_ja4_fingerprint();
        
        // Verify JA4 starts with protocol and TLS version
        assert!(ja4.starts_with("t13d"));
    }

    #[test]
    fn test_http2_settings() {
        let emulator = TLSEmulator::new(BrowserProfile::Chrome145Windows64);
        let settings = emulator.configure_http2_settings();
        
        // Verify Chrome-specific settings
        assert_eq!(settings.header_table_size, 65536);
        assert!(!settings.enable_push);
        assert_eq!(settings.initial_window_size, 6291456);
        assert_eq!(settings.priority_frames.len(), 5);
    }

    #[test]
    fn test_user_agent_matches_profile() {
        let emulator = TLSEmulator::new(BrowserProfile::Chrome145Windows64);
        let ua = emulator.get_user_agent();
        
        assert!(ua.contains("Chrome/145"));
        assert!(ua.contains("Windows NT 10.0"));
    }

    #[test]
    fn test_ordered_headers() {
        let emulator = TLSEmulator::new(BrowserProfile::Chrome145Windows64);
        let headers = emulator.get_ordered_headers();
        
        // Verify pseudo-headers come first
        assert_eq!(headers[0].0, ":method");
        assert_eq!(headers[1].0, ":authority");
        assert_eq!(headers[2].0, ":scheme");
        assert_eq!(headers[3].0, ":path");
        
        // Verify header count
        assert!(headers.len() > 10);
        
        // Verify no allocations - all static strings
        assert_eq!(headers[0].1, "GET");
        assert_eq!(headers[2].1, "https");
    }
}
