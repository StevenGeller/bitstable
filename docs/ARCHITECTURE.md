# Architecture Guide

## System Overview

BitStable is a decentralized stablecoin protocol built on Bitcoin, implementing an overcollateralized vault system where users lock Bitcoin as collateral to mint stable tokens pegged to USD.

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                         Users                                 │
│                    (Vault Owners, Liquidators)                │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                      API Gateway                              │
│                  (REST API, WebSocket)                        │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                    Core Services                              │
├────────────────────┬─────────────────┬──────────────────────┤
│   Vault Manager    │  Oracle Service │ Liquidation Engine   │
└────────────────────┴─────────────────┴──────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                     Data Layer                                │
├────────────────────┬─────────────────┬──────────────────────┤
│   Sled Database    │  Bitcoin Node   │  External APIs       │
└────────────────────┴─────────────────┴──────────────────────┘
```

## Core Components

### 1. Vault Manager

The Vault Manager handles the lifecycle of collateralized debt positions.

```rust
pub struct VaultManager {
    database: Arc<Database>,
    bitcoin_client: Arc<BitcoinRPC>,
    oracle: Arc<Oracle>,
}
```

**Responsibilities:**
- Vault creation and validation
- Collateral management
- Debt issuance and repayment
- Health monitoring
- State transitions

**Key Operations:**
```rust
impl VaultManager {
    pub async fn create_vault(&self, owner: String, collateral: u64, debt: f64) -> Result<Vault>;
    pub async fn add_collateral(&self, vault_id: &str, amount: u64) -> Result<()>;
    pub async fn withdraw_collateral(&self, vault_id: &str, amount: u64) -> Result<()>;
    pub async fn mint_stable(&self, vault_id: &str, amount: f64) -> Result<()>;
    pub async fn repay_debt(&self, vault_id: &str, amount: f64) -> Result<()>;
    pub async fn close_vault(&self, vault_id: &str) -> Result<()>;
}
```

### 2. Oracle Service

Aggregates price feeds from multiple sources to provide reliable BTC/USD pricing.

```rust
pub struct Oracle {
    sources: Vec<Box<dyn PriceSource>>,
    cache: Arc<RwLock<PriceCache>>,
    threshold: usize,  // Minimum sources required
}
```

**Price Aggregation Strategy:**
1. Fetch prices from multiple exchanges (Binance, Coinbase, Kraken)
2. Remove outliers (> 2% deviation from median)
3. Calculate median of remaining prices
4. Cache result with timestamp

**Reliability Features:**
- Fallback sources
- Outlier detection
- Staleness checks
- Historical price tracking

### 3. Liquidation Engine

Monitors vault health and executes liquidations when necessary.

```rust
pub struct LiquidationEngine {
    vault_manager: Arc<VaultManager>,
    oracle: Arc<Oracle>,
    auction_house: Arc<AuctionHouse>,
}
```

**Liquidation Process:**
```
1. Monitor all vaults continuously
2. Detect unhealthy vaults (ratio < 150%)
3. Initiate liquidation auction
4. Apply 13% penalty
5. Distribute proceeds
6. Update vault status
```

### 4. Bitcoin Integration

Handles all Bitcoin blockchain interactions.

```rust
pub struct BitcoinRPC {
    client: bitcoincore_rpc::Client,
    network: Network,
}
```

**Key Functions:**
- Transaction creation and signing
- Multisig address generation
- UTXO management
- Block monitoring
- Fee estimation

### 5. Database Layer

Persistent storage using Sled embedded database.

```rust
pub struct Database {
    db: sled::Db,
    vaults_tree: sled::Tree,
    prices_tree: sled::Tree,
    liquidations_tree: sled::Tree,
}
```

**Data Models:**
```rust
// Vault storage
Key: "vault:{vault_id}"
Value: Vault (serialized)

// Price history
Key: "price:{timestamp}"
Value: PriceData (serialized)

// Liquidation records
Key: "liquidation:{vault_id}:{timestamp}"
Value: Liquidation (serialized)
```

## Security Architecture

### 1. Multisig Custody

All Bitcoin collateral is held in 2-of-3 multisig addresses:

```rust
pub struct MultisigConfig {
    user_key: PublicKey,      // User controls
    protocol_key: PublicKey,   // Protocol controls
    backup_key: PublicKey,     // Emergency recovery
    threshold: u8,             // 2-of-3
}
```

### 2. Cryptographic Security

```rust
pub struct CryptoModule {
    // Key derivation
    pub fn derive_keys(seed: &[u8]) -> (SecretKey, PublicKey);
    
    // Signature generation
    pub fn sign_message(key: &SecretKey, message: &[u8]) -> Signature;
    
    // Threshold signatures (FROST ready)
    pub fn threshold_sign(shares: Vec<Share>, message: &[u8]) -> Signature;
}
```

### 3. Access Control

```rust
pub enum Permission {
    CreateVault,
    ModifyVault,
    Liquidate,
    AdminAccess,
}

pub struct AccessControl {
    pub fn check_permission(user: &User, action: Permission) -> Result<()>;
    pub fn verify_ownership(user: &User, vault: &Vault) -> Result<()>;
}
```

## Data Flow

### Vault Creation Flow

```
User Request → API Gateway → Vault Manager
    ↓
Validate Collateral Ratio
    ↓
Generate Multisig Address
    ↓
Create Database Entry
    ↓
Wait for Bitcoin Deposit
    ↓
Confirm Transaction
    ↓
Mint Stable Tokens
    ↓
Update Vault Status → Return to User
```

### Price Update Flow

```
External APIs → Oracle Service
    ↓
Aggregate & Validate
    ↓
Update Price Cache
    ↓
Broadcast via WebSocket
    ↓
Trigger Health Checks
    ↓
Update Database
```

### Liquidation Flow

```
Price Update → Liquidation Engine
    ↓
Scan All Vaults
    ↓
Identify Unhealthy Vaults
    ↓
Create Liquidation Auction
    ↓
Execute Best Bid
    ↓
Transfer Collateral
    ↓
Repay Debt
    ↓
Distribute Penalty
    ↓
Update Vault Status
```

## State Management

### Vault States

```rust
pub enum VaultStatus {
    Pending,      // Awaiting initial deposit
    Active,       // Normal operation
    Warning,      // Approaching liquidation
    Liquidating,  // In liquidation process
    Liquidated,   // Liquidation complete
    Closed,       // Voluntarily closed
}
```

### State Transitions

```
Pending → Active: Initial deposit confirmed
Active → Warning: Ratio falls below 160%
Warning → Active: Collateral added
Warning → Liquidating: Ratio falls below 150%
Liquidating → Liquidated: Liquidation complete
Active → Closed: All debt repaid
```

## Scalability Considerations

### 1. Database Optimization

- **Indexing**: Secondary indices for common queries
- **Caching**: LRU cache for frequently accessed vaults
- **Batching**: Batch writes for bulk operations
- **Sharding**: Future support for data partitioning

### 2. Concurrent Processing

```rust
// Parallel vault processing
pub async fn process_vaults_parallel(vaults: Vec<Vault>) {
    let futures: Vec<_> = vaults.into_iter()
        .map(|vault| tokio::spawn(process_vault(vault)))
        .collect();
    
    futures::future::join_all(futures).await;
}
```

### 3. Event-Driven Architecture

```rust
pub enum Event {
    VaultCreated(Vault),
    CollateralAdded(String, u64),
    PriceUpdated(f64),
    LiquidationTriggered(String),
}

pub struct EventBus {
    subscribers: HashMap<EventType, Vec<Subscriber>>,
}
```

## Monitoring & Observability

### 1. Metrics Collection

```rust
pub struct Metrics {
    vault_count: Counter,
    total_collateral: Gauge,
    liquidation_rate: Histogram,
    api_latency: Histogram,
}
```

### 2. Logging Strategy

```rust
// Structured logging
info!("Vault created"; 
    "vault_id" => vault.id, 
    "collateral" => vault.collateral,
    "debt" => vault.debt
);
```

### 3. Health Checks

```rust
pub struct HealthCheck {
    bitcoin_node: bool,
    database: bool,
    oracle_feeds: Vec<(String, bool)>,
    last_block: u64,
}
```

## Disaster Recovery

### 1. Backup Strategy

- **Database**: Hourly snapshots to remote storage
- **Configuration**: Version controlled
- **Keys**: Hardware security module (HSM) for production

### 2. Failover Mechanism

```rust
pub struct Failover {
    primary: Service,
    secondary: Service,
    health_checker: HealthChecker,
}

impl Failover {
    pub async fn get_active(&self) -> &Service {
        if self.health_checker.is_healthy(&self.primary).await {
            &self.primary
        } else {
            &self.secondary
        }
    }
}
```

## Future Enhancements

### 1. Layer 2 Integration

- Lightning Network for instant settlements
- State channels for high-frequency operations

### 2. Cross-chain Support

- Ethereum bridge for stable token
- Polygon/Arbitrum deployment

### 3. Advanced Features

- Flash loans
- Yield farming
- Governance token
- Insurance fund

## Performance Benchmarks

### Target Metrics

| Operation | Target Latency | Throughput |
|-----------|---------------|------------|
| Create Vault | < 100ms | 100/sec |
| Price Update | < 10ms | 1000/sec |
| Health Check | < 50ms | 500/sec |
| Liquidation | < 500ms | 50/sec |

### Optimization Techniques

1. **Connection Pooling**: Reuse Bitcoin RPC connections
2. **Query Optimization**: Indexed database queries
3. **Caching**: Redis for hot data
4. **Async I/O**: Non-blocking operations throughout

## Testing Strategy

### 1. Unit Tests

```rust
#[test]
fn test_collateral_ratio_calculation() {
    let vault = Vault {
        collateral: 10_000_000,
        debt: 4500.0,
        ..Default::default()
    };
    
    let ratio = vault.calculate_ratio(45000.0);
    assert_eq!(ratio, 100.0);
}
```

### 2. Integration Tests

```rust
#[tokio::test]
async fn test_full_vault_lifecycle() {
    let system = TestSystem::new().await;
    let vault = system.create_vault(10_000_000, 4500.0).await;
    
    system.add_collateral(&vault.id, 5_000_000).await;
    system.repay_debt(&vault.id, 4500.0).await;
    system.close_vault(&vault.id).await;
    
    assert_eq!(vault.status, VaultStatus::Closed);
}
```

### 3. Stress Testing

```bash
# Load testing with k6
k6 run --vus 100 --duration 30s stress_test.js
```

## Deployment Architecture

### Production Setup

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Load       │────▶│   API       │────▶│   Core      │
│  Balancer   │     │   Servers   │     │   Services  │
└─────────────┘     └─────────────┘     └─────────────┘
                            │                    │
                            ▼                    ▼
                    ┌─────────────┐     ┌─────────────┐
                    │   Redis     │     │  PostgreSQL │
                    │   Cache     │     │   Database  │
                    └─────────────┘     └─────────────┘
                                                │
                                                ▼
                                        ┌─────────────┐
                                        │   Bitcoin   │
                                        │    Node     │
                                        └─────────────┘
```

## Conclusion

BitStable's architecture prioritizes:
- **Security**: Multisig custody, threshold signatures
- **Reliability**: Redundant oracles, health monitoring
- **Scalability**: Async processing, efficient storage
- **Maintainability**: Modular design, comprehensive testing

The system is designed to handle thousands of vaults while maintaining sub-second response times and ensuring the safety of user funds.
