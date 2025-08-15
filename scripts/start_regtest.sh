#!/bin/bash

# BitStable Regtest Setup Script
# Starts Bitcoin Core in regtest mode with appropriate settings

echo "🤖 BitStable Regtest Setup"
echo "=========================="
echo ""

# Check if bitcoind is available
if ! command -v bitcoind &> /dev/null; then
    echo "❌ bitcoind not found in PATH"
    echo "💡 Please install Bitcoin Core:"
    echo "   • macOS: brew install bitcoin"
    echo "   • Ubuntu: sudo apt install bitcoind"
    echo "   • Or download from: https://bitcoincore.org/en/download/"
    exit 1
fi

# Create regtest data directory if it doesn't exist
BITCOIN_DIR="$HOME/.bitcoin"
REGTEST_DIR="$BITCOIN_DIR/regtest"

mkdir -p "$REGTEST_DIR"

# Create bitcoin.conf for regtest if it doesn't exist
CONF_FILE="$BITCOIN_DIR/bitcoin.conf"
if [ ! -f "$CONF_FILE" ]; then
    echo "📝 Creating Bitcoin configuration file..."
    cat > "$CONF_FILE" << EOF
# BitStable Bitcoin Configuration

# Network settings
regtest=1

# RPC settings
server=1
rpcuser=bitstable
rpcpassword=password
rpcallowip=127.0.0.1
rpcbind=127.0.0.1
rpcport=18443

# Wallet settings
fallbackfee=0.00001
EOF
    echo "✅ Created $CONF_FILE"
else
    echo "✅ Using existing $CONF_FILE"
fi

# Check if bitcoind is already running
if pgrep -x "bitcoind" > /dev/null; then
    echo "⚠️  bitcoind is already running"
    echo "💡 Stop it first with: bitcoin-cli stop"
    echo "   Or kill with: pkill bitcoind"
    read -p "🤔 Kill existing bitcoind and restart in regtest? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "🛑 Stopping existing bitcoind..."
        bitcoin-cli stop 2>/dev/null || pkill bitcoind
        sleep 3
    else
        echo "❌ Aborted. Please stop bitcoind manually first."
        exit 1
    fi
fi

# Start bitcoind in regtest mode
echo "🚀 Starting Bitcoin Core in regtest mode..."
bitcoind -regtest -daemon -rpcuser=bitstable -rpcpassword=password

# Wait for bitcoind to start
echo "⏳ Waiting for bitcoind to start..."
sleep 3

# Test connection
if bitcoin-cli -regtest getblockchaininfo &> /dev/null; then
    echo "✅ Bitcoin Core regtest is running!"
    
    # Show status
    BLOCK_COUNT=$(bitcoin-cli -regtest getblockcount)
    echo "📊 Regtest Status:"
    echo "   Block Height: $BLOCK_COUNT"
    echo "   RPC Port: 18443"
    echo "   Network: regtest"
    
    if [ "$BLOCK_COUNT" = "0" ]; then
        echo ""
        echo "🆕 Fresh regtest network detected!"
        echo "💡 The BitStable demo will automatically mine blocks to generate funds"
    fi
    
    echo ""
    echo "🎯 Ready for BitStable demo!"
    echo "   Run: cargo run --example automated_regtest_demo"
    
else
    echo "❌ Failed to connect to bitcoind"
    echo "💡 Check the logs for errors:"
    echo "   tail -f ~/.bitcoin/regtest/debug.log"
fi

echo ""
echo "📖 Useful Commands:"
echo "   • Check status: bitcoin-cli -regtest getblockchaininfo"
echo "   • Stop regtest: bitcoin-cli -regtest stop"
echo "   • View logs: tail -f ~/.bitcoin/regtest/debug.log"
echo "   • Reset blockchain: rm -rf ~/.bitcoin/regtest/blocks ~/.bitcoin/regtest/chainstate"