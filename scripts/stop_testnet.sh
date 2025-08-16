#!/bin/bash
# BitStable Testnet Shutdown Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🛑 Stopping BitStable Testnet System${NC}"

# Configuration
BITSTABLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_DIR="${BITSTABLE_DIR}/pids"

# Function to stop a service
stop_service() {
    local name=$1
    local pid_file="${PID_DIR}/${name}.pid"
    
    if [[ -f "$pid_file" ]]; then
        local pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}Stopping ${name} (PID: $pid)...${NC}"
            kill "$pid"
            
            # Wait for graceful shutdown
            local count=0
            while kill -0 "$pid" 2>/dev/null && [[ $count -lt 10 ]]; do
                sleep 1
                ((count++))
            done
            
            # Force kill if still running
            if kill -0 "$pid" 2>/dev/null; then
                echo -e "${RED}Force killing ${name}...${NC}"
                kill -9 "$pid"
            fi
            
            echo -e "${GREEN}✅ ${name} stopped${NC}"
        else
            echo -e "${YELLOW}${name} was not running${NC}"
        fi
        rm -f "$pid_file"
    else
        echo -e "${YELLOW}No PID file found for ${name}${NC}"
    fi
}

# Stop services in reverse order
stop_service "liquidator"
stop_service "oracle"

# Clean up any remaining processes
echo -e "${BLUE}Cleaning up any remaining processes...${NC}"
pkill -f "oracle-node" 2>/dev/null || true
pkill -f "liquidator-bot" 2>/dev/null || true

echo
echo -e "${GREEN}🏁 BitStable Testnet System Stopped Successfully!${NC}"
echo
echo -e "${YELLOW}Logs preserved in:${NC} ${BITSTABLE_DIR}/logs/"
echo -e "${YELLOW}To restart:${NC} ./scripts/start_testnet.sh"