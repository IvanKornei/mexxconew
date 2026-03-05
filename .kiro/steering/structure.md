---
inclusion: always
---

# Project Structure

## Rust Backend (Monorepo Root)
```
src/
├── main.rs                    # Entry point, Tokio runtime setup
├── exchanges/
│   ├── mod.rs
│   ├── binance.rs            # Binance Futures connector
│   ├── mexc.rs               # MEXC Futures connector
│   └── client.rs             # ExchangeClient trait
├── core/
│   ├── mod.rs
│   ├── price_feed.rs         # PriceFeedManager
│   ├── normalizer.rs         # Symbol normalization
│   ├── spread.rs             # SpreadCalculator
│   └── state.rs              # PriceState (shared state)
├── api/
│   ├── mod.rs
│   └── websocket.rs          # Axum WebSocket server
└── utils/
    ├── mod.rs
    ├── errors.rs             # Error types
    └── config.rs             # Configuration loading
```

## Angular Frontend
```
frontend/
├── src/
│   ├── app/
│   │   ├── services/
│   │   │   └── market-data.service.ts    # WebSocket client
│   │   ├── components/
│   │   │   ├── price-card/               # PriceCardComponent
│   │   │   └── spread-visualizer/        # SpreadVisualizerComponent
│   │   └── app.component.ts
│   └── styles.css                        # Tailwind imports
└── tailwind.config.js
```

## Configuration Files
```
.env                          # API keys (gitignored)
config.toml                   # System configuration
Cargo.toml                    # Rust dependencies
package.json                  # Angular dependencies
```

## Architecture Principles
- Modular structure with clear separation of concerns
- Exchange layer isolated from core logic
- Trait-based design for extensibility
- Async/await throughout with Tokio
- Lock-free or minimal locking for performance
