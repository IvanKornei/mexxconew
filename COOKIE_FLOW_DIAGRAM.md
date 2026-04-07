# Cookie Extraction System - Flow Diagram

## Общая архитектура

```
┌─────────────────────────────────────────────────────────────────┐
│                     MEXC Trading System                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Session Initialization                         │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Method 1   │  │   Method 2   │  │   Method 3   │          │
│  │ Auto-Extract │  │    Manual    │  │   Browser    │          │
│  │  (< 1 sec)   │  │  (instant)   │  │ Automation   │          │
│  │              │  │              │  │  (15-30 sec) │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                    │
│         └─────────────────┴─────────────────┘                    │
│                           │                                       │
└───────────────────────────┼───────────────────────────────────────┘
                            ▼
                   ┌─────────────────┐
                   │  Session Data   │
                   │  - Cookies      │
                   │  - Auth Token   │
                   │  - TLS Ticket   │
                   └─────────────────┘
```

## Method 1: Auto-Extract (Рекомендуется)

```
┌──────────────┐
│   Browser    │  User logged in to MEXC
│  (Chrome/    │  Session active
│  Edge/       │
│  Firefox)    │
└──────┬───────┘
       │
       │ Cookie Database
       │ (SQLite)
       ▼
┌──────────────────────────────────────┐
│  BrowserCookieExtractor              │
│                                      │
│  1. Auto-detect browser              │
│     - Check Chrome path              │
│     - Check Edge path                │
│     - Check Firefox path             │
│                                      │
│  2. Copy database to temp            │
│     (avoid locking issues)           │
│                                      │
│  3. Query MEXC cookies               │
│     SELECT * FROM cookies            │
│     WHERE host LIKE '%mexc.com%'     │
│                                      │
│  4. Parse and return                 │
└──────────────┬───────────────────────┘
               │
               ▼
       ┌───────────────┐
       │  Cookie List  │
       │  - session_id │
       │  - auth_token │
       │  - token      │
       │  - _mexc_*    │
       └───────────────┘
```

## Method 2: Manual Input

```
┌──────────────┐
│   Browser    │
│  DevTools    │
│  (F12)       │
└──────┬───────┘
       │
       │ User copies cookies
       │ from Application tab
       ▼
┌──────────────────────────────────────┐
│  Manual Cookie Input                 │
│                                      │
│  Format 1: String                    │
│  "name1=value1; name2=value2"        │
│                                      │
│  Format 2: JSON                      │
│  [{"name": "...", "value": "..."}]   │
│                                      │
│  Parse and validate                  │
└──────────────┬───────────────────────┘
               │
               ▼
       ┌───────────────┐
       │  Cookie List  │
       └───────────────┘
```

## Method 3: Browser Automation (Fallback)

```
┌──────────────────────────────────────┐
│  SessionInitializer                  │
│                                      │
│  1. Launch Chrome with profile       │
│     - TLS fingerprints               │
│     - User-Agent spoofing            │
│                                      │
│  2. Navigate to MEXC login           │
│     - Simulate typing                │
│     - Human-like delays              │
│                                      │
│  3. Login with credentials           │
│     - Type username (100-300ms/char) │
│     - Type password                  │
│     - Click login button             │
│                                      │
│  4. Navigate to Futures              │
│     - View charts                    │
│     - Check balance                  │
│     - Setup parameters               │
│                                      │
│  5. Extract cookies                  │
│     - From browser session           │
│     - TLS session ticket             │
└──────────────┬───────────────────────┘
               │
               ▼
       ┌───────────────┐
       │  Session Data │
       └───────────────┘
```

## Cookie Storage & Encryption

```
┌───────────────┐
│  Session Data │
└───────┬───────┘
        │
        ▼
┌──────────────────────────────────────┐
│  SessionPersistence                  │
│                                      │
│  1. Serialize to JSON                │
│     {                                │
│       "cookies": [...],              │
│       "auth_token": "...",           │
│       "expires_at": "..."            │
│     }                                │
│                                      │
│  2. Encrypt with AES-256-GCM         │
│     - Key from env: SESSION_KEY      │
│     - Random nonce                   │
│                                      │
│  3. Save to file                     │
│     .kiro/emulation/session.enc      │
└──────────────────────────────────────┘
```

## Auto-Refresh Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Trading System Running                                      │
└─────────────────────────────────────────────────────────────┘
                    │
                    │ Every 30 minutes
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Cookie Refresh Task                                         │
│                                                              │
│  1. Check session expiry                                     │
│     if expires_in < 30 minutes:                              │
│                                                              │
│  2. Try auto-extract from browser                            │
│     - User still logged in?                                  │
│     - Extract fresh cookies                                  │
│                                                              │
│  3. Update session                                           │
│     - Replace old cookies                                    │
│     - Update expiry time                                     │
│     - Save encrypted                                         │
│                                                              │
│  4. Continue trading                                         │
│     - No interruption                                        │
│     - Seamless transition                                    │
└─────────────────────────────────────────────────────────────┘
```

## Error Handling & Fallbacks

```
┌──────────────────────┐
│  Initialize Session  │
└──────────┬───────────┘
           │
           ▼
    ┌──────────────┐
    │ Try Method 1 │ ──────────┐
    │ Auto-Extract │           │ Success
    └──────┬───────┘           │
           │ Fail              ▼
           ▼              ┌─────────────┐
    ┌──────────────┐     │   Session   │
    │ Try Method 2 │ ────┤    Ready    │
    │   Manual     │     │             │
    └──────┬───────┘     └─────────────┘
           │ Fail              ▲
           ▼                   │
    ┌──────────────┐           │
    │ Try Method 3 │ ──────────┘
    │  Automation  │
    └──────┬───────┘
           │ Fail
           ▼
    ┌──────────────┐
    │    Error     │
    │  Show Guide  │
    └──────────────┘
```

## CLI Tool Flow

```
$ cargo run --example extract_cookies

┌─────────────────────────────────────┐
│  🍪 MEXC Cookie Extractor           │
└─────────────────────────────────────┘
           │
           ▼
    ┌──────────────┐
    │ Auto-detect  │
    │   Browser    │
    └──────┬───────┘
           │
           ├─ Chrome found ──┐
           ├─ Edge found ────┤
           └─ Firefox found ─┤
                             │
                             ▼
                    ┌─────────────────┐
                    │ Extract Cookies │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  Display Output │
                    │                 │
                    │  📌 Cookie 1    │
                    │  📌 Cookie 2    │
                    │  📌 Cookie 3    │
                    │                 │
                    │  📋 String:     │
                    │  "name=value;..." │
                    │                 │
                    │  📋 JSON:       │
                    │  [{...}]        │
                    └─────────────────┘
```

## Integration with Trading System

```
┌─────────────────────────────────────────────────────────────┐
│  main.rs                                                     │
│                                                              │
│  1. Load config from config.toml                             │
│     [emulation.cookies]                                      │
│     auto_extract_on_startup = true                           │
│                                                              │
│  2. Initialize session                                       │
│     let session = initializer                                │
│         .initialize_from_browser_cookies()                   │
│         .await?;                                             │
│                                                              │
│  3. Create MEXC client with session                          │
│     let mexc = MexcEmulator::new(session)?;                  │
│                                                              │
│  4. Start trading                                            │
│     - WebSocket with cookies                                 │
│     - HTTP requests with auth token                          │
│     - TLS with session ticket                                │
│                                                              │
│  5. Background refresh task                                  │
│     tokio::spawn(async move {                                │
│         loop {                                               │
│             sleep(30 minutes).await;                         │
│             refresh_cookies().await;                         │
│         }                                                    │
│     });                                                      │
└─────────────────────────────────────────────────────────────┘
```

## Security Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Environment Variables                              │
│  - EMULATION_SESSION_KEY (64 hex chars)                      │
│  - MEXC_COOKIES (optional manual input)                      │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: File Encryption                                    │
│  - AES-256-GCM encryption                                    │
│  - Random nonce per save                                     │
│  - Authenticated encryption                                  │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: File Permissions                                   │
│  - chmod 600 .env                                            │
│  - chmod 600 session.enc                                     │
│  - .gitignore for sensitive files                            │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 4: Runtime Security                                   │
│  - Cookies never logged                                      │
│  - Memory cleared on drop                                    │
│  - No network transmission of raw cookies                    │
└─────────────────────────────────────────────────────────────┘
```

## Performance Metrics

```
Method 1: Auto-Extract
├─ Browser detection: ~10ms
├─ Database copy: ~50ms
├─ Cookie query: ~100ms
├─ Parsing: ~10ms
└─ Total: ~170ms ✅

Method 2: Manual Input
├─ String parsing: ~5ms
├─ Validation: ~5ms
└─ Total: ~10ms ✅✅

Method 3: Browser Automation
├─ Browser launch: ~2000ms
├─ Navigation: ~3000ms
├─ Login: ~5000ms
├─ Trading setup: ~8000ms
├─ Cookie extraction: ~100ms
└─ Total: ~18000ms ⚠️

Session Save/Load
├─ Serialize: ~10ms
├─ Encrypt: ~20ms
├─ Write file: ~20ms
├─ Read file: ~10ms
├─ Decrypt: ~20ms
├─ Deserialize: ~10ms
└─ Total: ~90ms ✅
```
