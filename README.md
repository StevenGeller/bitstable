# BitStable - Decentralized Bitcoin-Backed Stablecoin

## Overview

BitStable is a decentralized stablecoin protocol built on Bitcoin that enables users to mint stable value tokens backed by Bitcoin collateral. The protocol implements an overcollateralized vault system with automated liquidation mechanisms to maintain peg stability.

## Key Features

- **Overcollateralized Vaults**: 150% minimum collateralization ratio
- **Decentralized Oracle Network**: Multiple price feeds with threshold aggregation
- **2-of-3 Multisig Custody**: Secure Bitcoin collateral management
- **Automated Liquidations**: Market-based liquidation with 13% penalty
- **Stability Fee**: 2% annual fee for vault positions
- **On-chain Settlement**: All operations settled on Bitcoin blockchain

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   User Wallet   │────▶│  BitStable Core │────▶│  Bitcoin Node   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │   Oracle Network    │
                    └─────────────────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │   Database (Sled)   │
                    └─────────────────────┘
```

## Project Structure

```
bitstable/
├── src/
│   ├── main.rs           # Application entry point
│   ├── vault.rs          # Vault management logic
│   ├── oracle.rs         # Price oracle implementation
│   ├── liquidation.rs    # Liquidation engine
│   ├── bitcoin_rpc.rs    # Bitcoin node integration
│   ├── database.rs       # Persistence layer
│   ├── crypto.rs         # Cryptographic operations
│   └── lib.rs            # Library exports
├── tests/
│   └── integration.rs    # Integration tests
├── Cargo.toml            # Dependencies
└── README.md             # This file
```

## Quick Start

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Bitcoin Core node (testnet or regtest)
- Git

### Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/bitstable.git
cd bitstable
```

2. Build the project:
```bash
cargo build --release
```

3. Run tests:
```bash
cargo test
```

4. Start the application:
```bash
cargo run --release
```

## Configuration

Create a `.env` file in the project root:

```env
# Bitcoin RPC Configuration
BITCOIN_RPC_URL=http://localhost:18332
BITCOIN_RPC_USER=your_rpc_user
BITCOIN_RPC_PASS=your_rpc_password

# Database Configuration
DATABASE_PATH=./data/bitstable.db

# Oracle Configuration
ORACLE_UPDATE_INTERVAL=60  # seconds
ORACLE_THRESHOLD=2          # minimum confirmations

# Network Configuration
NETWORK=testnet             # mainnet, testnet, or regtest
API_PORT=8080
```

## Usage

### Creating a Vault

```bash
# Create a new vault with 0.1 BTC collateral to mint 1000 stable tokens
curl -X POST http://localhost:8080/api/vault/create \
  -H "Content-Type: application/json" \
  -d '{
    "collateral_amount": 10000000,
    "stable_amount": 1000
  }'
```

### Checking Vault Status

```bash
# Get vault details by ID
curl http://localhost:8080/api/vault/{vault_id}
```

### Adding Collateral

```bash
# Add more collateral to improve health factor
curl -X POST http://localhost:8080/api/vault/{vault_id}/add-collateral \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 5000000
  }'
```

## Development

### Running in Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run with file watching (requires cargo-watch)
cargo watch -x run
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test module
cargo test vault::tests

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Security audit
cargo audit
```

## Security Considerations

- **Private Keys**: Never commit private keys or mnemonics
- **Multisig Setup**: Requires secure key ceremony for production
- **Oracle Trust**: System relies on honest oracle majority
- **Liquidation Risks**: Users can lose collateral if BTC price drops
- **Smart Contract Audit**: Recommended before mainnet deployment

## API Documentation

See [API.md](./docs/API.md) for complete API reference.

## Contributing

Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution guidelines.

## License

This project is licensed under the MIT License - see [LICENSE](./LICENSE) file for details.

## Roadmap

- [x] Core vault implementation
- [x] Oracle network integration
- [x] Database persistence
- [x] Cryptographic security
- [ ] Bitcoin RPC integration
- [ ] FROST threshold signatures
- [ ] WebSocket real-time updates
- [ ] Admin dashboard
- [ ] Mainnet deployment
