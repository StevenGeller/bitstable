# User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Creating Your First Vault](#creating-your-first-vault)
4. [Managing Vaults](#managing-vaults)
5. [Understanding Liquidations](#understanding-liquidations)
6. [Using the API](#using-the-api)
7. [Security Best Practices](#security-best-practices)
8. [FAQ](#faq)

## Introduction

BitStable is a decentralized stablecoin protocol that allows you to:
- Lock Bitcoin as collateral to mint stable tokens
- Maintain a stable value pegged to USD
- Earn from providing liquidity
- Participate in decentralized governance

### Key Concepts

- **Vault**: A smart contract holding your Bitcoin collateral
- **Collateralization Ratio**: The ratio of collateral value to debt (minimum 150%)
- **Stability Fee**: Annual 2% fee on borrowed stable tokens
- **Liquidation**: Automatic sale of collateral when ratio falls below 150%
- **Oracle Price**: Decentralized price feed for BTC/USD

## Getting Started

### 1. Check System Status

Before creating vaults, verify the system is operational:

```bash
# Check health status
curl http://localhost:8080/health

# Get current BTC price
curl http://localhost:8080/api/oracle/price

# Response example:
{
  "price": 45000.00,
  "timestamp": "2024-01-15T10:30:00Z",
  "sources": 3
}
```

### 2. Generate a Wallet

BitStable uses Bitcoin addresses for vault ownership:

```bash
# Generate new address (using Bitcoin Core)
bitcoin-cli -testnet getnewaddress "bitstable_vault"

# Save your address
export VAULT_ADDRESS="tb1q..."
```

### 3. Fund Your Wallet

Get testnet Bitcoin from a faucet:
- https://testnet-faucet.mempool.co/
- https://bitcoinfaucet.uo1.net/

## Creating Your First Vault

### Step 1: Calculate Collateral

To mint 1000 stable tokens at current BTC price of $45,000:

```
Required Collateral = (Stable Amount × 1.5) / BTC Price
Required Collateral = (1000 × 1.5) / 45000
Required Collateral = 0.0333 BTC
```

### Step 2: Create Vault

```bash
curl -X POST http://localhost:8080/api/vault/create \
  -H "Content-Type: application/json" \
  -d '{
    "owner_address": "'$VAULT_ADDRESS'",
    "collateral_satoshis": 3333333,
    "stable_amount": 1000
  }'

# Response:
{
  "vault_id": "vault_abc123",
  "owner": "tb1q...",
  "collateral": 3333333,
  "debt": 1000,
  "collateralization_ratio": 150.0,
  "status": "active",
  "created_at": "2024-01-15T10:35:00Z"
}
```

### Step 3: Send Bitcoin to Vault

```bash
# Get vault deposit address
curl http://localhost:8080/api/vault/vault_abc123/deposit-address

# Send Bitcoin
bitcoin-cli -testnet sendtoaddress "tb1q_vault_address" 0.03333333
```

### Step 4: Confirm Deposit

```bash
# Check vault status
curl http://localhost:8080/api/vault/vault_abc123

# Wait for confirmations (usually 3 blocks)
```

## Managing Vaults

### View Your Vaults

```bash
# List all your vaults
curl http://localhost:8080/api/vaults?owner=$VAULT_ADDRESS

# Get specific vault details
curl http://localhost:8080/api/vault/vault_abc123
```

### Add Collateral

Improve your vault's health by adding more Bitcoin:

```bash
curl -X POST http://localhost:8080/api/vault/vault_abc123/add-collateral \
  -H "Content-Type: application/json" \
  -d '{
    "satoshis": 1000000
  }'
```

### Withdraw Excess Collateral

If your ratio is above 150%, withdraw excess:

```bash
# Calculate withdrawable amount
curl http://localhost:8080/api/vault/vault_abc123/withdrawable

# Withdraw collateral
curl -X POST http://localhost:8080/api/vault/vault_abc123/withdraw \
  -H "Content-Type: application/json" \
  -d '{
    "satoshis": 500000,
    "to_address": "'$VAULT_ADDRESS'"
  }'
```

### Repay Debt

Reduce or close your position:

```bash
# Partial repayment
curl -X POST http://localhost:8080/api/vault/vault_abc123/repay \
  -H "Content-Type: application/json" \
  -d '{
    "stable_amount": 500
  }'

# Full repayment (closes vault)
curl -X POST http://localhost:8080/api/vault/vault_abc123/close
```

## Understanding Liquidations

### Liquidation Triggers

Your vault gets liquidated when:
- Collateralization ratio falls below 150%
- Usually due to BTC price decline

### Liquidation Process

1. **Detection**: System monitors all vaults continuously
2. **Auction**: Collateral offered at 13% discount
3. **Settlement**: Debt repaid from auction proceeds
4. **Return**: Any excess returned to vault owner

### Avoiding Liquidation

Monitor your vault health:

```bash
# Check health factor
curl http://localhost:8080/api/vault/vault_abc123/health

# Response:
{
  "health_factor": 1.65,
  "collateralization_ratio": 165.0,
  "liquidation_price": 40909.09,
  "current_btc_price": 45000.00,
  "safe": true
}
```

Set up alerts:

```bash
# Register for notifications
curl -X POST http://localhost:8080/api/alerts/register \
  -H "Content-Type: application/json" \
  -d '{
    "vault_id": "vault_abc123",
    "threshold_ratio": 160,
    "webhook_url": "https://your-webhook.com/alert"
  }'
```

## Using the API

### Authentication

For production, use API keys:

```bash
# Include API key in headers
curl -H "X-API-Key: your_api_key_here" \
  http://localhost:8080/api/vault/vault_abc123
```

### Rate Limits

- Public endpoints: 100 requests/minute
- Authenticated: 1000 requests/minute

### WebSocket Subscriptions

Real-time updates:

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');

ws.send(JSON.stringify({
  type: 'subscribe',
  channels: ['vault:vault_abc123', 'oracle:price']
}));

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Update:', data);
};
```

### Error Handling

API errors follow standard format:

```json
{
  "error": {
    "code": "INSUFFICIENT_COLLATERAL",
    "message": "Collateral ratio would fall below minimum",
    "details": {
      "current_ratio": 145.5,
      "required_ratio": 150.0
    }
  }
}
```

## Security Best Practices

### 1. Key Management

- **Never share private keys**
- **Use hardware wallets for mainnet**
- **Backup seed phrases securely**
- **Use multisig for large vaults**

### 2. Vault Management

- **Monitor regularly**: Check daily during volatile periods
- **Maintain buffer**: Keep ratio above 200% for safety
- **Set alerts**: Configure multiple notification channels
- **Test first**: Use testnet before mainnet

### 3. Transaction Security

- **Verify addresses**: Double-check before sending
- **Use appropriate fees**: Ensure timely confirmations
- **Wait for confirmations**: 3+ for deposits, 6+ for withdrawals

### 4. API Security

- **Rotate API keys regularly**
- **Use HTTPS in production**
- **Implement rate limiting**
- **Log all operations**

## FAQ

### General Questions

**Q: What is the minimum vault size?**
A: Minimum 100 stable tokens, requiring ~$150 in BTC at 150% ratio.

**Q: How long do transactions take?**
A: Deposits confirm in ~30 minutes (3 blocks), withdrawals in ~60 minutes (6 blocks).

**Q: Can I have multiple vaults?**
A: Yes, unlimited vaults per address.

### Fees

**Q: What fees are charged?**
A: 
- Stability fee: 2% annual on debt
- Liquidation penalty: 13% on liquidated collateral
- Bitcoin network fees: Variable based on congestion

**Q: How is the stability fee calculated?**
A: Accrued continuously, charged on repayment:
```
Fee = Debt × (0.02 × Days / 365)
```

### Liquidations

**Q: What happens if my vault is liquidated?**
A: Your collateral is sold to repay debt. Any excess after the 13% penalty is returned.

**Q: Can I stop a liquidation in progress?**
A: No, but you can add collateral before ratio hits 150%.

**Q: How quickly do liquidations happen?**
A: Immediately when ratio falls below 150%.

### Technical

**Q: Which Bitcoin addresses are supported?**
A: Native SegWit (bc1/tb1), P2SH-SegWit (3/2), and Legacy (1/m/n).

**Q: Is the protocol audited?**
A: Yes, see [audit reports](./audits/).

**Q: What's the oracle update frequency?**
A: Every 60 seconds, using median of 3+ sources.

## Troubleshooting

### Common Issues

#### "Insufficient collateral"
- Check BTC price hasn't dropped
- Ensure you're sending enough BTC
- Account for stability fee

#### "Transaction not found"
- Wait for Bitcoin confirmations
- Check transaction on blockchain explorer
- Verify correct network (mainnet/testnet)

#### "Vault not found"
- Confirm vault ID is correct
- Check vault hasn't been closed
- Ensure using correct API endpoint

### Getting Help

- Documentation: https://docs.bitstable.io
- Discord: https://discord.gg/bitstable
- Email: support@bitstable.io
- GitHub Issues: https://github.com/bitstable/bitstable/issues

## Advanced Usage

### Automation

Create a monitoring script:

```python
import requests
import time

VAULT_ID = "vault_abc123"
API_URL = "http://localhost:8080/api"
WARNING_RATIO = 180  # Alert when below 180%

while True:
    response = requests.get(f"{API_URL}/vault/{VAULT_ID}/health")
    data = response.json()
    
    if data["collateralization_ratio"] < WARNING_RATIO:
        print(f"⚠️ Warning: Ratio at {data['collateralization_ratio']}%")
        # Send alert, add collateral, etc.
    
    time.sleep(60)  # Check every minute
```

### Integration

Integrate BitStable into your application:

```javascript
const BitStable = require('@bitstable/sdk');

const client = new BitStable({
  apiUrl: 'http://localhost:8080',
  apiKey: process.env.BITSTABLE_API_KEY
});

// Create vault
const vault = await client.createVault({
  collateral: 0.05,  // BTC
  stableAmount: 2000  // Stable tokens
});

// Monitor health
const health = await client.getVaultHealth(vault.id);
if (health.ratio < 180) {
  await client.addCollateral(vault.id, 0.01);
}
```

## Next Steps

- Explore [API Documentation](./API.md) for full endpoint reference
- Read [Architecture Guide](./ARCHITECTURE.md) to understand internals
- Join our [Discord](https://discord.gg/bitstable) community
- Follow [@bitstable](https://twitter.com/bitstable) for updates
