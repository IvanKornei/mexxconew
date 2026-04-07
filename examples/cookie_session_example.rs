/// Example: Initialize MEXC session using browser cookies
///
/// This example demonstrates three methods of session initialization:
/// 1. Auto-extract from browser (recommended)
/// 2. Manual cookie input
/// 3. Full browser automation (fallback)
///
/// Run with:
///   cargo run --example cookie_session_example

use arbitrage_system::emulation::{
    config::EmulationConfig,
    session::SessionInitializer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🎭 MEXC Session Initialization Example\n");

    // Load configuration
    let config = EmulationConfig::default();
    let mut initializer = SessionInitializer::new(config.clone())?;

    // Method 1: Auto-extract from browser (RECOMMENDED)
    println!("📌 Method 1: Auto-extract from browser");
    println!("─────────────────────────────────────\n");
    
    match initializer.initialize_from_browser_cookies().await {
        Ok(session) => {
            println!("✅ Session initialized successfully!");
            println!("   Cookies: {}", session.cookies.len());
            println!("   Valid until: {}", session.expires_at);
            println!("   User-Agent: {}", session.user_agent);
            
            // Display cookie names (not values for security)
            println!("\n   Cookie names:");
            for cookie in &session.cookies {
                println!("     - {} (domain: {})", cookie.name, cookie.domain);
            }
            
            return Ok(());
        }
        Err(e) => {
            println!("⚠️  Auto-extraction failed: {}", e);
            println!("   Trying alternative methods...\n");
        }
    }

    // Method 2: Manual cookie input from environment
    println!("📌 Method 2: Manual cookie input");
    println!("─────────────────────────────────────\n");
    
    if let Ok(cookie_string) = std::env::var(&config.cookies.manual_cookies_env) {
        println!("Found cookies in environment variable: {}", config.cookies.manual_cookies_env);
        
        match initializer.initialize_from_manual_cookies(&cookie_string).await {
            Ok(session) => {
                println!("✅ Session initialized from manual cookies!");
                println!("   Cookies: {}", session.cookies.len());
                return Ok(());
            }
            Err(e) => {
                println!("⚠️  Manual cookie initialization failed: {}", e);
                println!("   Trying full browser automation...\n");
            }
        }
    } else {
        println!("No manual cookies found in environment");
        println!("Set {} to use manual cookies\n", config.cookies.manual_cookies_env);
    }

    // Method 3: Full browser automation (slowest, most reliable)
    println!("📌 Method 3: Full browser automation");
    println!("─────────────────────────────────────\n");
    println!("This will:");
    println!("  1. Launch Chrome browser");
    println!("  2. Navigate to MEXC and login");
    println!("  3. Simulate human behavior");
    println!("  4. Extract session cookies");
    println!("  Expected time: 15-30 seconds\n");

    match initializer.initialize_session().await {
        Ok(session) => {
            println!("✅ Session initialized via browser automation!");
            println!("   Cookies: {}", session.cookies.len());
            println!("   Valid until: {}", session.expires_at);
            
            // Save session for future use
            println!("\n💾 Session saved to: {}", config.session_storage_path.display());
            println!("   Next time it will load instantly from cache!");
            
            Ok(())
        }
        Err(e) => {
            println!("❌ All initialization methods failed!");
            println!("   Error: {}", e);
            println!("\n📖 Troubleshooting:");
            println!("   1. Make sure you're logged into MEXC in Chrome/Edge/Firefox");
            println!("   2. Close your browser and try auto-extraction again");
            println!("   3. Or manually copy cookies from DevTools");
            println!("   4. See COOKIE_EXTRACTION_GUIDE.md for detailed instructions");
            
            Err(e.into())
        }
    }
}
