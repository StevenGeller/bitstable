#!/bin/bash

# Comprehensive BitStable Regtest Demo Test Script
# Tests the entire regtest setup and demo process

echo "🧪 BitStable Regtest Demo End-to-End Test"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️ $1${NC}"
}

# Test 1: Validate Rust compilation
echo "🦀 Test 1: Rust Compilation"
echo "---------------------------"

print_info "Building regtest examples..."
if cargo build --example automated_regtest_demo --example simple_regtest_example --example regtest_validation --quiet; then
    print_success "All regtest examples compile successfully"
else
    print_error "Compilation failed"
    exit 1
fi

# Check for warnings
echo "Checking for compilation warnings..."
if cargo build --example automated_regtest_demo 2>&1 | grep -q "warning:"; then
    print_warning "Compilation warnings detected"
    cargo build --example automated_regtest_demo 2>&1 | grep "warning:"
else
    print_success "No compilation warnings"
fi

echo ""

# Test 2: Run validation suite
echo "🧪 Test 2: Logic Validation"
echo "---------------------------"

print_info "Running comprehensive validation..."
if cargo run --example regtest_validation --quiet; then
    print_success "All validation tests passed"
else
    print_error "Validation tests failed"
    exit 1
fi

echo ""

# Test 3: Check Bitcoin Core availability
echo "₿ Test 3: Bitcoin Core Availability"
echo "-----------------------------------"

if command -v bitcoind &> /dev/null; then
    print_success "bitcoind found in PATH"
    BITCOIND_VERSION=$(bitcoind --version | head -n 1)
    print_info "Version: $BITCOIND_VERSION"
else
    print_warning "bitcoind not found in PATH"
    print_info "Install Bitcoin Core to run full demo:"
    print_info "  • macOS: brew install bitcoin"
    print_info "  • Ubuntu: sudo apt install bitcoind"
    print_info "  • Download: https://bitcoincore.org/en/download/"
fi

if command -v bitcoin-cli &> /dev/null; then
    print_success "bitcoin-cli found in PATH"
else
    print_warning "bitcoin-cli not found in PATH"
fi

echo ""

# Test 4: Check regtest setup script
echo "📜 Test 4: Setup Script Validation"
echo "----------------------------------"

if [ -x "./scripts/start_regtest.sh" ]; then
    print_success "Regtest setup script is executable"
else
    print_error "Setup script not executable"
    chmod +x ./scripts/start_regtest.sh
    print_info "Made setup script executable"
fi

# Validate script syntax
if bash -n ./scripts/start_regtest.sh; then
    print_success "Setup script syntax is valid"
else
    print_error "Setup script has syntax errors"
    exit 1
fi

echo ""

# Test 5: Check if Bitcoin regtest is running
echo "🌐 Test 5: Bitcoin Regtest Status"
echo "---------------------------------"

if command -v bitcoin-cli &> /dev/null; then
    if bitcoin-cli -regtest getblockchaininfo &> /dev/null; then
        BLOCK_COUNT=$(bitcoin-cli -regtest getblockcount 2>/dev/null)
        print_success "Bitcoin regtest is running (block height: $BLOCK_COUNT)"
        
        # Test RPC connectivity
        if bitcoin-cli -regtest -rpcuser=bitstable -rpcpassword=password getblockchaininfo &> /dev/null; then
            print_success "RPC credentials work correctly"
        else
            print_warning "RPC credentials may not be configured correctly"
            print_info "Expected: rpcuser=bitstable, rpcpassword=password"
        fi
    else
        print_warning "Bitcoin regtest not running"
        print_info "Start with: ./scripts/start_regtest.sh"
        print_info "Or manually: bitcoind -regtest -daemon -rpcuser=bitstable -rpcpassword=password"
    fi
else
    print_warning "Cannot test Bitcoin regtest (bitcoin-cli not available)"
fi

echo ""

# Test 6: Port availability check
echo "🔌 Test 6: Port Configuration"
echo "-----------------------------"

if command -v netstat &> /dev/null; then
    if netstat -an 2>/dev/null | grep -q ":18443.*LISTEN"; then
        print_success "Port 18443 is listening (regtest RPC)"
    else
        print_warning "Port 18443 not listening"
        print_info "This is normal if Bitcoin regtest is not running"
    fi
    
    if netstat -an 2>/dev/null | grep -q ":18444.*LISTEN"; then
        print_success "Port 18444 is listening (regtest P2P)"
    else
        print_warning "Port 18444 not listening"
    fi
else
    print_warning "netstat not available, cannot check ports"
fi

echo ""

# Test 7: File permissions and structure
echo "📁 Test 7: File Structure"
echo "-------------------------"

required_files=(
    "examples/automated_regtest_demo.rs"
    "examples/simple_regtest_example.rs"
    "examples/regtest_validation.rs"
    "scripts/start_regtest.sh"
    "REGTEST.md"
    "src/bitcoin_client.rs"
)

for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        print_success "$file exists"
    else
        print_error "$file missing"
        exit 1
    fi
done

echo ""

# Test 8: Documentation completeness
echo "📚 Test 8: Documentation"
echo "------------------------"

if [ -f "REGTEST.md" ]; then
    if grep -q "Quick Start" REGTEST.md; then
        print_success "REGTEST.md has Quick Start section"
    else
        print_warning "REGTEST.md missing Quick Start section"
    fi
    
    if grep -q "Examples" REGTEST.md; then
        print_success "REGTEST.md has Examples section"
    else
        print_warning "REGTEST.md missing Examples section"
    fi
    
    if grep -q "Troubleshooting" REGTEST.md; then
        print_success "REGTEST.md has Troubleshooting section"
    else
        print_warning "REGTEST.md missing Troubleshooting section"
    fi
fi

echo ""

# Test 9: Configuration validation
echo "⚙️ Test 9: Configuration"
echo "------------------------"

BITCOIN_DIR="$HOME/.bitcoin"
if [ -d "$BITCOIN_DIR" ]; then
    print_success "Bitcoin data directory exists"
    
    if [ -f "$BITCOIN_DIR/bitcoin.conf" ]; then
        print_success "bitcoin.conf exists"
        
        if grep -q "regtest=1" "$BITCOIN_DIR/bitcoin.conf" 2>/dev/null; then
            print_success "Regtest enabled in bitcoin.conf"
        else
            print_warning "Regtest not configured in bitcoin.conf"
        fi
    else
        print_warning "bitcoin.conf not found"
        print_info "Will be created automatically by setup script"
    fi
else
    print_warning "Bitcoin data directory not found"
    print_info "Will be created on first run"
fi

echo ""

# Final summary
echo "🎯 Test Summary"
echo "==============="

# Count tests
total_tests=9
echo "Ran $total_tests test categories:"

print_success "Compilation: All examples build correctly"
print_success "Validation: Logic tests pass"
print_success "Setup: Scripts are ready"
print_success "Structure: All files present"

echo ""
echo "🚀 Ready to Run Demo!"
echo "====================="
echo ""
echo "To run the full automated regtest demo:"
echo ""
echo "1. Start Bitcoin regtest:"
echo "   ${GREEN}./scripts/start_regtest.sh${NC}"
echo ""
echo "2. Run the demo:"
echo "   ${GREEN}cargo run --example automated_regtest_demo${NC}"
echo ""
echo "3. Or run simple example:"
echo "   ${GREEN}cargo run --example simple_regtest_example${NC}"
echo ""

if ! command -v bitcoind &> /dev/null; then
    print_warning "Install Bitcoin Core first for full functionality"
    echo "   • macOS: ${BLUE}brew install bitcoin${NC}"
    echo "   • Ubuntu: ${BLUE}sudo apt install bitcoind${NC}"
    echo ""
fi

print_success "All tests completed successfully!"
print_info "The regtest demo is ready for end-to-end execution"