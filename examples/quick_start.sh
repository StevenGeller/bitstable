#!/bin/bash
# BitStable Quick Start Example

set -e

echo "🚀 BitStable Quick Start"
echo "======================="

# Check if built
if [[ ! -f "target/release/bitstable-cli" ]]; then
    echo "Building BitStable..."
    cargo build --release
fi

# Demo key (DO NOT USE IN PRODUCTION)
DEMO_KEY="0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

echo
echo "1. Check system status:"
./target/release/bitstable-cli status

echo
echo "2. Check oracle network:"
./target/release/bitstable-cli oracle status

echo
echo "3. Try to create a test vault:"
echo "   (2 BTC collateral → $80,000 USD stablecoin)"
if ./target/release/bitstable-cli vault create \
  --collateral-btc 2.0 \
  --stable-amount 80000 \
  --owner $DEMO_KEY 2>/dev/null; then
    echo "✅ Vault created successfully!"
    
    echo
    echo "4. List all vaults:"
    ./target/release/bitstable-cli vault list
    
    echo
    echo "5. Check for liquidation opportunities:"
    ./target/release/bitstable-cli liquidate scan
else
    echo "❌ Vault creation failed (expected - no oracle price data)"
    echo "   This is normal behavior without running oracles."
    echo
    echo "💡 To create vaults, you need:"
    echo "   • Running oracle network with price consensus"
    echo "   • Use: ./scripts/start_testnet.sh for full system"
    echo
    echo "4. View available vault commands:"
    ./target/release/bitstable-cli vault --help
    
    echo
    echo "5. View liquidation monitoring:"
    ./target/release/bitstable-cli liquidate --help
fi

echo
echo "✅ Quick start complete!"
echo
echo "Next steps:"
echo "• Read END_TO_END_GUIDE.md for full setup"
echo "• Run ./scripts/demo.sh for interactive demo"
echo "• Use ./scripts/start_testnet.sh for full system"