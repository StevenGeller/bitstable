# Bitcoin Testnet Setup Guide for BitStable

This guide explains how to set up and run BitStable with **real Bitcoin testnet** integration for end-to-end testing with actual on-chain transactions.

## 🚨 Important Notice

**This integration uses REAL Bitcoin testnet transactions!**
- Real multisig addresses are created
- Real Bitcoin transactions are built and broadcast
- Real UTXOs are spent on the Bitcoin testnet
- All operations are on-chain and permanent

## Prerequisites

### 1. Bitcoin Core Node Setup

You need a running Bitcoin Core node connected to testnet:

#### Installation
```bash
# Download Bitcoin Core from https://bitcoincore.org/en/download/
# Or using package manager:
brew install bitcoin  # macOS
sudo apt install bitcoin-core  # Ubuntu
```

#### Configuration
Create `~/.bitcoin/bitcoin.conf`:
```ini
# Enable testnet
testnet=1

# Enable RPC server
server=1
rpcuser=bitcoin
rpcpassword=password
rpcallowip=127.0.0.1

# Optional: reduce bandwidth usage
prune=2000

# Optional: faster sync for testing
assumevalid=0000000000000000000000000000000000000000000000000000000000000000
```

#### Start Bitcoin Core
```bash
# Start daemon
bitcoind -daemon

# Check status
bitcoin-cli -testnet getblockchaininfo

# Get new testnet address
bitcoin-cli -testnet getnewaddress
```

### 2. Testnet Bitcoin Funding

Get testnet Bitcoin from faucets:
- [CoinFaucet.eu](https://coinfaucet.eu/en/btc-testnet/)
- [Testnet Faucet](https://testnet-faucet.com/btc-testnet)
- [Bitcoin Testnet Sandbox](https://bitcoinfaucet.uo1.net/)

## BitStable Real Bitcoin Integration

### Architecture Overview

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   BitStable     │────│  Bitcoin Core    │────│  Bitcoin        │
│   Protocol      │    │  Testnet Node    │    │  Testnet        │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         │              RPC Calls (JSON-RPC)             │
         │                       │                       │
    ┌─────────┐                  │                  ┌─────────┐
    │ Custody │            ┌─────────────┐          │ Network │
    │ Manager │            │ Real UTXOs  │          │ Mempool │
    └─────────┘            │ Real Addrs  │          │  Blocks │
                           │ Real Txns   │          └─────────┘
                           └─────────────┘
```

### Real Bitcoin Features Implemented

#### 1. **Real Multisig Escrow Contracts**
```rust
// Creates actual 2-of-3 multisig P2WSH addresses
let (escrow_address, script) = bitcoin_client.create_escrow_multisig(
    user_pubkey,     // User can spend
    oracle_pubkey,   // Oracle for liquidations  
    liquidator_key   // Liquidator for liquidations
)?;
```

#### 2. **Real Transaction Building & Signing**
```rust
// Builds real Bitcoin transactions with proper inputs/outputs
let funding_tx = bitcoin_client.build_funding_transaction(
    source_utxos,        // Real UTXOs from Bitcoin testnet
    source_private_key,  // Real private key for signing
    escrow_address,      // Real multisig address
    amount,              // Real BTC amount
    fee_rate            // Real network fee rate
)?;
```

#### 3. **Real Network Broadcasting**
```rust
// Broadcasts to actual Bitcoin testnet network
let txid = bitcoin_client.broadcast_transaction(&tx)?;
// Transaction appears in Bitcoin testnet mempool and blocks
```

#### 4. **Real UTXO Management**
```rust
// Queries real Bitcoin node for spendable outputs
let utxos = bitcoin_client.get_spendable_utxos(address, min_confirmations).await?;
```

## Running the Demos

### 1. Simulation Demo (No Real Bitcoin)
```bash
# Uses mock data and simulated operations
./run_comprehensive_test.sh
```

### 2. Real Bitcoin Testnet Demo
```bash
# Uses REAL Bitcoin testnet transactions!
./run_real_bitcoin_testnet_demo.sh
```

## Demo Walkthrough

The real Bitcoin testnet demo demonstrates:

### Step 1: Bitcoin Node Connection
- Connects to your local Bitcoin Core testnet node
- Verifies RPC access and network status
- Shows current block height, difficulty, and fee rates

### Step 2: Real Address Generation
- Generates real Bitcoin testnet addresses for users
- Uses proper secp256k1 key generation
- Creates P2WPKH (native segwit) addresses

### Step 3: Protocol Initialization
- Initializes BitStable with real Bitcoin client
- Connects custody manager to Bitcoin node
- Sets up oracle and liquidator keys

### Step 4: Live Exchange Rate Feeds
- Fetches real BTC prices from CoinGecko API
- Calculates real USD/EUR/GBP exchange rates
- Updates protocol with current market data

### Step 5: Real Escrow Contract Creation
- Creates actual 2-of-3 multisig P2WSH address
- Generates real Bitcoin script with user/oracle/liquidator keys
- Address can receive real Bitcoin on testnet

### Step 6: Testnet Funding Process
- Shows how to request funds from Bitcoin testnet faucets
- Demonstrates transaction building to fund escrow
- Explains confirmation waiting and UTXO management

### Step 7: Blockchain Monitoring
- Shows how to monitor Bitcoin addresses for funding
- Demonstrates real transaction confirmation checking
- Explains UTXO detection and validation

### Step 8: Real Liquidation Process
- Demonstrates building liquidation transactions
- Shows proper multisig signing with oracle + liquidator keys
- Explains real Bitcoin settlement process

### Step 9: Network Health Monitoring
- Shows Bitcoin node connectivity status
- Demonstrates blockchain synchronization checking
- Explains real network statistics monitoring

## Key Differences from Simulation

| Aspect | Simulation Mode | Real Bitcoin Mode |
|--------|----------------|-------------------|
| Addresses | Mock/Generated | Real testnet addresses |
| Transactions | In-memory only | Broadcast to testnet |
| UTXOs | Simulated | Real Bitcoin UTXOs |
| Confirmations | Instant | Real network timing |
| Fees | Fixed/Mock | Real network fees |
| Multisig | Script only | Real P2WSH addresses |
| Liquidations | Database only | Real Bitcoin settlement |

## Security Considerations

### Testnet Safety
- ✅ Testnet Bitcoin has no monetary value
- ✅ Safe for development and testing
- ✅ Can request unlimited testnet coins from faucets
- ⚠️ Transactions are publicly visible on testnet blockchain

### Private Key Management
- 🔐 Demo generates new keys each run
- 🔐 Keys are only stored in memory during execution
- 🔐 Production should use hardware security modules
- 🔐 Never use testnet keys for mainnet

### Production Considerations
- 🔧 Replace testnet configuration with mainnet
- 🔧 Implement proper key backup and recovery
- 🔧 Add comprehensive error handling and retries
- 🔧 Implement fee optimization strategies
- 🔧 Add multi-node redundancy for Bitcoin connections

## Troubleshooting

### Bitcoin Core Issues
```bash
# Check if Bitcoin Core is running
ps aux | grep bitcoind

# Check testnet connection
bitcoin-cli -testnet getblockchaininfo

# Check RPC connectivity
curl -u bitcoin:password --data-binary '{"jsonrpc":"1.0","id":"test","method":"getblockcount","params":[]}' -H 'content-type: text/plain;' http://127.0.0.1:18332/
```

### Sync Issues
```bash
# Check sync progress
bitcoin-cli -testnet getblockchaininfo | grep progress

# Check peer connections
bitcoin-cli -testnet getpeerinfo | wc -l
```

### RPC Authentication
- Verify username/password in `bitcoin.conf`
- Check `rpcallowip` settings
- Ensure Bitcoin Core is listening on port 18332

## Development Workflow

1. **Setup**: Start Bitcoin Core testnet node
2. **Fund**: Get testnet BTC from faucets  
3. **Test**: Run BitStable with real Bitcoin integration
4. **Monitor**: Watch transactions on testnet explorer
5. **Debug**: Check Bitcoin Core logs for issues

## Production Deployment

To deploy on Bitcoin mainnet:

1. Change `Network::Testnet` to `Network::Bitcoin`
2. Update RPC port from 18332 to 8332
3. Use mainnet Bitcoin addresses and transactions
4. Implement proper security measures for mainnet
5. Add comprehensive monitoring and alerting

## Resources

- [Bitcoin Core Documentation](https://bitcoincore.org/en/doc/)
- [Bitcoin Testnet Explorer](https://blockstream.info/testnet/)
- [Bitcoin RPC API Reference](https://developer.bitcoin.org/reference/rpc/)
- [BitStable Protocol Whitepaper](./whitepaper.md)

---

**⚠️ Remember: This uses REAL Bitcoin testnet. All transactions are permanent and visible on the blockchain!**