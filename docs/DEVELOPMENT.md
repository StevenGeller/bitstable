# Development Guide

## Table of Contents

1. [Project Setup](#project-setup)
2. [Architecture Overview](#architecture-overview)
3. [Code Structure](#code-structure)
4. [Development Workflow](#development-workflow)
5. [Testing](#testing)
6. [Debugging](#debugging)
7. [Contributing](#contributing)
8. [Deployment](#deployment)

## Project Setup

### Prerequisites

Ensure you have the following installed:
- Rust 1.70+ with cargo
- Bitcoin Core (testnet/regtest)
- Git
- Optional: Docker, cargo-watch, cargo-tarpaulin

### Initial Setup

1. **Clone and setup:**
```bash
git clone https://github.com/yourusername/bitstable.git
cd bitstable
cargo build
```

2. **Configure environment:**
```bash
cp .env.example .env
# Edit .env with your configuration
```

3. **Setup Bitcoin regtest for development:**
```bash
# Start Bitcoin in regtest mode
bitcoind -regtest -daemon \
  -rpcuser=bitstable \
  -rpcpassword=development \
  -rpcport=18443

# Generate some blocks
bitcoin-cli -regtest generatetoaddress 101 $(bitcoin-cli -regtest getnewaddress)
```

## Architecture Overview

### System Components

```
┌─────────────────────────────────────────────────┐
│                   API Layer                      │
│  (REST API, WebSocket, Authentication)           │
└─────────────────────────────────────────────────┘
                         │
┌─────────────────────────────────────────────────┐
│                 Business Logic                   │
│  (Vault Manager, Oracle, Liquidation Engine)     │
└─────────────────────────────────────────────────┘
                         │
┌─────────────────────────────────────────────────┐
│                 Data Layer                       │
│  (Database, Bitcoin RPC, External APIs)          │
└─────────────────────────────────────────────────┘
```

### Core Modules

- **vault.rs**: Vault lifecycle management
- **oracle.rs**: Price feed aggregation
- **liquidation.rs**: Liquidation detection and execution
- **bitcoin_rpc.rs**: Bitcoin node communication
- **database.rs**: Persistent storage
- **crypto.rs**: Cryptographic operations

## Code Structure

### Module Organization

```rust
// src/lib.rs - Public API
pub mod vault;
pub mod oracle;
pub mod liquidation;

// Internal modules
mod bitcoin_rpc;
mod database;
mod crypto;
```

### Key Data Structures

```rust
// Vault representation
pub struct Vault {
    pub id: String,
    pub owner: String,
    pub collateral: u64,  // satoshis
    pub debt: f64,        // stable tokens
    pub created_at: DateTime<Utc>,
    pub status: VaultStatus,
}

// Oracle price data
pub struct PriceData {
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub sources: Vec<PriceSource>,
}

// Liquidation event
pub struct Liquidation {
    pub vault_id: String,
    pub collateral: u64,
    pub debt: f64,
    pub penalty: f64,
    pub liquidator: String,
}
```

## Development Workflow

### 1. Feature Development

Create a new feature branch:
```bash
git checkout -b feature/your-feature-name
```

### 2. Code Style

Follow Rust conventions:
```rust
// Good
pub fn calculate_ratio(collateral: u64, debt: f64, price: f64) -> f64 {
    let collateral_value = (collateral as f64) * price / 100_000_000.0;
    collateral_value / debt
}

// Bad
pub fn calc(c: u64, d: f64, p: f64) -> f64 {
    (c as f64) * p / 100000000.0 / d
}
```

### 3. Error Handling

Use Result types consistently:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Insufficient collateral: required {required}, got {actual}")]
    InsufficientCollateral { required: u64, actual: u64 },
    
    #[error("Vault not found: {0}")]
    NotFound(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
}

pub fn create_vault(collateral: u64, debt: f64) -> Result<Vault, VaultError> {
    if collateral < calculate_min_collateral(debt) {
        return Err(VaultError::InsufficientCollateral {
            required: calculate_min_collateral(debt),
            actual: collateral,
        });
    }
    // ... vault creation logic
    Ok(vault)
}
```

### 4. Logging

Use structured logging:
```rust
use log::{info, warn, error, debug};

pub fn process_liquidation(vault: &Vault) {
    info!("Processing liquidation for vault {}", vault.id);
    debug!("Vault details: collateral={}, debt={}", 
           vault.collateral, vault.debt);
    
    match execute_liquidation(vault) {
        Ok(result) => info!("Liquidation successful: {:?}", result),
        Err(e) => error!("Liquidation failed: {}", e),
    }
}
```

## Testing

### Unit Tests

Write tests in the same file:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_ratio() {
        let collateral = 10_000_000; // 0.1 BTC
        let debt = 4500.0;
        let price = 45000.0;
        
        let ratio = calculate_ratio(collateral, debt, price);
        assert_eq!(ratio, 1.0);
    }
    
    #[test]
    #[should_panic(expected = "InsufficientCollateral")]
    fn test_insufficient_collateral() {
        create_vault(1000, 1000.0).unwrap();
    }
}
```

### Integration Tests

Create tests in `tests/` directory:
```rust
// tests/integration.rs
use bitstable::{Vault, Oracle};

#[tokio::test]
async fn test_vault_lifecycle() {
    // Setup
    let db = setup_test_db().await;
    let oracle = Oracle::new_mock(45000.0);
    
    // Create vault
    let vault = Vault::create(
        &db,
        "tb1q...",
        10_000_000,
        4500.0
    ).await.unwrap();
    
    // Add collateral
    vault.add_collateral(5_000_000).await.unwrap();
    assert_eq!(vault.collateral, 15_000_000);
    
    // Cleanup
    cleanup_test_db(db).await;
}
```

### Test Coverage

```bash
# Run tests with coverage
cargo tarpaulin --out Html

# View coverage report
open tarpaulin-report.html
```

## Debugging

### 1. Enable Debug Logging

```bash
RUST_LOG=debug cargo run
```

### 2. Use Debug Prints

```rust
dbg!(&vault);  // Prints to stderr with file:line
println!("Vault: {:?}", vault);  // Standard debug output
```

### 3. Interactive Debugging with LLDB

```bash
# Install lldb
brew install llvm  # macOS
sudo apt-get install lldb  # Linux

# Build with debug symbols
cargo build

# Debug with lldb
rust-lldb target/debug/bitstable
(lldb) breakpoint set -n create_vault
(lldb) run
```

### 4. Performance Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Profile the application
cargo flamegraph --bin bitstable

# View the flamegraph
open flamegraph.svg
```

## Contributing

### Code Review Checklist

Before submitting a PR, ensure:

- [ ] All tests pass: `cargo test`
- [ ] Code is formatted: `cargo fmt`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Documentation updated: `cargo doc --open`
- [ ] Changelog updated
- [ ] Security considerations addressed

### Commit Messages

Follow conventional commits:
```
feat: add liquidation webhook support
fix: correct collateral calculation overflow
docs: update API documentation
test: add integration tests for oracle
refactor: simplify vault state machine
perf: optimize database queries
```

### Pull Request Process

1. Create feature branch
2. Make changes with tests
3. Update documentation
4. Submit PR with description
5. Address review feedback
6. Merge after approval

## Deployment

### 1. Production Build

```bash
# Optimize for production
cargo build --release

# Strip debug symbols
strip target/release/bitstable

# Verify binary
./target/release/bitstable --version
```

### 2. Configuration Management

```bash
# Production config
cat > .env.production <<EOF
RUST_LOG=info
NETWORK=mainnet
DATABASE_PATH=/var/lib/bitstable/db
DATABASE_BACKUP_PATH=/var/lib/bitstable/backups
BITCOIN_RPC_URL=http://localhost:8332
API_PORT=8080
EOF
```

### 3. Database Migration

```rust
// src/bin/migrate.rs
use bitstable::database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = database::open("./data/bitstable.db")?;
    
    // Run migrations
    database::migrate(&db)?;
    
    println!("Migration completed successfully");
    Ok(())
}
```

### 4. Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bitstable /usr/local/bin/
COPY --from=builder /app/.env.production /app/.env

WORKDIR /app
CMD ["bitstable"]
```

### 5. Monitoring

```rust
// Add metrics endpoint
use prometheus::{Encoder, TextEncoder, Counter, Gauge};

lazy_static! {
    static ref VAULT_COUNTER: Counter = Counter::new(
        "bitstable_vaults_total", "Total number of vaults"
    ).unwrap();
    
    static ref COLLATERAL_GAUGE: Gauge = Gauge::new(
        "bitstable_collateral_btc", "Total BTC locked"
    ).unwrap();
}

pub fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
```

### 6. Health Checks

```rust
pub async fn health_check() -> Result<HealthStatus, Error> {
    let mut status = HealthStatus::default();
    
    // Check Bitcoin connection
    status.bitcoin = bitcoin_rpc::ping().await.is_ok();
    
    // Check database
    status.database = database::ping().is_ok();
    
    // Check oracle
    status.oracle = oracle::get_price().await.is_ok();
    
    if !status.bitcoin || !status.database || !status.oracle {
        return Err(Error::Unhealthy(status));
    }
    
    Ok(status)
}
```

## Advanced Topics

### 1. Threshold Signatures (FROST)

```rust
// Future implementation
use frost_secp256k1 as frost;

pub async fn setup_frost_signing(
    participants: Vec<PublicKey>,
    threshold: u16,
) -> Result<SigningGroup, Error> {
    // Key generation ceremony
    let (shares, pubkey) = frost::keygen(participants, threshold)?;
    
    // Distribute shares securely
    for (participant, share) in shares {
        send_encrypted_share(participant, share).await?;
    }
    
    Ok(SigningGroup { pubkey, threshold })
}
```

### 2. WebSocket Implementation

```rust
use tokio_tungstenite::{accept_async, tungstenite::Message};

pub async fn websocket_handler(stream: TcpStream) {
    let ws_stream = accept_async(stream).await.unwrap();
    let (tx, rx) = ws_stream.split();
    
    // Subscribe to events
    let mut price_rx = oracle::subscribe_prices();
    
    tokio::spawn(async move {
        while let Ok(price) = price_rx.recv().await {
            let msg = Message::text(json!({
                "type": "price_update",
                "price": price
            }).to_string());
            tx.send(msg).await.ok();
        }
    });
}
```

### 3. Performance Optimization

```rust
// Use batch operations
pub async fn batch_update_vaults(updates: Vec<VaultUpdate>) -> Result<(), Error> {
    let batch = database::batch();
    
    for update in updates {
        batch.insert(
            format!("vault:{}", update.id),
            bincode::serialize(&update)?
        );
    }
    
    database::apply_batch(batch)?;
    Ok(())
}

// Cache frequently accessed data
use lru::LruCache;

lazy_static! {
    static ref VAULT_CACHE: Mutex<LruCache<String, Vault>> = 
        Mutex::new(LruCache::new(1000));
}
```

## Resources

### Documentation
- [Rust Book](https://doc.rust-lang.org/book/)
- [Async Rust](https://rust-lang.github.io/async-book/)
- [Bitcoin Developer Guide](https://developer.bitcoin.org/)

### Tools
- [cargo-watch](https://github.com/watchexec/cargo-watch) - Auto-rebuild
- [cargo-audit](https://github.com/RustSec/cargo-audit) - Security audits
- [cargo-flamegraph](https://github.com/flamegraph-rs/flamegraph) - Performance profiling

### Community
- Discord: https://discord.gg/bitstable
- GitHub Discussions: https://github.com/bitstable/bitstable/discussions
- Stack Overflow: [bitstable] tag
