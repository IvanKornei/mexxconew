---
inclusion: always
---

# Technology Stack

## Backend
- Language: Rust
- Runtime: Tokio (async)
- WebSocket: tokio-tungstenite
- HTTP Client: reqwest
- Web Framework: axum (WebSocket server)
- Serialization: serde with optimized attributes
- Logging: tracing crate with structured logging
- Authentication: Hmac-Sha256 for exchange APIs

## Frontend
- Framework: Angular 21
- State Management: Angular Signals (minimize Zone.js)
- Styling: Tailwind CSS (dark theme, trading terminal style)
- WebSocket: Native WebSocket API

## Key Libraries
- `async-trait` for trait-based async methods
- `thiserror` for error handling
- `tokio::sync::watch` or `Arc<RwLock>` for shared state
- `criterion` for performance benchmarking

## Common Commands

### Rust Backend
```bash
# Build
cargo build --release

# Run
cargo run --release

# Test
cargo test

# Benchmark
cargo bench
```

### Angular Frontend
```bash
# Install dependencies
npm install

# Development server
npm start

# Build
npm run build

# Test
npm test
```

## Configuration
- API keys: Load from `.env` file (MEXC_API_KEY, MEXC_API_SECRET)
- System config: `config.toml` for endpoints, fees, thresholds
- Logs: Rotate at 100MB, retain 30 days
