# Setup Instructions for Ubuntu WSL

## Prerequisites

### 1. Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version  # Should show 1.84+
```

### 2. Install Node.js and npm
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
node --version  # Should show v20+
npm --version
```

### 3. Install Angular CLI
```bash
npm install -g @angular/cli@21
ng version
```

## Quick Start (Development)

### 1. Clone and Setup
```bash
git clone <your-repo>
cd arbitrage-system
```

### 2. Configure Environment
```bash
# Copy environment template
cp .env.example .env

# Edit .env with your MEXC API credentials (optional for monitoring only)
nano .env
```

### 3. Start Backend
```bash
# Terminal 1: Run Rust backend
cargo run --release

# The backend will start on http://localhost:3000
# WebSocket endpoint: ws://localhost:3000/ws
# Health check: http://localhost:3000/health
```

### 4. Start Frontend
```bash
# Terminal 2: Run Angular frontend
cd frontend
npm install
npm start

# The frontend will start on http://localhost:4200
# Open browser: http://localhost:4200
```

## Docker Deployment (Recommended for Production)

### 1. Install Docker and Docker Compose
```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo apt-get install docker-compose-plugin

# Logout and login again for group changes
```

### 2. Build and Run
```bash
# Create .env file
cp .env.example .env

# Build and start services
docker-compose up -d

# Check logs
docker-compose logs -f

# Stop services
docker-compose down
```

### 3. Access Services
- Backend: http://localhost:3000
- Frontend: http://localhost:4200
- Health Check: http://localhost:3000/health

## Verification

### 1. Check Backend
```bash
# Health check
curl http://localhost:3000/health

# Should return: OK
```

### 2. Check WebSocket
```bash
# Install wscat for testing
npm install -g wscat

# Connect to WebSocket
wscat -c ws://localhost:3000/ws

# You should see JSON messages with price data
```

### 3. Check Frontend
Open browser to http://localhost:4200

You should see:
- Binance Futures price card
- MEXC Futures price card
- Spread visualizer with latency
- Real-time updates

## Troubleshooting

### Backend Issues

**Problem**: Connection refused to exchanges
```bash
# Check internet connection
ping binance.com

# Check if ports are available
sudo netstat -tulpn | grep 3000
```

**Problem**: Compilation errors
```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Frontend Issues

**Problem**: npm install fails
```bash
# Clear npm cache
npm cache clean --force

# Remove node_modules and reinstall
rm -rf node_modules package-lock.json
npm install
```

**Problem**: WebSocket connection fails
- Check if backend is running on port 3000
- Check browser console for errors
- Verify WebSocket URL in `market-data.service.ts`

### Docker Issues

**Problem**: Permission denied
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Logout and login again
```

**Problem**: Port already in use
```bash
# Check what's using the port
sudo lsof -i :3000
sudo lsof -i :4200

# Kill the process or change ports in config
```

## Performance Tuning

### For WSL
```bash
# Add to ~/.bashrc for better performance
export CARGO_BUILD_JOBS=4
export MAKEFLAGS="-j4"

# Increase file watchers for Angular
echo fs.inotify.max_user_watches=524288 | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

### For Production
```bash
# Build optimized backend
cargo build --release

# Build optimized frontend
cd frontend
npm run build

# Serve with nginx or use Docker
```

## Development Workflow

### Backend Development
```bash
# Run with auto-reload (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check code
cargo clippy
cargo fmt
```

### Frontend Development
```bash
cd frontend

# Development server with hot reload
npm start

# Run tests
npm test

# Build for production
npm run build
```

## Monitoring

### Logs
```bash
# Backend logs (if using systemd)
journalctl -u arbitrage-system -f

# Docker logs
docker-compose logs -f backend
docker-compose logs -f frontend

# File logs
tail -f logs/arbitrage-system.log
```

### Metrics
- Check latency in dashboard
- Monitor WebSocket connection status
- Watch for "STALE" indicators
- Check spread calculations

## Next Steps

1. Monitor the system for a few hours
2. Verify spread calculations are accurate
3. Check latency metrics
4. Implement trading logic (if needed)
5. Add alerting for opportunities
6. Set up production monitoring

## Support

For issues or questions:
1. Check logs for error messages
2. Verify configuration in `config.toml`
3. Test WebSocket connection manually
4. Check system resources (CPU, memory, network)

## Security Notes

- Never commit `.env` file to git
- Keep API keys secure
- Use read-only API keys for monitoring
- Enable 2FA on exchange accounts
- Monitor for unusual activity
