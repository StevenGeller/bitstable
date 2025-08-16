# BitStable End-to-End Setup Guide

This guide walks you through setting up and running the complete BitStable system from scratch.

## 🎯 Overview

BitStable is a multi-currency stablecoin protocol built on Bitcoin with:
- **Progressive liquidation system** (25%/50%/75%/100% based on collateral ratios)  
- **Multi-currency support** (USD, EUR, GBP, JPY, NGN, MXN)
- **Bonded oracle network** with circuit breakers
- **Proof-of-reserves** with Merkle tree commitments
- **Automated liquidation bots**

## 📋 Prerequisites

### System Requirements
- **Rust**: 1.70+ (with cargo)
- **Bitcoin Core**: 25.0+ (for mainnet/testnet)
- **OS**: Linux, macOS, or Windows
- **Memory**: 4GB+ RAM
- **Storage**: 10GB+ available space

### Optional (for production)
- **PostgreSQL**: For persistent data storage
- **Redis**: For caching and pub/sub
- **Prometheus**: For monitoring
- **Docker**: For containerized deployment

## 🚀 Quick Start (Testnet)

### 1. Build the System
```bash
# Clone and build (if not already done)
git clone git@github.com:StevenGeller/bitstable.git
cd bitstable
cargo build --release

# Verify build
ls target/release/
# Should show: bitstable-cli, oracle-node, liquidator-bot
```

### 2. Start with Testnet (No Bitcoin Core Required)
```bash
# Check system status
./target/release/bitstable-cli status

# View available commands
./target/release/bitstable-cli --help
```

### 3. Run Oracle Network (Simulated)
```bash
# Terminal 1: Start oracle node
./target/release/oracle-node --network testnet --update-interval 10 --verbose

# Terminal 2: Check oracle status
./target/release/bitstable-cli oracle status
./target/release/bitstable-cli oracle price
```

### 4. Create Your First Vault
```bash
# Generate a test public key (use any valid Bitcoin pubkey)
TEST_PUBKEY="0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

# Create vault with 1 BTC collateral, mint 30,000 USD
./target/release/bitstable-cli vault create \
  --collateral-btc 1.0 \
  --stable-amount 30000 \
  --owner $TEST_PUBKEY

# List vaults
./target/release/bitstable-cli vault list
```

### 5. Run Liquidator Bot
```bash
# Terminal 3: Start liquidation monitoring
./target/release/liquidator-bot \
  --liquidator-key $TEST_PUBKEY \
  --dry-run \
  --scan-interval 30 \
  --verbose
```

## 🏗️ Full Production Setup

### Step 1: Bitcoin Core Setup

#### Install Bitcoin Core
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install bitcoind

# macOS (with Homebrew)
brew install bitcoin

# Or download from https://bitcoincore.org/en/download/
```

#### Configure Bitcoin Core
```bash
# Create bitcoin.conf
mkdir -p ~/.bitcoin
cat > ~/.bitcoin/bitcoin.conf << EOF
# Network settings
testnet=1
server=1
daemon=1
listen=1

# RPC settings
rpcuser=bitstable
rpcpassword=your_secure_password_here
rpcport=18332
rpcbind=127.0.0.1
rpcallowip=127.0.0.1

# Connection settings
maxconnections=50
addnode=testnet-seed.bitcoin.jonasschnelli.ch
addnode=seed.tbtc.petertodd.org

# Wallet settings
disablewallet=0
EOF

# Start Bitcoin Core
bitcoind -daemon

# Wait for sync (can take hours for first time)
bitcoin-cli -testnet getblockchaininfo
```

### Step 2: BitStable Configuration

#### Create Configuration File
```bash
cat > config/testnet.json << EOF
{
  "network": "Testnet",
  "min_collateral_ratio": 1.75,
  "liquidation_threshold": 1.25,
  "stability_fee_apr": 0.02,
  "oracle_threshold": 3,
  "bitcoin_config": {
    "network": "Testnet",
    "rpc_url": "http://127.0.0.1:18332",
    "rpc_user": "bitstable",
    "rpc_password": "your_secure_password_here",
    "min_confirmations": 1
  },
  "oracle_endpoints": [
    {
      "name": "coinbase",
      "url": "https://api.coinbase.com/v2/exchange-rates?currency=BTC",
      "weight": 1.0
    },
    {
      "name": "bitstamp",
      "url": "https://www.bitstamp.net/api/v2/ticker/btcusd/",
      "weight": 1.0
    },
    {
      "name": "kraken",
      "url": "https://api.kraken.com/0/public/Ticker?pair=XBTUSD",
      "weight": 1.0
    }
  ],
  "supported_currencies": [
    {
      "code": "USD",
      "name": "US Dollar",
      "decimals": 2,
      "stability_fee_modifier": 1.0
    },
    {
      "code": "EUR", 
      "name": "Euro",
      "decimals": 2,
      "stability_fee_modifier": 1.1
    }
  ]
}
EOF
```

### Step 3: Start the Complete System

#### Terminal 1: Oracle Network
```bash
# Start oracle with real price feeds
./target/release/oracle-node \
  --config config/testnet.json \
  --network testnet \
  --listen 127.0.0.1:8336 \
  --update-interval 30 \
  --verbose
```

#### Terminal 2: Main Protocol
```bash
# Check system status with config
./target/release/bitstable-cli \
  --config config/testnet.json \
  --network testnet \
  status

# Monitor oracle prices
watch -n 5 "./target/release/bitstable-cli --config config/testnet.json oracle price"
```

#### Terminal 3: Liquidator Bot
```bash
# Start liquidation monitoring (remove --dry-run for live trading)
./target/release/liquidator-bot \
  --config config/testnet.json \
  --liquidator-key $YOUR_LIQUIDATOR_KEY \
  --min-profit-btc 0.001 \
  --max-gas-btc 0.0001 \
  --scan-interval 30 \
  --verbose
```

## 🧪 Testing Scenarios

### Scenario 1: Basic Vault Operations
```bash
# 1. Create vault
OWNER_KEY="your_public_key_here"
./target/release/bitstable-cli vault create \
  --collateral-btc 2.0 \
  --stable-amount 50000 \
  --owner $OWNER_KEY

# 2. Check vault health
VAULT_ID=$(./target/release/bitstable-cli vault list | grep $OWNER_KEY | awk '{print $2}')
./target/release/bitstable-cli vault show $VAULT_ID

# 3. Monitor liquidation risk
./target/release/bitstable-cli liquidate scan
```

### Scenario 2: Multi-Currency Operations
```bash
# Create EUR-denominated vault
./target/release/bitstable-cli vault create \
  --collateral-btc 1.5 \
  --stable-amount 35000 \
  --currency EUR \
  --owner $OWNER_KEY

# Check multi-currency positions
./target/release/bitstable-cli vault list
```

### Scenario 3: Stress Testing
```bash
# Monitor system during price volatility
./target/release/bitstable-cli oracle test

# Check emergency triggers
./target/release/bitstable-cli status | grep -E "(Alert|Emergency)"

# View liquidation statistics
./target/release/bitstable-cli liquidate stats
```

## 📊 Monitoring & Operations

### Key Metrics to Monitor
```bash
# System health
./target/release/bitstable-cli status

# Oracle network
./target/release/bitstable-cli oracle status
./target/release/bitstable-cli oracle list

# Liquidation engine
./target/release/bitstable-cli liquidate stats
./target/release/bitstable-cli liquidate history

# Custody system
./target/release/bitstable-cli custody stats
./target/release/bitstable-cli custody contracts
```

### Troubleshooting Commands
```bash
# Check logs
tail -f /var/log/bitstable/*.log

# Verify Bitcoin connection
bitcoin-cli -testnet getnetworkinfo

# Test oracle connectivity
./target/release/bitstable-cli oracle test

# Emergency operations
./target/release/bitstable-cli vault update-fees
```

## 🔒 Security Considerations

### Key Management
- Use hardware wallets for production keys
- Implement multi-signature setups for protocol operations
- Rotate oracle keys regularly
- Secure RPC endpoints with firewalls

### Network Security
- Run Bitcoin Core with restricted RPC access
- Use VPN or private networks for oracle communication
- Implement rate limiting on public APIs
- Monitor for unusual liquidation activity

### Operational Security
- Regular database backups
- Monitoring alerts for system health
- Emergency shutdown procedures
- Incident response plans

## 🚀 Deployment Options

### Docker Deployment
```bash
# Build Docker images
docker build -t bitstable:latest .

# Run complete stack
docker-compose up -d
```

### Kubernetes Deployment
```bash
# Deploy to k8s cluster
kubectl apply -f k8s/

# Monitor pods
kubectl get pods -l app=bitstable
```

### Cloud Deployment
- **AWS**: Use ECS/EKS with RDS for databases
- **GCP**: Use GKE with Cloud SQL
- **Azure**: Use AKS with Azure Database

## 📈 Scaling Considerations

### High Availability
- Multiple oracle nodes across regions
- Load balancers for API endpoints
- Database replication
- Geographic distribution

### Performance Optimization
- Redis caching for frequent queries
- Database indexing optimization
- Async processing for heavy operations
- Rate limiting and request batching

## 🔧 Advanced Configuration

### Custom Oracle Endpoints
Add your own price feeds to the oracle network configuration.

### Multi-Currency Support
Extend the supported currencies by adding exchange rate mappings.

### Progressive Liquidation Tuning
Adjust liquidation thresholds based on market conditions.

### Proof-of-Reserves Frequency
Configure how often Merkle tree commitments are generated.

---

## 🆘 Support & Resources

- **Documentation**: Full API docs available
- **Issues**: Report bugs on GitHub
- **Discord**: Join the community for support
- **Email**: technical-support@bitstable.org

Happy trading with BitStable! 🚀