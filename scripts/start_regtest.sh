#!/bin/bash

# BitStable Regtest Setup - Production Ready
# Uses isolated data directory to avoid conflicts

echo "🤖 BitStable Regtest Setup"
echo "=========================="
echo ""

# Check if bitcoind is available
if ! command -v bitcoind &> /dev/null; then
    echo "❌ bitcoind not found in PATH"
    echo "💡 Please install Bitcoin Core:"
    echo "   • macOS: brew install bitcoin"
    echo "   • Ubuntu: sudo apt install bitcoind"
    exit 1
fi

# Define our isolated data directory
BITSTABLE_DATADIR="$HOME/.bitstable-regtest"

# Kill any existing Bitcoin processes
if pgrep -x "bitcoind" > /dev/null; then
    echo "🛑 Stopping existing bitcoind..."
    bitcoin-cli -datadir="$BITSTABLE_DATADIR" stop 2>/dev/null || pkill -9 bitcoind
    sleep 2
fi

# Set file descriptor limit
echo "🔧 Setting file descriptor limits..."
ulimit -Sn 10240
echo "✅ File descriptor limit: $(ulimit -n)"

# Run the wrapper script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "🚀 Starting Bitcoin Core regtest..."
bash "$SCRIPT_DIR/bitcoin_wrapper.sh"

# Wait for Bitcoin to start
echo "⏳ Waiting for Bitcoin regtest to start..."
for i in {1..30}; do
    if bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password getblockchaininfo &> /dev/null; then
        echo ""
        echo "✅ Bitcoin Core regtest is running!"
        
        # Get blockchain info
        BLOCK_COUNT=$(bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password getblockcount 2>/dev/null || echo "0")
        
        echo ""
        echo "📊 Regtest Status:"
        echo "   • Network: regtest"
        echo "   • Block Height: $BLOCK_COUNT"
        echo "   • Data Directory: $BITSTABLE_DATADIR"
        echo "   • RPC Port: 18443"
        echo "   • RPC User: bitstable"
        echo "   • RPC Password: password"
        
        if [ "$BLOCK_COUNT" = "0" ]; then
            echo ""
            echo "💡 Mining initial blocks for testing..."
            # Create wallet if it doesn't exist
            bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password createwallet "bitstable" 2>/dev/null || true
            
            # Get address and mine blocks
            ADDR=$(bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password getnewaddress 2>/dev/null)
            if [ ! -z "$ADDR" ]; then
                bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password generatetoaddress 101 "$ADDR" > /dev/null 2>&1
                echo "✅ Mined 101 blocks to $ADDR"
                
                # Show balance
                BALANCE=$(bitcoin-cli -datadir="$BITSTABLE_DATADIR" -rpcuser=bitstable -rpcpassword=password getbalance 2>/dev/null)
                echo "💰 Wallet balance: $BALANCE BTC"
            fi
        fi
        
        echo ""
        echo "🎯 Bitcoin regtest ready for BitStable!"
        echo ""
        echo "📝 Quick Commands (add -datadir=$BITSTABLE_DATADIR to all):"
        echo "   • Status: bitcoin-cli -datadir=\"$BITSTABLE_DATADIR\" getblockchaininfo"
        echo "   • Balance: bitcoin-cli -datadir=\"$BITSTABLE_DATADIR\" getbalance"
        echo "   • Mine: bitcoin-cli -datadir=\"$BITSTABLE_DATADIR\" generatetoaddress 1 <address>"
        echo "   • Stop: bitcoin-cli -datadir=\"$BITSTABLE_DATADIR\" stop"
        echo ""
        
        # Export for other scripts
        export BITCOIN_DATADIR="$BITSTABLE_DATADIR"
        export BITCOIN_RPC_USER="bitstable"
        export BITCOIN_RPC_PASSWORD="password"
        export BITCOIN_RPC_PORT="18443"
        
        exit 0
    fi
    
    # Show progress
    if [ $((i % 5)) -eq 0 ]; then
        echo -n "."
    fi
    
    sleep 1
done

echo ""
echo "❌ Bitcoin failed to start after 30 seconds"
echo ""
echo "🔍 Troubleshooting:"
echo "   1. Check logs: tail -f $BITSTABLE_DATADIR/regtest/debug.log"
echo "   2. Clear data: rm -rf $BITSTABLE_DATADIR"
echo "   3. Check port: lsof -i :18443"
echo ""
exit 1