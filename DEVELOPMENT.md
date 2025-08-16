# DEVELOPMENT.md

This file provides guidance for developers working with the BitStable codebase.

## Project Overview

BitStable is a Bitcoin-backed multi-currency stablecoin protocol written in Rust. It enables users to mint stable value tokens (USD, EUR) by locking Bitcoin as collateral in overcollateralized vaults with automated liquidation mechanisms.

## Build and Development Commands

### Essential Commands
```bash
# Build the project
cargo build --release

# Run all tests
cargo test

# Run specific test module
cargo test vault::tests

# Quick functionality test
./quick_test.sh

# Comprehensive test suite
./run_comprehensive_test.sh

# Run simple example
cargo run --example simple_test

# Run full demo
cargo run --example final_demo

# Run testnet demo (requires Bitcoin testnet node)
cargo run --example testnet_demo

# Run regtest validation (local Bitcoin testing)
cargo run --example regtest_validation
```

### CLI Tools
```bash
# Main CLI interface
cargo run --bin bitstable-cli -- [commands]

# Oracle price feed node
cargo run --bin oracle-node

# Liquidation bot
cargo run --bin liquidator-bot
```

## Architecture Overview

The codebase follows a modular, event-driven architecture:

1. **Core Protocol** (`src/lib.rs`): Central `BitStableProtocol` orchestrator managing all subsystems
2. **Vault System** (`src/vault.rs`): Collateralized debt positions with state machine (Pending → Active → Warning → Liquidating → Closed)
3. **Multi-Currency** (`src/multi_currency.rs`): USD/EUR support with real-time exchange rates
4. **Stability Controllers** (`src/stability_controller.rs`): Automated portfolio rebalancing (fixed amount or percentage-based)
5. **Oracle Network** (`src/oracle.rs`): Multi-source price aggregation with 3-of-5 consensus
6. **Liquidation Engine** (`src/liquidation.rs`): Monitors health and triggers liquidations at <110% collateral ratio
7. **Bitcoin Integration** (`src/bitcoin_client.rs`): Full Bitcoin Core RPC integration for on-chain operations
8. **Custody** (`src/custody.rs`): 2-of-3 multisig custody with threshold signature support
9. **Database** (`src/database.rs`): Persistent storage using Sled embedded database

## Critical Protocol Parameters

- **Minimum Collateral Ratio**: 150%
- **Liquidation Threshold**: 110%
- **Liquidation Penalty**: 5%
- **Stability Fee**: 2% APR
- **Oracle Consensus**: 3 of 5 sources required

## Testing Approach

The project uses Rust's built-in testing framework with unit tests in each module. Key testing patterns:

- Unit tests are inline in each module (e.g., `mod tests` blocks)
- Integration examples in `examples/` directory
- Shell scripts for comprehensive testing scenarios
- Regtest support for local Bitcoin network testing

## Bitcoin Configuration

For Bitcoin integration, configure RPC credentials:
- Set environment variables or create `.env` file:
  ```
  BITCOIN_RPC_URL=http://localhost:18332
  BITCOIN_RPC_USER=your_user
  BITCOIN_RPC_PASSWORD=your_password
  ```
- Supports testnet (port 18332), regtest (port 18443), and mainnet (port 8332)

## Key Development Patterns

1. **Error Handling**: Uses `anyhow::Result` throughout with custom `BitstableError` types
2. **Async Operations**: Tokio runtime for all async operations
3. **Database Operations**: All database writes are atomic using Sled transactions
4. **Bitcoin Transactions**: Always verify transaction creation before broadcasting
5. **Price Oracle**: Always check consensus before using price data
6. **Vault Operations**: State transitions must be validated before persistence

## Common Development Tasks

### Adding New Currency Support
1. Update `Currency` enum in `src/multi_currency.rs`
2. Add exchange rate sources in `src/oracle.rs`
3. Update stability controller logic in `src/stability_controller.rs`
4. Add tests for new currency operations

### Modifying Liquidation Parameters
1. Update constants in `src/liquidation.rs`
2. Adjust tests in liquidation module
3. Update documentation in `docs/ARCHITECTURE.md`

### Testing Bitcoin Integration
1. Use regtest for local testing: `bitcoind -regtest -daemon`
2. Run `cargo run --example regtest_validation` for automated testing
3. For testnet: ensure testnet node is synced before running examples

## Dependencies

Core dependencies (from Cargo.toml):
- `bitcoin` 0.32 - Bitcoin protocol implementation
- `bitcoincore-rpc` 0.19 - Bitcoin Core RPC client
- `tokio` 1.0 - Async runtime
- `sled` 0.34 - Embedded database
- `secp256k1` 0.29 - Cryptographic operations
- `reqwest` 0.12 - HTTP client for oracle feeds
- `clap` 4.0 - CLI argument parsing