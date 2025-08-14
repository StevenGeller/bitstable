# API Documentation

## Base URL

```
http://localhost:8080/api
```

## Authentication

Include API key in request headers:

```http
X-API-Key: your_api_key_here
```

## Response Format

All responses are JSON with this structure:

### Success Response
```json
{
  "success": true,
  "data": { ... },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Error Response
```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human readable message",
    "details": { ... }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Endpoints

### System

#### GET /health
Health check endpoint

**Response:**
```json
{
  "status": "healthy",
  "bitcoin_connected": true,
  "database_connected": true,
  "oracle_active": true,
  "version": "1.0.0"
}
```

#### GET /info
System information

**Response:**
```json
{
  "network": "testnet",
  "total_vaults": 150,
  "total_collateral_btc": 45.5,
  "total_debt_stable": 1500000,
  "stability_fee_rate": 0.02,
  "liquidation_penalty": 0.13,
  "min_collateral_ratio": 1.5
}
```

### Oracle

#### GET /oracle/price
Get current BTC price

**Response:**
```json
{
  "price": 45000.00,
  "timestamp": "2024-01-15T10:30:00Z",
  "sources": [
    {"name": "binance", "price": 45010.00},
    {"name": "coinbase", "price": 44995.00},
    {"name": "kraken", "price": 44995.00}
  ],
  "median_price": 45000.00
}
```

#### GET /oracle/history
Get price history

**Query Parameters:**
- `from`: Start timestamp (ISO 8601)
- `to`: End timestamp (ISO 8601)
- `interval`: Interval in minutes (default: 60)

**Response:**
```json
{
  "prices": [
    {
      "price": 45000.00,
      "timestamp": "2024-01-15T10:00:00Z"
    },
    {
      "price": 45100.00,
      "timestamp": "2024-01-15T11:00:00Z"
    }
  ]
}
```

### Vaults

#### POST /vault/create
Create a new vault

**Request Body:**
```json
{
  "owner_address": "tb1q...",
  "collateral_satoshis": 5000000,
  "stable_amount": 2000
}
```

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "owner": "tb1q...",
  "collateral": 5000000,
  "debt": 2000,
  "collateralization_ratio": 150.0,
  "status": "pending",
  "deposit_address": "tb1q_vault_deposit",
  "created_at": "2024-01-15T10:35:00Z"
}
```

#### GET /vault/{vault_id}
Get vault details

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "owner": "tb1q...",
  "collateral": 5000000,
  "debt": 2000,
  "accrued_fee": 0.11,
  "collateralization_ratio": 168.75,
  "liquidation_price": 40000.00,
  "status": "active",
  "created_at": "2024-01-15T10:35:00Z",
  "updated_at": "2024-01-15T12:00:00Z"
}
```

#### GET /vaults
List vaults

**Query Parameters:**
- `owner`: Filter by owner address
- `status`: Filter by status (active, liquidated, closed)
- `page`: Page number (default: 1)
- `limit`: Items per page (default: 20, max: 100)

**Response:**
```json
{
  "vaults": [
    {
      "vault_id": "vault_abc123",
      "owner": "tb1q...",
      "collateral": 5000000,
      "debt": 2000,
      "collateralization_ratio": 168.75,
      "status": "active"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 45,
    "pages": 3
  }
}
```

#### GET /vault/{vault_id}/health
Get vault health metrics

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "health_factor": 1.125,
  "collateralization_ratio": 168.75,
  "current_btc_price": 45000.00,
  "liquidation_price": 40000.00,
  "price_drop_percentage": 11.11,
  "safe": true,
  "warning_level": "none"
}
```

#### POST /vault/{vault_id}/add-collateral
Add collateral to vault

**Request Body:**
```json
{
  "satoshis": 1000000
}
```

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "new_collateral": 6000000,
  "new_ratio": 202.5,
  "transaction_id": "tx_def456",
  "status": "pending"
}
```

#### POST /vault/{vault_id}/withdraw
Withdraw excess collateral

**Request Body:**
```json
{
  "satoshis": 500000,
  "to_address": "tb1q..."
}
```

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "withdrawn": 500000,
  "remaining_collateral": 5500000,
  "new_ratio": 185.625,
  "transaction_id": "tx_ghi789",
  "status": "pending"
}
```

#### POST /vault/{vault_id}/mint
Mint additional stable tokens

**Request Body:**
```json
{
  "stable_amount": 500
}
```

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "minted": 500,
  "total_debt": 2500,
  "new_ratio": 135.0,
  "warning": "Approaching minimum collateral ratio"
}
```

#### POST /vault/{vault_id}/repay
Repay vault debt

**Request Body:**
```json
{
  "stable_amount": 1000
}
```

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "repaid": 1000,
  "stability_fee": 0.055,
  "total_paid": 1000.055,
  "remaining_debt": 1000,
  "new_ratio": 337.5
}
```

#### POST /vault/{vault_id}/close
Close vault (repay all debt)

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "debt_repaid": 2000,
  "stability_fee": 0.11,
  "total_paid": 2000.11,
  "collateral_returned": 5000000,
  "return_address": "tb1q...",
  "transaction_id": "tx_jkl012",
  "status": "closed"
}
```

#### GET /vault/{vault_id}/history
Get vault transaction history

**Response:**
```json
{
  "vault_id": "vault_abc123",
  "history": [
    {
      "type": "create",
      "timestamp": "2024-01-15T10:35:00Z",
      "details": {
        "collateral": 5000000,
        "debt": 2000
      }
    },
    {
      "type": "add_collateral",
      "timestamp": "2024-01-15T12:00:00Z",
      "details": {
        "amount": 1000000,
        "transaction_id": "tx_def456"
      }
    }
  ]
}
```

### Liquidations

#### GET /liquidations
Get liquidation opportunities

**Response:**
```json
{
  "liquidations": [
    {
      "vault_id": "vault_xyz789",
      "collateral": 3000000,
      "debt": 2000,
      "collateralization_ratio": 67.5,
      "discount_price": 39150.00,
      "profit_potential": 260.00
    }
  ]
}
```

#### POST /liquidation/bid
Submit liquidation bid

**Request Body:**
```json
{
  "vault_id": "vault_xyz789",
  "stable_amount": 2000
}
```

**Response:**
```json
{
  "liquidation_id": "liq_mno345",
  "vault_id": "vault_xyz789",
  "collateral_received": 3000000,
  "stable_paid": 2000,
  "penalty_amount": 260,
  "profit": 235.00,
  "transaction_id": "tx_pqr678",
  "status": "completed"
}
```

#### GET /liquidations/history
Get liquidation history

**Query Parameters:**
- `vault_id`: Filter by vault
- `from`: Start date
- `to`: End date
- `page`: Page number
- `limit`: Items per page

**Response:**
```json
{
  "liquidations": [
    {
      "liquidation_id": "liq_mno345",
      "vault_id": "vault_xyz789",
      "timestamp": "2024-01-15T14:30:00Z",
      "collateral": 3000000,
      "debt": 2000,
      "penalty": 260,
      "liquidator": "tb1q..."
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 5
  }
}
```

### Bitcoin

#### GET /bitcoin/info
Get Bitcoin network info

**Response:**
```json
{
  "chain": "test",
  "blocks": 2500000,
  "headers": 2500000,
  "bestblockhash": "000000...",
  "difficulty": 1,
  "mediantime": 1705315800,
  "verificationprogress": 0.999999,
  "chainwork": "000000...",
  "pruned": false
}
```

#### GET /bitcoin/transaction/{txid}
Get transaction details

**Response:**
```json
{
  "txid": "abc123...",
  "confirmations": 3,
  "blockhash": "000000...",
  "blocktime": 1705315800,
  "size": 250,
  "vsize": 175,
  "weight": 700,
  "fee": 0.00001000,
  "status": "confirmed"
}
```

#### POST /bitcoin/estimate-fee
Estimate transaction fee

**Request Body:**
```json
{
  "blocks": 6
}
```

**Response:**
```json
{
  "feerate": 0.00001000,
  "blocks": 6
}
```

### Alerts

#### POST /alerts/register
Register for vault alerts

**Request Body:**
```json
{
  "vault_id": "vault_abc123",
  "threshold_ratio": 160,
  "webhook_url": "https://your-webhook.com/alert",
  "email": "user@example.com"
}
```

**Response:**
```json
{
  "alert_id": "alert_stu901",
  "vault_id": "vault_abc123",
  "threshold_ratio": 160,
  "channels": ["webhook", "email"],
  "status": "active"
}
```

#### GET /alerts
List active alerts

**Response:**
```json
{
  "alerts": [
    {
      "alert_id": "alert_stu901",
      "vault_id": "vault_abc123",
      "threshold_ratio": 160,
      "channels": ["webhook", "email"],
      "status": "active",
      "last_triggered": null
    }
  ]
}
```

#### DELETE /alerts/{alert_id}
Delete an alert

**Response:**
```json
{
  "alert_id": "alert_stu901",
  "status": "deleted"
}
```

## WebSocket API

### Connection

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');
```

### Authentication

```json
{
  "type": "auth",
  "api_key": "your_api_key_here"
}
```

### Subscriptions

#### Subscribe to channels

```json
{
  "type": "subscribe",
  "channels": [
    "oracle:price",
    "vault:vault_abc123",
    "liquidations"
  ]
}
```

#### Unsubscribe

```json
{
  "type": "unsubscribe",
  "channels": ["oracle:price"]
}
```

### Event Types

#### Price Update
```json
{
  "type": "price_update",
  "channel": "oracle:price",
  "data": {
    "price": 45100.00,
    "timestamp": "2024-01-15T10:31:00Z"
  }
}
```

#### Vault Update
```json
{
  "type": "vault_update",
  "channel": "vault:vault_abc123",
  "data": {
    "vault_id": "vault_abc123",
    "collateralization_ratio": 165.5,
    "health_factor": 1.103
  }
}
```

#### Liquidation Alert
```json
{
  "type": "liquidation_opportunity",
  "channel": "liquidations",
  "data": {
    "vault_id": "vault_xyz789",
    "collateral": 3000000,
    "debt": 2000,
    "discount_price": 39150.00
  }
}
```

## Error Codes

| Code | Description |
|------|-------------|
| `INVALID_REQUEST` | Malformed request |
| `UNAUTHORIZED` | Invalid or missing API key |
| `FORBIDDEN` | Access denied |
| `NOT_FOUND` | Resource not found |
| `INSUFFICIENT_COLLATERAL` | Collateral below minimum |
| `VAULT_UNHEALTHY` | Vault below liquidation threshold |
| `INSUFFICIENT_BALANCE` | Not enough stable tokens |
| `INVALID_ADDRESS` | Invalid Bitcoin address |
| `TRANSACTION_FAILED` | Bitcoin transaction failed |
| `ORACLE_ERROR` | Price feed unavailable |
| `RATE_LIMITED` | Too many requests |
| `INTERNAL_ERROR` | Server error |

## Rate Limits

| Endpoint Type | Limit |
|--------------|-------|
| Public | 100 requests/minute |
| Authenticated | 1000 requests/minute |
| WebSocket | 100 messages/minute |

## Pagination

Paginated endpoints accept:
- `page`: Page number (starts at 1)
- `limit`: Items per page (max 100)

Response includes:
```json
{
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 150,
    "pages": 8,
    "has_next": true,
    "has_prev": false
  }
}
```

## Versioning

API version in URL:
```
http://localhost:8080/api/v1/...
```

Or in header:
```http
X-API-Version: 1
```

## SDK Examples

### JavaScript/TypeScript

```javascript
const BitStableSDK = require('@bitstable/sdk');

const client = new BitStableSDK({
  apiUrl: 'http://localhost:8080',
  apiKey: process.env.BITSTABLE_API_KEY
});

// Create vault
const vault = await client.vaults.create({
  collateralSatoshis: 5000000,
  stableAmount: 2000
});

// Monitor price
client.oracle.subscribe('price', (price) => {
  console.log('BTC Price:', price);
});
```

### Python

```python
from bitstable import BitStableClient

client = BitStableClient(
    api_url='http://localhost:8080',
    api_key=os.environ['BITSTABLE_API_KEY']
)

# Create vault
vault = client.vaults.create(
    collateral_satoshis=5000000,
    stable_amount=2000
)

# Check health
health = client.vaults.get_health(vault['vault_id'])
if health['collateralization_ratio'] < 160:
    print('Warning: Vault at risk!')
```

### Go

```go
import "github.com/bitstable/go-sdk"

client := bitstable.NewClient(
    "http://localhost:8080",
    os.Getenv("BITSTABLE_API_KEY"),
)

// Create vault
vault, err := client.Vaults.Create(&bitstable.CreateVaultRequest{
    CollateralSatoshis: 5000000,
    StableAmount: 2000,
})

// Monitor health
health, err := client.Vaults.GetHealth(vault.ID)
if health.CollateralizationRatio < 160 {
    log.Warn("Vault at risk!")
}
```

## Testing

### Test Environment

```
URL: https://testnet-api.bitstable.io
Network: Bitcoin Testnet
```

### Test API Key

Request at: https://testnet.bitstable.io/developer

### Postman Collection

Download: [BitStable API.postman_collection.json](./postman/collection.json)

## Support

- Documentation: https://docs.bitstable.io
- GitHub: https://github.com/bitstable/bitstable
- Discord: https://discord.gg/bitstable
- Email: api-support@bitstable.io
