#!/bin/bash

# Quick BitStable Test - Runs basic functionality tests
echo "⚡ BitStable Quick Test"
echo "====================="
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Run this from the BitStable project root"
    exit 1
fi

echo "🧪 Running library tests..."
cargo test --release --lib

echo ""
echo "🚀 Running simple example..."
cargo run --release --example simple_test

echo ""
echo "✅ Quick test complete! Run './run_comprehensive_test.sh' for full testing."