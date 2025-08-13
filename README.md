# BitStable

A truly decentralized stable value protocol on Bitcoin using market-based liquidations and oracle consensus.

## Core Principles

- **No trusted parties** - Only math and incentives
- **Permissionless** - Anyone can participate  
- **Censorship resistant** - No blacklists possible
- **Self-custodial** - Users control their keys always
- **Liquidation-based** - Market incentives maintain stability

## How It Works

BitStable creates stable value backed by Bitcoin collateral through:

1. **Overcollateralized Vaults**: Users deposit Bitcoin (150% minimum) to mint stable value
2. **Multisig Custody**: Trustless Bitcoin escrow with 2-of-3 multisig contracts
3. **Oracle Price Feeds**: Decentralized price consensus with threshold signatures
4. **Market Liquidations**: Anyone can liquidate unhealthy vaults for profit
5. **Stability Fees**: Small annual fee (2%) to stabilize the peg

## Quick Start

### Installation

```bash
# Install Rust if you haven't already
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/StevenGeller/bitstable.git
cd bitstable
cargo build --release
```

### Basic Usage

```bash
# Check protocol status
cargo run --bin bitstable-cli -- status

# Get current Bitcoin price
cargo run --bin bitstable-cli -- oracle price

# Create a vault (deposit 0.2 BTC to mint $15,000 stable value)
cargo run --bin bitstable-cli -- vault create \
  --collateral-btc 0.2 \
  --stable-amount 15000 \
  --owner YOUR_PUBKEY

# Fund the vault's escrow contract
cargo run --bin bitstable-cli -- vault fund VAULT_ID \
  --txid FUNDING_TXID \
  --vout 0 \
  --amount 0.2

# Check escrow contract details
cargo run --bin bitstable-cli -- vault escrow VAULT_ID

# List all vaults
cargo run --bin bitstable-cli -- vault list

# Scan for liquidation opportunities
cargo run --bin bitstable-cli -- liquidate scan

# Check custody system status
cargo run --bin bitstable-cli -- custody stats
```

### Running Network Nodes

#### Oracle Node
Provides price feeds to the network:

```bash
cargo run --bin oracle-node
```

#### Liquidator Bot
Automatically liquidates unhealthy vaults:

```bash
cargo run --bin liquidator-bot \
  --liquidator-key YOUR_PUBKEY \
  --min-profit-btc 0.001 \
  --scan-interval 30
```

## Architecture

### Vault System
- **Minimum Collateral Ratio**: 150%
- **Liquidation Threshold**: 110%
- **Stability Fee**: 2% APR

### Bitcoin Custody System
- **Multisig Escrow**: 2-of-3 signatures required (vault owner + protocol)
- **P2WSH Addresses**: SegWit-native multisig for efficiency
- **Trustless Settlements**: Bitcoin transactions enforce liquidations
- **SIGHASH_ALL**: Proper transaction signing for security

### Oracle Network
- **Threshold Consensus**: 3 of 5 oracles must agree
- **Price Sources**: Coinbase, Binance, Kraken, Bitstamp, CoinGecko
- **Cryptographic Signatures**: ECDSA signatures on price data
- **Threshold Signatures**: Aggregated signatures for consensus
- **Update Frequency**: 30 seconds

### Liquidation Engine
- **On-Chain Settlement**: Bitcoin transactions execute liquidations
- **Liquidation Bonus**: 5% of debt value
- **Protocol Fees**: 1% of liquidated amount
- **Permissionless**: Anyone can liquidate
- **Immediate**: No waiting periods

## Example: Creating Stable Value

```bash
# Current BTC price: $119,600
# Want to create: $15,000 stable value
# Required collateral: $15,000 × 1.5 = $22,500
# BTC needed: $22,500 / $119,600 = 0.188 BTC

cargo run --bin bitstable-cli -- vault create \
  --collateral-btc 0.2 \
  --stable-amount 15000 \
  --owner 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798

# Output:
# ✅ Created vault: fd559ae2e790b87f...
# Collateral: 0.2 BTC
# Stable debt: $15000
# 🔐 Escrow Address: bc1qwqdg6squsna38e46795at95yu9atm8azzmyvckulcc7kytlcckxswvvzej
# Send 0.2 BTC to this address to fund the vault
```

## Risk Management

### For Vault Owners
- Monitor collateral ratio regularly
- Add collateral or repay debt if ratio approaches 110%
- Use `vault show VAULT_ID` to check health

### For Liquidators
- Profit from liquidating unhealthy vaults
- 5% bonus on liquidated collateral
- Automated bots can maximize efficiency

## Network Health Monitoring

```bash
# Check oracle consensus
cargo run --bin bitstable-cli -- oracle status

# View liquidation statistics
cargo run --bin bitstable-cli -- liquidate stats

# Monitor vault health
cargo run --bin bitstable-cli -- vault list --liquidatable

# Check custody system health
cargo run --bin bitstable-cli -- custody contracts

# Monitor Bitcoin settlements
cargo run --bin bitstable-cli -- custody settlements
```

## Configuration

Default configuration is optimized for Bitcoin testnet. For mainnet:

```toml
[protocol]
network = "mainnet"
min_collateral_ratio = 1.5
liquidation_threshold = 1.1
stability_fee_apr = 0.02
```

## Security

- **No Admin Keys**: Protocol is fully decentralized
- **Bitcoin Custody**: Trustless multisig escrow with on-chain settlements
- **Oracle Security**: Cryptographic signatures and threshold consensus
- **Liquidation Incentives**: Market forces maintain system health
- **Open Source**: All code is auditable and public domain

## Comparison with Other Stablecoins

| Feature | BitStable | DAI | USDC | Tether |
|---------|-----------|-----|------|--------|
| Decentralized | ✅ | ⚠️ | ❌ | ❌ |
| Censorship Resistant | ✅ | ⚠️ | ❌ | ❌ |
| Backed by Bitcoin | ✅ | ❌ | ❌ | ❌ |
| No Admin Keys | ✅ | ❌ | ❌ | ❌ |
| Permissionless Liquidation | ✅ | ✅ | ❌ | ❌ |

## Technical Details

### Bitcoin Custody System
- **Multisig Escrow Contracts**: 2-of-3 signatures (vault owner + protocol keys)
- **P2WSH Addresses**: Native SegWit for lower fees and better scaling
- **Trustless Liquidation Settlement**: Bitcoin transactions enforce liquidations
- **UTXO Management**: Efficient tracking of collateral and settlements

### Collateral Management
- Dynamic collateral ratios based on market conditions
- Automatic liquidation below threshold through Bitcoin transactions
- No governance required for parameter updates
- Real Bitcoin custody with cryptographic guarantees

### Price Oracle System
- **Cryptographic Signatures**: Each oracle signs price data with ECDSA
- **Threshold Signatures**: Aggregated signatures prove consensus
- **Multiple Data Sources**: Coinbase, Binance, Kraken, Bitstamp, CoinGecko
- **Median Price Calculation**: Robust against outliers and manipulation
- **Signature Verification**: All price data is cryptographically verified

### Liquidation Mechanics
- **On-Chain Settlement**: Bitcoin transactions execute liquidations
- **First-come-first-served**: No auction delays or governance intervention
- **Cryptographic Enforcement**: Multisig contracts ensure trustless execution
- **Profit incentives**: 5% bonus aligns liquidator behavior with system health
- **No time delays**: Immediate liquidation when threshold is breached

## Development

### Building from Source
```bash
cargo build
cargo test
cargo run --bin bitstable-cli -- help
```

### Contributing
1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## License

This software is released into the public domain under The Unlicense.
See UNLICENSE file for details.

## Support

- GitHub Issues: https://github.com/StevenGeller/bitstable/issues
- Documentation: See `/docs` directory

---

**⚠️ Testnet Only**: This implementation is for Bitcoin testnet only. DO NOT use with real Bitcoin on mainnet without proper security audits.

**🚀 BitStable**: True Bitcoin-native stable value without central authority.