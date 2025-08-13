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
2. **Oracle Price Feeds**: Decentralized price consensus from multiple exchanges
3. **Market Liquidations**: Anyone can liquidate unhealthy vaults for profit
4. **Stability Fees**: Small annual fee (2%) to stabilize the peg

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

# List all vaults
cargo run --bin bitstable-cli -- vault list

# Scan for liquidation opportunities
cargo run --bin bitstable-cli -- liquidate scan
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

### Oracle Network
- **Threshold Consensus**: 3 of 5 oracles must agree
- **Price Sources**: Coinbase, Binance, Kraken, Bitstamp, CoinGecko
- **Update Frequency**: 30 seconds

### Liquidation Engine
- **Liquidation Bonus**: 5% of debt value
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
# Collateral ratio: 159.49%
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
- **Oracle Security**: Requires 3 of 5 oracles for price consensus
- **Liquidation Incentives**: Market forces maintain system health
- **Open Source**: All code is auditable

## Comparison with Other Stablecoins

| Feature | BitStable | DAI | USDC | Tether |
|---------|-----------|-----|------|--------|
| Decentralized | ✅ | ⚠️ | ❌ | ❌ |
| Censorship Resistant | ✅ | ⚠️ | ❌ | ❌ |
| Backed by Bitcoin | ✅ | ❌ | ❌ | ❌ |
| No Admin Keys | ✅ | ❌ | ❌ | ❌ |
| Permissionless Liquidation | ✅ | ✅ | ❌ | ❌ |

## Technical Details

### Collateral Management
- Dynamic collateral ratios based on market conditions
- Automatic liquidation below threshold
- No governance required for parameter updates

### Price Oracle
- Multiple independent data sources
- Median price calculation for consensus
- Circuit breakers for extreme price movements

### Liquidation Mechanics
- First-come-first-served liquidation queue
- Profit incentives align liquidator behavior
- No time delays or auction periods

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

MIT OR Apache-2.0

## Support

- GitHub Issues: https://github.com/StevenGeller/bitstable/issues
- Documentation: See `/docs` directory

---

**⚠️ Testnet Only**: This implementation is for Bitcoin testnet only. DO NOT use with real Bitcoin on mainnet without proper security audits.

**🚀 BitStable**: True Bitcoin-native stable value without central authority.