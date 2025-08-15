#!/bin/bash

echo "🪙  BitStable Real Bitcoin Testnet Demo Runner"
echo "============================================="
echo ""

echo "🧹 Cleaning previous builds..."
cargo clean > /dev/null 2>&1

echo "🔨 Building BitStable library and examples..."
if cargo build --release --lib --examples; then
    echo "✅ Build successful!"
else
    echo "❌ Build failed!"
    exit 1
fi

echo ""
echo "🪙  Running REAL Bitcoin testnet demo..."
echo "======================================="
echo "⚠️  WARNING: This will attempt to use REAL Bitcoin testnet!"
echo "📋 Prerequisites:"
echo "   • Bitcoin Core testnet node running"
echo "   • RPC access configured (bitcoin:password)"
echo "   • Node synced with testnet blockchain"
echo ""

# Check if Bitcoin Core is running
if ! netstat -an | grep -q ":18332.*LISTEN"; then
    echo "❌ Bitcoin Core testnet RPC not detected on port 18332"
    echo "💡 Start Bitcoin Core with testnet=1 and RPC enabled"
    echo "   Example command: bitcoind -testnet -rpcuser=bitcoin -rpcpassword=password"
    echo ""
    echo "🔧 Or create ~/.bitcoin/bitcoin.conf with:"
    echo "   testnet=1"
    echo "   server=1"
    echo "   rpcuser=bitcoin"
    echo "   rpcpassword=password"
    echo "   rpcallowip=127.0.0.1"
    echo ""
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo ""
echo "🚀 Starting real Bitcoin testnet demo..."
echo ""

# Run the real Bitcoin testnet demo
cargo run --release --example real_bitcoin_testnet_demo