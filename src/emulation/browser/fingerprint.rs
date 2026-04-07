/// Browser fingerprint generation and management
/// 
/// Generates consistent, realistic browser fingerprints that match
/// real Chrome installations on Windows.

use serde::{Deserialize, Serialize};

/// Browser fingerprint data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserFingerprint {
    pub user_agent: String,
    pub platform: String,
    pub vendor: String,
    pub languages: Vec<String>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub color_depth: u8,
    pub pixel_ratio: f32,
    pub timezone: String,
    pub hardware_concurrency: u8,
    pub device_memory: u8,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    pub canvas_fingerprint: String,
    pub audio_fingerprint: String,
}

impl Default for BrowserFingerprint {
    fn default() -> Self {
        Self::chrome_145_windows()
    }
}

impl BrowserFingerprint {
    /// Generate Chrome 145 on Windows 10 fingerprint
    pub fn chrome_145_windows() -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36".to_string(),
            platform: "Win32".to_string(),
            vendor: "Google Inc.".to_string(),
            languages: vec![
                "ru-RU".to_string(),
                "ru".to_string(),
                "en-US".to_string(),
                "en".to_string(),
            ],
            screen_width: 1920,
            screen_height: 1080,
            color_depth: 24,
            pixel_ratio: 1.0,
            timezone: "Europe/Moscow".to_string(),
            hardware_concurrency: 8,
            device_memory: 8,
            webgl_vendor: "Google Inc. (NVIDIA)".to_string(),
            webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)".to_string(),
            canvas_fingerprint: Self::generate_canvas_fingerprint(),
            audio_fingerprint: Self::generate_audio_fingerprint(),
        }
    }
    
    /// Generate consistent canvas fingerprint
    fn generate_canvas_fingerprint() -> String {
        // This would be a real canvas fingerprint in production
        // For now, use a realistic hash
        "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6".to_string()
    }
    
    /// Generate consistent audio fingerprint
    fn generate_audio_fingerprint() -> String {
        // This would be a real audio context fingerprint
        "124.04347527516074".to_string()
    }
    
    /// Get JavaScript code to apply this fingerprint
    pub fn to_js_injection(&self) -> String {
        format!(
            r#"
            // Apply browser fingerprint
            (function() {{
                // User Agent
                Object.defineProperty(navigator, 'userAgent', {{
                    get: () => '{}'
                }});
                
                // Platform
                Object.defineProperty(navigator, 'platform', {{
                    get: () => '{}'
                }});
                
                // Vendor
                Object.defineProperty(navigator, 'vendor', {{
                    get: () => '{}'
                }});
                
                // Languages
                Object.defineProperty(navigator, 'languages', {{
                    get: () => {}
                }});
                
                // Hardware Concurrency
                Object.defineProperty(navigator, 'hardwareConcurrency', {{
                    get: () => {}
                }});
                
                // Device Memory
                Object.defineProperty(navigator, 'deviceMemory', {{
                    get: () => {}
                }});
                
                // Screen
                Object.defineProperty(screen, 'width', {{
                    get: () => {}
                }});
                Object.defineProperty(screen, 'height', {{
                    get: () => {}
                }});
                Object.defineProperty(screen, 'colorDepth', {{
                    get: () => {}
                }});
                Object.defineProperty(screen, 'pixelDepth', {{
                    get: () => {}
                }});
                
                // WebGL
                const getParameter = WebGLRenderingContext.prototype.getParameter;
                WebGLRenderingContext.prototype.getParameter = function(parameter) {{
                    if (parameter === 37445) {{
                        return '{}';
                    }}
                    if (parameter === 37446) {{
                        return '{}';
                    }}
                    return getParameter.call(this, parameter);
                }};
                
                // Canvas fingerprint
                const toDataURL = HTMLCanvasElement.prototype.toDataURL;
                HTMLCanvasElement.prototype.toDataURL = function() {{
                    // Add consistent noise to canvas
                    const context = this.getContext('2d');
                    if (context) {{
                        const imageData = context.getImageData(0, 0, this.width, this.height);
                        // Modify pixels slightly but consistently
                        for (let i = 0; i < imageData.data.length; i += 4) {{
                            imageData.data[i] = (imageData.data[i] + 1) % 256;
                        }}
                        context.putImageData(imageData, 0, 0);
                    }}
                    return toDataURL.apply(this, arguments);
                }};
                
                console.log('🎭 Fingerprint applied: Chrome 145 Windows');
            }})();
            "#,
            self.user_agent,
            self.platform,
            self.vendor,
            serde_json::to_string(&self.languages).unwrap(),
            self.hardware_concurrency,
            self.device_memory,
            self.screen_width,
            self.screen_height,
            self.color_depth,
            self.color_depth,
            self.webgl_vendor,
            self.webgl_renderer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fingerprint_generation() {
        let fp = BrowserFingerprint::chrome_145_windows();
        
        assert_eq!(fp.platform, "Win32");
        assert_eq!(fp.hardware_concurrency, 8);
        assert_eq!(fp.screen_width, 1920);
        assert!(fp.user_agent.contains("Chrome/145"));
    }
    
    #[test]
    fn test_js_injection() {
        let fp = BrowserFingerprint::chrome_145_windows();
        let js = fp.to_js_injection();
        
        assert!(js.contains("Chrome/145"));
        assert!(js.contains("Win32"));
        assert!(js.contains("Google Inc."));
    }
}
