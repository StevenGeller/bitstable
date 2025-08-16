#!/bin/bash

# Bitcoin Core Wrapper Script for macOS - Fixed Version
# Uses isolated data directory to avoid conflicts

# Set file descriptor limit for this session
ulimit -Sn 10240
ulimit -Hn 10240

# Clean up any existing Bitcoin processes
pkill -9 bitcoind 2>/dev/null
sleep 1

# Use a dedicated BitStable regtest directory to avoid conflicts
BITSTABLE_DATADIR="$HOME/.bitstable-regtest"
mkdir -p "$BITSTABLE_DATADIR"

# Create a minimal bitcoin.conf in our isolated directory
cat > "$BITSTABLE_DATADIR/bitcoin.conf" << 'EOF'
# BitStable Regtest Configuration
regtest=1

[regtest]
server=1
rpcuser=bitstable
rpcpassword=password
rpcallowip=127.0.0.1
rpcport=18443
rpcbind=127.0.0.1
fallbackfee=0.00001
disablewallet=0
maxconnections=4
rpcthreads=2
par=1
dbcache=50
maxmempool=50
EOF

echo "🚀 Starting Bitcoin Core with isolated data directory..."
echo "   Data directory: $BITSTABLE_DATADIR"
echo "   File descriptor limit: $(ulimit -n)"

# Start bitcoind with our isolated data directory
bitcoind -datadir="$BITSTABLE_DATADIR" -daemon