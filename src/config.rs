use serde::{Deserialize, Serialize};
use bitcoin::Network;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub network: Network,
    pub min_collateral_ratio: f64,
    pub liquidation_threshold: f64,
    pub liquidation_penalty: f64,
    pub stability_fee_apr: f64,
    pub oracle_threshold: usize,
    pub oracle_timeout_seconds: u64,
    pub database_path: String,
    pub oracle_endpoints: Vec<OracleEndpoint>,
    // Progressive liquidation thresholds
    pub progressive_liquidation_threshold: f64,  // 130%
    pub partial_liquidation_25: f64,            // 130%
    pub partial_liquidation_50: f64,            // 127.5%
    pub partial_liquidation_75: f64,            // 125%
    pub insurance_fund_fee_rate: f64,           // 1% of fees to insurance
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleEndpoint {
    pub name: String,
    pub url: String,
    pub pubkey: String,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            network: Network::Testnet,
            min_collateral_ratio: 1.75,  // 175% (increased from 150%)
            liquidation_threshold: 1.25,  // 125% (increased from 110%)
            liquidation_penalty: 0.05,   // 5%
            stability_fee_apr: 0.02,     // 2%
            oracle_threshold: 3,         // 3 of 5 oracles
            oracle_timeout_seconds: 30,
            database_path: "./bitstable.db".to_string(),
            oracle_endpoints: vec![
                OracleEndpoint {
                    name: "Coinbase".to_string(),
                    url: "https://api.coinbase.com/v2/exchange-rates?currency=BTC".to_string(),
                    pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
                },
                OracleEndpoint {
                    name: "Binance".to_string(),
                    url: "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT".to_string(),
                    pubkey: "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9".to_string(),
                },
                OracleEndpoint {
                    name: "Kraken".to_string(),
                    url: "https://api.kraken.com/0/public/Ticker?pair=XBTUSD".to_string(),
                    pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
                },
                OracleEndpoint {
                    name: "Bitstamp".to_string(),
                    url: "https://www.bitstamp.net/api/v2/ticker/btcusd/".to_string(),
                    pubkey: "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9".to_string(),
                },
                OracleEndpoint {
                    name: "CoinGecko".to_string(),
                    url: "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd".to_string(),
                    pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
                },
            ],
            // Progressive liquidation configuration
            progressive_liquidation_threshold: 1.30,  // 130%
            partial_liquidation_25: 1.30,             // 25% at 130%
            partial_liquidation_50: 1.275,            // 50% at 127.5%
            partial_liquidation_75: 1.25,             // 75% at 125%
            insurance_fund_fee_rate: 0.01,            // 1% of fees
        }
    }
}

impl ProtocolConfig {
    pub fn testnet() -> Self {
        Self::default()
    }

    pub fn mainnet() -> Self {
        let mut config = Self::default();
        config.network = Network::Bitcoin;
        config.database_path = "./bitstable-mainnet.db".to_string();
        config
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.min_collateral_ratio <= 1.0 {
            return Err(crate::BitStableError::InvalidConfig(
                "min_collateral_ratio must be > 1.0".to_string()
            ));
        }

        if self.liquidation_threshold >= self.min_collateral_ratio {
            return Err(crate::BitStableError::InvalidConfig(
                "liquidation_threshold must be < min_collateral_ratio".to_string()
            ));
        }

        if self.oracle_threshold > self.oracle_endpoints.len() {
            return Err(crate::BitStableError::InvalidConfig(
                "oracle_threshold cannot exceed number of oracle endpoints".to_string()
            ));
        }

        Ok(())
    }
}