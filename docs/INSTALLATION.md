# Installation Guide

## System Requirements

### Minimum Requirements
- CPU: 2 cores
- RAM: 4 GB
- Storage: 20 GB (for Bitcoin node data)
- OS: Linux, macOS, or Windows with WSL2

### Recommended Requirements
- CPU: 4+ cores
- RAM: 8+ GB
- Storage: 100+ GB SSD
- OS: Ubuntu 22.04 LTS or macOS 13+

## Prerequisites

### 1. Install Rust

```bash
# Install rustup (Rust installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add Rust to PATH
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### 2. Install Bitcoin Core

#### macOS
```bash
# Using Homebrew
brew install bitcoin

# Or download from bitcoin.org
wget https://bitcoin.org/bin/bitcoin-core-25.0/bitcoin-25.0-osx64.tar.gz
tar -xzf bitcoin-25.0-osx64.tar.gz
```

#### Linux (Ubuntu/Debian)
```bash
# Add Bitcoin PPA
sudo add-apt-repository ppa:bitcoin/bitcoin
sudo apt-get update

# Install Bitcoin Core
sudo apt-get install bitcoind bitcoin-qt

# Or download binary
wget https://bitcoin.org/bin/bitcoin-core-25.0/bitcoin-25.0-x86_64-linux-gnu.tar.gz
tar -xzf bitcoin-25.0-x86_64-linux-gnu.tar.gz
```

### 3. Configure Bitcoin Core

Create `~/.bitcoin/bitcoin.conf`:

```conf
# Network
testnet=1  # Use testnet for development

# RPC Settings
server=1
rpcuser=bitstable_rpc
rpcpassword=your_secure_password_here
rpcport=18332

# Performance
dbcache=1000
maxconnections=20

# Indexing (required for BitStable)
txindex=1
```

Start Bitcoin Core:
```bash
# Start in daemon mode
bitcoind -daemon

# Check sync status
bitcoin-cli -testnet getblockchaininfo
```

### 4. Install Development Tools

```bash
# macOS
brew install git make gcc

# Linux
sudo apt-get install build-essential git
```

## BitStable Installation

### 1. Clone Repository

```bash
# Clone the repository
git clone https://github.com/yourusername/bitstable.git
cd bitstable
```

### 2. Install Dependencies

```bash
# Install Rust dependencies
cargo fetch

# Optional: Install development tools
cargo install cargo-watch cargo-tarpaulin cargo-audit
```

### 3. Configure Environment

Create `.env` file:

```bash
cp .env.example .env
```

Edit `.env` with your configuration:

```env
# Bitcoin RPC (must match bitcoin.conf)
BITCOIN_RPC_URL=http://localhost:18332
BITCOIN_RPC_USER=bitstable_rpc
BITCOIN_RPC_PASS=your_secure_password_here

# Database
DATABASE_PATH=./data/bitstable.db
DATABASE_BACKUP_PATH=./data/backups

# Oracle Settings
ORACLE_UPDATE_INTERVAL=60
ORACLE_THRESHOLD=2
ORACLE_SOURCES=binance,coinbase,kraken

# Application
RUST_LOG=info
API_PORT=8080
API_HOST=127.0.0.1

# Network
NETWORK=testnet
```

### 4. Build BitStable

```bash
# Development build
cargo build

# Production build (optimized)
cargo build --release
```

### 5. Initialize Database

```bash
# Create data directories
mkdir -p data/backups

# Initialize database (first run)
cargo run --bin init-db
```

### 6. Run Tests

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_vault_creation
```

## Running BitStable

### Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run with auto-reload on code changes
cargo watch -x run
```

### Production Mode

```bash
# Run optimized binary
./target/release/bitstable

# Or with cargo
cargo run --release
```

### Using systemd (Linux)

Create `/etc/systemd/system/bitstable.service`:

```ini
[Unit]
Description=BitStable Stablecoin Protocol
After=network.target bitcoind.service

[Service]
Type=simple
User=bitstable
Group=bitstable
WorkingDirectory=/opt/bitstable
Environment="RUST_LOG=info"
ExecStart=/opt/bitstable/target/release/bitstable
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable bitstable
sudo systemctl start bitstable
sudo systemctl status bitstable
```

## Docker Installation

### Build Docker Image

```bash
# Build image
docker build -t bitstable:latest .

# Run container
docker run -d \
  --name bitstable \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  --env-file .env \
  bitstable:latest
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  bitcoind:
    image: ruimarinho/bitcoin-core:latest
    command: [
      "-testnet",
      "-server",
      "-rpcuser=bitstable_rpc",
      "-rpcpassword=your_secure_password",
      "-rpcallowip=172.0.0.0/8",
      "-txindex=1"
    ]
    volumes:
      - bitcoin-data:/home/bitcoin/.bitcoin
    ports:
      - "18332:18332"

  bitstable:
    build: .
    depends_on:
      - bitcoind
    environment:
      - BITCOIN_RPC_URL=http://bitcoind:18332
      - BITCOIN_RPC_USER=bitstable_rpc
      - BITCOIN_RPC_PASS=your_secure_password
    volumes:
      - ./data:/app/data
    ports:
      - "8080:8080"

volumes:
  bitcoin-data:
```

Run with Docker Compose:
```bash
docker-compose up -d
```

## Verification

### 1. Check Service Health

```bash
# Check API health
curl http://localhost:8080/health

# Expected response
{"status":"healthy","bitcoin_connected":true,"database_connected":true}
```

### 2. Check Bitcoin Connection

```bash
# Get blockchain info via BitStable
curl http://localhost:8080/api/bitcoin/info
```

### 3. Check Oracle Status

```bash
# Get current BTC price
curl http://localhost:8080/api/oracle/price
```

## Troubleshooting

### Common Issues

#### Bitcoin RPC Connection Failed
```bash
# Check Bitcoin is running
bitcoin-cli -testnet ping

# Check RPC credentials match
grep rpc ~/.bitcoin/bitcoin.conf
grep BITCOIN_RPC .env
```

#### Database Errors
```bash
# Reset database
rm -rf ./data/bitstable.db
cargo run --bin init-db
```

#### Port Already in Use
```bash
# Find process using port
lsof -i :8080

# Change port in .env
API_PORT=8081
```

### Logging

Adjust log levels in `.env`:
```env
# Log levels: trace, debug, info, warn, error
RUST_LOG=bitstable=debug,bitcoin_rpc=trace
```

## Updating

```bash
# Pull latest changes
git pull origin main

# Update dependencies
cargo update

# Rebuild
cargo build --release

# Run migrations (if any)
cargo run --bin migrate
```

## Uninstallation

```bash
# Stop service
sudo systemctl stop bitstable

# Remove service
sudo systemctl disable bitstable
sudo rm /etc/systemd/system/bitstable.service

# Remove data (optional - backup first!)
rm -rf ./data

# Remove project
cd ..
rm -rf bitstable
```

## Next Steps

- Read the [User Guide](./USAGE.md) to learn how to use BitStable
- Check [API Documentation](./API.md) for integration
- Join our [Discord](https://discord.gg/bitstable) for support
