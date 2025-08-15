# BitStable Regtest Automation

This document explains how to use BitStable with Bitcoin regtest for fully automated testing and development.

## What is Regtest?

Regtest (Regression Test Mode) is a local Bitcoin network that you fully control:

- ✅ **Instant mining** - Generate blocks on demand
- ✅ **No external dependencies** - Completely local
- ✅ **Unlimited funds** - Mine as much Bitcoin as needed
- ✅ **Fast confirmations** - No waiting for network
- ✅ **Reset anytime** - Start fresh whenever needed

Perfect for development, testing, and demonstrations!

## Quick Start

### 1. Start Bitcoin Regtest

Use our automated setup script:

```bash
./scripts/start_regtest.sh
```

Or manually:

```bash
bitcoind -regtest -daemon -rpcuser=bitstable -rpcpassword=password
```

### 2. Run Automated Demo

```bash
cargo run --example automated_regtest_demo
```

This will automatically:
- Connect to regtest node
- Generate Bitcoin addresses
- Mine blocks to create funds
- Create multisig escrow contracts
- Execute real Bitcoin transactions
- Demonstrate liquidation mechanics

## Examples

### Simple Regtest Operations

```bash
cargo run --example simple_regtest_example
```

Shows basic regtest automation:
- Address generation
- Automatic fund creation via mining
- Balance checking

### Full BitStable Protocol Demo

```bash
cargo run --example automated_regtest_demo
```

Complete end-to-end demonstration:
- Full protocol initialization
- Real Bitcoin multisig escrow
- Transaction building and broadcasting
- Price simulation and liquidation

## Code Examples

### Basic Regtest Client

```rust
use bitstable::BitcoinClient;

#[tokio::main]
async fn main() -> bitstable::Result<()> {
    // Connect to regtest
    let client = BitcoinClient::regtest(
        "http://127.0.0.1:18443", 
        "bitstable", 
        "password"
    )?;
    
    // Generate an address
    let (address, _private_key) = client.generate_address()?;
    
    // Automatically mine blocks to fund the address
    let balance = client.generate_regtest_funds(&address, 1.0).await?;
    println!("Generated {} BTC!", balance.to_btc());
    
    Ok(())
}
```

### Mining Blocks

```rust
// Mine 10 blocks to an address
let block_hashes = client.mine_blocks(10, &address).await?;

// Confirm pending transactions
client.confirm_transactions(1).await?;
```

### Automatic Fund Generation

```rust
// Generate 5.0 BTC for an address
let amount = client.generate_regtest_funds(&address, 5.0).await?;

// This will:
// 1. Calculate blocks needed (5.0 BTC / 50 BTC per block = 1 block)
// 2. Mine at least 101 blocks (for coinbase maturity)
// 3. Return the actual amount generated
```

## Configuration

### Bitcoin Core Setup

Create `~/.bitcoin/bitcoin.conf`:

```ini
# BitStable Regtest Configuration
regtest=1
server=1
rpcuser=bitstable
rpcpassword=password
rpcallowip=127.0.0.1
rpcbind=127.0.0.1
rpcport=18443
fallbackfee=0.00001
```

### BitStable Client

```rust
use bitstable::bitcoin_client::BitcoinConfig;

// Use default regtest configuration
let config = BitcoinConfig::regtest();

// Or customize
let config = BitcoinConfig {
    rpc_url: "http://127.0.0.1:18443".to_string(),
    rpc_username: "bitstable".to_string(),
    rpc_password: "password".to_string(),
    network: Network::Regtest,
    min_confirmations: 1,
    fee_target_blocks: 1,
};
```

## Available Methods

### Mining & Fund Generation

- `mine_blocks(num_blocks, address)` - Mine blocks to specific address
- `generate_regtest_funds(address, amount_btc)` - Auto-generate funds
- `confirm_transactions(num_blocks)` - Confirm pending transactions

### Network Operations

- `get_blockchain_info()` - Network status and stats
- `get_difficulty()` - Current mining difficulty
- `reset_regtest()` - Reset blockchain (requires restart)

### Standard Operations

All standard Bitcoin operations work in regtest:
- `generate_address()` - Create new addresses
- `get_utxos(address)` - Check address balance
- `build_funding_transaction()` - Create transactions
- `broadcast_transaction()` - Send to network
- `create_escrow_multisig()` - Multisig addresses

## Advantages Over Testnet

| Feature | Regtest | Testnet |
|---------|---------|---------|
| Speed | Instant | 10+ minutes |
| Reliability | 100% | Depends on network |
| Cost | Free | Faucet limits |
| Control | Full control | External dependency |
| Reset | Anytime | Never |
| Privacy | Local only | Public network |

## Troubleshooting

### Bitcoin Core Not Starting

```bash
# Check if already running
ps aux | grep bitcoind

# Stop existing instance
bitcoin-cli -regtest stop

# Check logs
tail -f ~/.bitcoin/regtest/debug.log
```

### Connection Failed

```bash
# Test RPC connection
bitcoin-cli -regtest getblockchaininfo

# Check port is listening
netstat -an | grep 18443
```

### Reset Blockchain

```bash
# Stop bitcoind
bitcoin-cli -regtest stop

# Remove blockchain data
rm -rf ~/.bitcoin/regtest/blocks ~/.bitcoin/regtest/chainstate

# Restart
bitcoind -regtest -daemon -rpcuser=bitstable -rpcpassword=password
```

## Integration with CI/CD

Regtest is perfect for automated testing:

```yaml
# GitHub Actions example
- name: Start Bitcoin Regtest
  run: |
    bitcoind -regtest -daemon -rpcuser=test -rpcpassword=test
    sleep 5
    
- name: Run BitStable Tests
  run: cargo test --all-features
  
- name: Run Regtest Demo
  run: cargo run --example automated_regtest_demo
```

## Performance Notes

- **Block generation**: ~1ms per block
- **Transaction confirmation**: Instant with mining
- **Network sync**: Not required (local only)
- **Memory usage**: Minimal (no full blockchain)
- **Disk usage**: <10MB for typical testing

Perfect for development workflows and automated testing pipelines!