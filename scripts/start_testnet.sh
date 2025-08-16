#!/bin/bash
# BitStable Testnet Startup Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting BitStable Testnet System${NC}"

# Configuration
BITSTABLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_FILE="${BITSTABLE_DIR}/config/testnet.json"
LOG_DIR="${BITSTABLE_DIR}/logs"
PID_DIR="${BITSTABLE_DIR}/pids"

# Create directories
mkdir -p "$LOG_DIR" "$PID_DIR"

# Default keys for testing (DO NOT USE IN PRODUCTION)
DEFAULT_ORACLE_KEY="0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
DEFAULT_LIQUIDATOR_KEY="02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"

# Parse command line arguments
ORACLE_KEY=${ORACLE_KEY:-$DEFAULT_ORACLE_KEY}
LIQUIDATOR_KEY=${LIQUIDATOR_KEY:-$DEFAULT_LIQUIDATOR_KEY}
DRY_RUN=${DRY_RUN:-true}

echo -e "${YELLOW}Configuration:${NC}"
echo "  • Config: $CONFIG_FILE"
echo "  • Oracle Key: ${ORACLE_KEY:0:20}..."
echo "  • Liquidator Key: ${LIQUIDATOR_KEY:0:20}..."
echo "  • Dry Run: $DRY_RUN"
echo

# Check if binaries exist
BINARIES=("bitstable-cli" "oracle-node" "liquidator-bot")
for binary in "${BINARIES[@]}"; do
    if [[ ! -f "${BITSTABLE_DIR}/target/release/${binary}" ]]; then
        echo -e "${RED}❌ Binary not found: ${binary}${NC}"
        echo "Please run: cargo build --release"
        exit 1
    fi
done

# Function to start a service
start_service() {
    local name=$1
    local cmd=$2
    local log_file="${LOG_DIR}/${name}.log"
    local pid_file="${PID_DIR}/${name}.pid"
    
    echo -e "${BLUE}Starting ${name}...${NC}"
    
    # Kill existing process if running
    if [[ -f "$pid_file" ]]; then
        local old_pid=$(cat "$pid_file")
        if kill -0 "$old_pid" 2>/dev/null; then
            echo -e "${YELLOW}Stopping existing ${name} process (PID: $old_pid)${NC}"
            kill "$old_pid"
            sleep 2
        fi
        rm -f "$pid_file"
    fi
    
    # Start new process
    eval "$cmd" > "$log_file" 2>&1 &
    local pid=$!
    echo $pid > "$pid_file"
    
    # Verify it started
    sleep 2
    if kill -0 $pid 2>/dev/null; then
        echo -e "${GREEN}✅ ${name} started (PID: $pid)${NC}"
        echo "   Log: $log_file"
    else
        echo -e "${RED}❌ Failed to start ${name}${NC}"
        echo "Check log: $log_file"
        exit 1
    fi
}

# Start Oracle Node
ORACLE_CMD="${BITSTABLE_DIR}/target/release/oracle-node \
    --config \"$CONFIG_FILE\" \
    --network testnet \
    --listen 127.0.0.1:8336 \
    --oracle-key \"$ORACLE_KEY\" \
    --update-interval 30 \
    --verbose"

start_service "oracle" "$ORACLE_CMD"

# Wait for oracle to initialize
echo -e "${BLUE}Waiting for oracle to initialize...${NC}"
sleep 5

# Start Liquidator Bot
LIQUIDATOR_CMD="${BITSTABLE_DIR}/target/release/liquidator-bot \
    --config \"$CONFIG_FILE\" \
    --liquidator-key \"$LIQUIDATOR_KEY\" \
    --min-profit-btc 0.001 \
    --max-gas-btc 0.0001 \
    --scan-interval 30 \
    --max-liquidations-per-round 3"

if [[ "$DRY_RUN" == "true" ]]; then
    LIQUIDATOR_CMD="$LIQUIDATOR_CMD --dry-run"
fi

LIQUIDATOR_CMD="$LIQUIDATOR_CMD --verbose"

start_service "liquidator" "$LIQUIDATOR_CMD"

echo
echo -e "${GREEN}🎉 BitStable Testnet System Started Successfully!${NC}"
echo
echo -e "${YELLOW}Available Commands:${NC}"
echo "  • Check Status:    ./target/release/bitstable-cli --config config/testnet.json status"
echo "  • Oracle Status:   ./target/release/bitstable-cli --config config/testnet.json oracle status" 
echo "  • Create Vault:    ./target/release/bitstable-cli --config config/testnet.json vault create --help"
echo "  • List Vaults:     ./target/release/bitstable-cli --config config/testnet.json vault list"
echo "  • Stop System:     ./scripts/stop_testnet.sh"
echo
echo -e "${YELLOW}Monitoring:${NC}"
echo "  • Oracle Log:      tail -f logs/oracle.log"
echo "  • Liquidator Log:  tail -f logs/liquidator.log"
echo "  • System Monitor:  watch -n 5 './target/release/bitstable-cli --config config/testnet.json status'"
echo
echo -e "${BLUE}System is ready for testing! 🚀${NC}"