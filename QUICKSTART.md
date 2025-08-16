# 🚀 BitStable Quick Start

Get BitStable running in 3 simple ways:

## Option 1: Instant Demo (No Setup Required)
```bash
# Check if system works
./target/release/bitstable-cli status

# View vault management interface
./target/release/bitstable-cli vault --help

# See liquidation monitoring
./target/release/bitstable-cli liquidate --help
```

## Option 2: Interactive Demo 
```bash
# Run the full interactive demo
./scripts/demo.sh
```

## Option 3: Full System (With Oracle Network)
```bash
# Start complete testnet system
./scripts/start_testnet.sh

# In another terminal - monitor the system
watch -n 5 './target/release/bitstable-cli --config config/testnet.json status'

# Create your first vault (after oracles are running)
./target/release/bitstable-cli --config config/testnet.json vault create \
  --collateral-btc 1.0 \
  --stable-amount 40000 \
  --owner 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798

# Stop the system
./scripts/stop_testnet.sh
```

## What Each Option Shows

### Option 1: Basic Commands
- ✅ CLI interface and help system
- ✅ System status without dependencies
- ✅ Available commands and features

### Option 2: Interactive Demo  
- ✅ Guided walkthrough of all features
- ✅ System architecture explanation
- ✅ Multi-currency stablecoin concepts
- ✅ Production deployment guidance

### Option 3: Full System
- ✅ Real oracle network with price feeds
- ✅ Automated liquidation monitoring
- ✅ Actual vault creation and management
- ✅ Live system monitoring
- ✅ Production-like environment

## Next Steps

1. **Read the Guide**: Check `END_TO_END_GUIDE.md` for complete setup
2. **Production Setup**: Use `config/mainnet.json` for live deployment
3. **Bitcoin Integration**: Setup Bitcoin Core for real transactions
4. **Community**: Join Discord for support and updates

## Key Features Demonstrated

- 🏦 **Multi-Currency Vaults**: Create USD, EUR, GBP, JPY stablecoins
- ⚡ **Progressive Liquidation**: 25%/50%/75%/100% liquidation system
- 🔮 **Oracle Security**: Multi-source price feeds with circuit breakers
- 📊 **Proof-of-Reserves**: Real-time transparency with Merkle trees
- 🤖 **Automated Systems**: Liquidation bots and monitoring tools

Ready to build the future of Bitcoin-backed stablecoins! 🌟