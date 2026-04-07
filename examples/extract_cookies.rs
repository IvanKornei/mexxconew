/// CLI tool for extracting MEXC cookies from browser
///
/// Usage:
///   cargo run --example extract_cookies
///
/// This will:
/// 1. Auto-detect your browser (Chrome/Edge/Firefox)
/// 2. Extract MEXC cookies from the browser database
/// 3. Display the cookies in a format you can use
///
/// Requirements:
/// - You must be logged into MEXC in your browser
/// - Browser should be closed for best results (to avoid database locks)

use arbitrage_system::emulation::browser_cookies::{BrowserCookieExtractor, Browser};

fn main() {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🍪 MEXC Cookie Extractor\n");
    println!("This tool will extract MEXC cookies from your browser.");
    println!("Make sure you're logged into MEXC in your browser.\n");

    // Try auto-detection
    println!("🔍 Auto-detecting browser...");
    match BrowserCookieExtractor::auto_detect() {
        Ok(extractor) => {
            println!("✅ Browser detected!\n");
            
            match extractor.extract_mexc_cookies() {
                Ok(cookies) => {
                    if cookies.is_empty() {
                        println!("❌ No MEXC cookies found!");
                        println!("\nPlease make sure:");
                        println!("  1. You're logged into MEXC in your browser");
                        println!("  2. You've visited futures.mexc.com");
                        println!("  3. Your browser is closed (to avoid database locks)");
                        return;
                    }

                    println!("✅ Found {} cookies:\n", cookies.len());
                    
                    // Display cookies
                    for cookie in &cookies {
                        println!("  📌 {}", cookie.name);
                        println!("     Domain: {}", cookie.domain);
                        println!("     Value: {}...", 
                            if cookie.value.len() > 40 {
                                &cookie.value[..40]
                            } else {
                                &cookie.value
                            }
                        );
                        println!("     Secure: {}, HttpOnly: {}", cookie.secure, cookie.http_only);
                        if let Some(expires) = cookie.expires {
                            println!("     Expires: {}", expires);
                        }
                        println!();
                    }

                    // Generate cookie string for manual input
                    println!("\n📋 Cookie string for manual input:");
                    println!("─────────────────────────────────────");
                    let cookie_string: Vec<String> = cookies
                        .iter()
                        .map(|c| format!("{}={}", c.name, c.value))
                        .collect();
                    println!("{}", cookie_string.join("; "));
                    println!("─────────────────────────────────────\n");

                    // Generate JSON format
                    println!("📋 JSON format:");
                    println!("─────────────────────────────────────");
                    match serde_json::to_string_pretty(&cookies) {
                        Ok(json) => println!("{}", json),
                        Err(e) => println!("Error serializing to JSON: {}", e),
                    }
                    println!("─────────────────────────────────────\n");

                    println!("✅ Cookies extracted successfully!");
                    println!("\nYou can now use these cookies with the trading system.");
                    println!("\nTo use them, add to your .env file:");
                    println!("MEXC_COOKIES=\"{}\"", cookie_string.join("; "));
                }
                Err(e) => {
                    println!("❌ Failed to extract cookies: {}", e);
                    println!("\nTroubleshooting:");
                    println!("  1. Close your browser completely");
                    println!("  2. Make sure you're logged into MEXC");
                    println!("  3. Try running as administrator (Windows)");
                }
            }
        }
        Err(e) => {
            println!("❌ Could not detect browser: {}", e);
            println!("\nSupported browsers:");
            println!("  - Google Chrome");
            println!("  - Microsoft Edge");
            println!("  - Mozilla Firefox");
            println!("\nManual extraction:");
            println!("  1. Open MEXC Futures in your browser");
            println!("  2. Press F12 to open DevTools");
            println!("  3. Go to Application -> Cookies -> futures.mexc.com");
            println!("  4. Copy all cookie values");
        }
    }

    // Try specific browsers if auto-detection failed
    println!("\n🔍 Trying specific browsers...\n");
    
    for browser in [Browser::Chrome, Browser::Edge, Browser::Firefox] {
        let extractor = BrowserCookieExtractor::new(browser);
        match extractor.extract_mexc_cookies() {
            Ok(cookies) if !cookies.is_empty() => {
                println!("✅ Found {} cookies in {:?}", cookies.len(), browser);
            }
            Ok(_) => {
                println!("⚠️  {:?}: No MEXC cookies found", browser);
            }
            Err(e) => {
                println!("❌ {:?}: {}", browser, e);
            }
        }
    }
}
