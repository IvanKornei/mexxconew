# HFT Arbitrage System

High-frequency trading arbitrage monitoring system for BTC/USDT between Binance Futures and MEXC Futures.

## Features

- **Real-time Price Monitoring**: WebSocket connections to Binance and MEXC Futures
- **Ultra-low Latency**: <1ms data processing with optimized Rust backend
- **Spread Calculation**: Automatic spread calculation with fee consideration
- **Stale Detection**: Identifies outdated data (>300ms)
- **Latency Tracking**: Real-time latency monitoring
- **Modern Dashboard**: Angular 21 with Signals and Tailwind CSS

## Tech Stack

### Backend
- Rust 1.84+
- Tokio (async runtime)
- Axum (WebSocket server)
- tokio-tungstenite (WebSocket client)
- serde (JSON parsing)

### Frontend
- Angular 21
- Signals (state management)
- Tailwind CSS 4.0+
- Zoneless change detection

## Quick Start

### Prerequisites

- Rust 1.84+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 20+ and npm
- Ubuntu/WSL (recommended)

### Backend Setup

1. Clone the repository
```bash
git clone <repo-url>
cd arbitrage-system
```

2. Copy environment template
```bash
cp .env.example .env
# Edit .env with your MEXC API credentials (optional for monitoring)
```

3. Build and run
```bash
# Development
cargo run

# Production (optimized)
cargo build --release
./target/release/arbitrage-system
```

The backend will start on `http://localhost:3000`

### Frontend Setup

```bash
cd frontend
npm install
npm start
```

The frontend will start on `http://localhost:4200`

## Configuration

Edit `config.toml` to customize:

- WebSocket endpoints
- Trading fees
- Stale timeout
- Latency thresholds
- API server settings

## API Endpoints

- `ws://localhost:3000/ws` - WebSocket for real-time price updates
- `http://localhost:3000/health` - Health check endpoint

### WebSocket Message Format

```json
{
  "binance": 50000.0,
  "mexc": 50100.0,
  "spread": 0.2,
  "is_stale": false,
  "latency_ms": 45,
  "binance_timestamp": 1704067200000,
  "mexc_timestamp": 1704067200050,
  "system_timestamp": 1704067200100
}
```

## Project Structure

```
.
├── src/
│   ├── main.rs              # Entry point
│   ├── api/                 # WebSocket server
│   ├── core/                # Core logic (state, spread, normalizer)
│   ├── exchanges/           # Exchange connectors
│   └── utils/               # Utilities (config, errors)
├── frontend/                # Angular application
├── config.toml              # System configuration
├── .env.example             # Environment template
└── Cargo.toml               # Rust dependencies
```

## Development

### Run Tests
```bash
cargo test
```

### Run Benchmarks
```bash
cargo bench
```

### Check Code
```bash
cargo clippy
cargo fmt
```

## Docker Deployment

```bash
docker-compose up -d
```

## Performance

- JSON parsing: <100μs
- Spread calculation: <10μs
- End-to-end latency: <1ms
- WebSocket throughput: >1000 msg/s

## Trading (Future Implementation)

The system includes boilerplate for MEXC trading client with HMAC-SHA256 authentication. To enable trading:

1. Add your API keys to `.env`
2. Implement the TODO methods in `src/exchanges/mexc.rs`
3. Add risk management logic

## License

MIT

## Disclaimer

This software is for educational purposes only. Use at your own risk. Cryptocurrency trading involves substantial risk of loss.
