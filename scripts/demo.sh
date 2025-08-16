#!/bin/bash
# BitStable Interactive Demo Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

clear

echo -e "${BLUE}"
cat << "EOF"
 ____  _ _   ____  _        _     _      
| __ )(_) |_/ ___|| |_ __ _| |__ | | ___ 
|  _ \| | __\___ \| __/ _` | '_ \| |/ _ \
| |_) | | |_ ___) | || (_| | |_) | |  __/
|____/|_|\__|____/ \__\__,_|_.__/|_|\___|

Interactive Demo - Bitcoin-Collateralized Stablecoins
EOF
echo -e "${NC}"

# Configuration
BITSTABLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLI="${BITSTABLE_DIR}/target/release/bitstable-cli"
CONFIG="${BITSTABLE_DIR}/config/testnet.json"

# Demo keys (DO NOT USE IN PRODUCTION)
DEMO_USER1="0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
DEMO_USER2="02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"

# Utility functions
wait_for_enter() {
    echo -e "${CYAN}Press ENTER to continue...${NC}"
    read
}

run_command() {
    local desc=$1
    local cmd=$2
    local pause=${3:-true}
    
    echo -e "${YELLOW}📋 $desc${NC}"
    echo -e "${PURPLE}$ $cmd${NC}"
    
    if [[ "$pause" == "true" ]]; then
        wait_for_enter
    fi
    
    eval "$cmd"
    echo
    
    if [[ "$pause" == "true" ]]; then
        wait_for_enter
    fi
}

echo -e "${GREEN}Welcome to the BitStable Interactive Demo!${NC}"
echo
echo "This demo will show you:"
echo "  🏦 Creating Bitcoin-collateralized vaults"
echo "  💰 Minting multi-currency stablecoins"
echo "  ⚡ Monitoring liquidation systems"
echo "  🔮 Oracle price feeds"
echo "  📊 System health monitoring"
echo
wait_for_enter

# Check if system is built
if [[ ! -f "$CLI" ]]; then
    echo -e "${RED}❌ BitStable CLI not found!${NC}"
    echo "Please run: cargo build --release"
    exit 1
fi

# Demo Step 1: System Status
echo -e "${BLUE}=== Step 1: System Overview ===${NC}"
run_command "Check overall system status" \
    "$CLI --config $CONFIG status"

# Demo Step 2: Oracle Network
echo -e "${BLUE}=== Step 2: Oracle Network ===${NC}"
run_command "Check oracle network status" \
    "$CLI --config $CONFIG oracle status"

run_command "Get current Bitcoin price consensus" \
    "$CLI --config $CONFIG oracle price"

run_command "List configured oracle endpoints" \
    "$CLI --config $CONFIG oracle list"

# Demo Step 3: Create Vaults
echo -e "${BLUE}=== Step 3: Creating Vaults ===${NC}"
echo -e "${GREEN}Let's create some Bitcoin-collateralized vaults!${NC}"
echo

run_command "Create a USD vault (2 BTC → $80k stable)" \
    "$CLI --config $CONFIG vault create --collateral-btc 2.0 --stable-amount 80000 --owner $DEMO_USER1"

run_command "Create a EUR vault (1.5 BTC → €45k stable)" \
    "$CLI --config $CONFIG vault create --collateral-btc 1.5 --stable-amount 45000 --owner $DEMO_USER2"

run_command "List all active vaults" \
    "$CLI --config $CONFIG vault list"

# Demo Step 4: Vault Details
echo -e "${BLUE}=== Step 4: Vault Management ===${NC}"

# Get vault IDs (simplified - in reality you'd parse the output)
echo -e "${YELLOW}📋 Let's examine a specific vault in detail${NC}"
run_command "Show detailed vault information" \
    "$CLI --config $CONFIG vault list --liquidatable false"

# Demo Step 5: Liquidation System
echo -e "${BLUE}=== Step 5: Liquidation System ===${NC}"
run_command "Scan for liquidation opportunities" \
    "$CLI --config $CONFIG liquidate scan"

run_command "View liquidation statistics" \
    "$CLI --config $CONFIG liquidate stats"

run_command "Check liquidation history" \
    "$CLI --config $CONFIG liquidate history --limit 5"

# Demo Step 6: Stable Value Operations
echo -e "${BLUE}=== Step 6: Stable Value Operations ===${NC}"
run_command "Check stable value supply" \
    "$CLI --config $CONFIG stable supply"

echo -e "${YELLOW}Note: Transfer and burn operations require active vaults${NC}"
echo

# Demo Step 7: Custody System
echo -e "${BLUE}=== Step 7: Custody & Security ===${NC}"
run_command "View custody system statistics" \
    "$CLI --config $CONFIG custody stats"

run_command "List escrow contracts" \
    "$CLI --config $CONFIG custody contracts"

# Demo Step 8: Advanced Features
echo -e "${BLUE}=== Step 8: Advanced Features ===${NC}"
echo -e "${GREEN}BitStable includes advanced DeFi features:${NC}"
echo
echo "🔒 Progressive Liquidation System:"
echo "   • 25% liquidation at 175% collateral ratio"
echo "   • 50% liquidation at 150% collateral ratio" 
echo "   • 75% liquidation at 135% collateral ratio"
echo "   • 100% liquidation at 125% collateral ratio"
echo
echo "🔮 Oracle Security:"
echo "   • Multi-source price aggregation"
echo "   • Circuit breakers for price manipulation"
echo "   • Economic bonding and slashing"
echo
echo "📊 Proof-of-Reserves:"
echo "   • Real-time Merkle tree commitments"
echo "   • Bitcoin blockchain anchoring"
echo "   • Trustless verification"
echo
wait_for_enter

# Demo Step 9: Monitoring
echo -e "${BLUE}=== Step 9: Monitoring Tools ===${NC}"
echo -e "${GREEN}Here are the key monitoring commands:${NC}"
echo
echo -e "${YELLOW}Real-time monitoring:${NC}"
echo "  watch -n 5 '$CLI --config $CONFIG status'"
echo
echo -e "${YELLOW}Oracle price tracking:${NC}"
echo "  watch -n 10 '$CLI --config $CONFIG oracle price'"
echo
echo -e "${YELLOW}Liquidation monitoring:${NC}"
echo "  watch -n 30 '$CLI --config $CONFIG liquidate scan'"
echo
wait_for_enter

# Demo Step 10: Production Setup
echo -e "${BLUE}=== Step 10: Production Deployment ===${NC}"
echo -e "${GREEN}For production deployment:${NC}"
echo
echo "1. 🔧 Setup Bitcoin Core node"
echo "2. 🔑 Generate secure keys with hardware wallets"
echo "3. 🌐 Configure multi-region oracle network"
echo "4. ⚡ Deploy automated liquidation bots"
echo "5. 📊 Setup monitoring and alerting"
echo "6. 🔒 Implement multi-signature governance"
echo
echo -e "${YELLOW}Reference files:${NC}"
echo "  • Full guide: END_TO_END_GUIDE.md"
echo "  • Production config: config/mainnet.json"
echo "  • Startup scripts: scripts/"
echo
wait_for_enter

# Demo Complete
echo -e "${GREEN}"
cat << "EOF"
🎉 Demo Complete! 

BitStable Features Demonstrated:
✅ Multi-currency stablecoin creation
✅ Progressive liquidation system
✅ Oracle network with security features
✅ Proof-of-reserves transparency
✅ Comprehensive monitoring tools
✅ Production-ready architecture

Next Steps:
• Read the full documentation
• Experiment with vault creation
• Test liquidation scenarios
• Setup production environment
• Join the community for support

Happy building with BitStable! 🚀
EOF
echo -e "${NC}"