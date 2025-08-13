use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use bitcoin::secp256k1::PublicKey;
use crate::{BitStableError, Result, ProtocolConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub price_usd: f64,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Oracle {
    pub name: String,
    pub url: String,
    pub pubkey: PublicKey,
    pub last_price: Option<PriceData>,
    pub client: reqwest::Client,
}

impl Oracle {
    pub fn new(name: String, url: String, pubkey: PublicKey) -> Self {
        Self {
            name,
            url,
            pubkey,
            last_price: None,
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_price(&mut self) -> Result<PriceData> {
        let response = self.client
            .get(&self.url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let price = self.parse_price_response(response).await?;
        self.last_price = Some(price.clone());
        
        log::debug!("Oracle {} fetched price: ${}", self.name, price.price_usd);
        
        Ok(price)
    }

    async fn parse_price_response(&self, response: reqwest::Response) -> Result<PriceData> {
        let text = response.text().await?;
        
        // Parse different exchange formats
        let price = match self.name.as_str() {
            "Coinbase" => self.parse_coinbase(&text)?,
            "Binance" => self.parse_binance(&text)?,
            "Kraken" => self.parse_kraken(&text)?,
            "Bitstamp" => self.parse_bitstamp(&text)?,
            "CoinGecko" => self.parse_coingecko(&text)?,
            _ => return Err(BitStableError::PriceFeedError(format!("Unknown oracle: {}", self.name))),
        };

        Ok(PriceData {
            price_usd: price,
            timestamp: Utc::now(),
            source: self.name.clone(),
            signature: None, // TODO: Implement oracle signatures
        })
    }

    fn parse_coinbase(&self, text: &str) -> Result<f64> {
        #[derive(Deserialize)]
        struct CoinbaseResponse {
            data: CoinbaseData,
        }
        
        #[derive(Deserialize)]
        struct CoinbaseData {
            rates: HashMap<String, String>,
        }

        let response: CoinbaseResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("Coinbase parse error: {}", e)))?;
        
        response.data.rates.get("USD")
            .ok_or_else(|| BitStableError::PriceFeedError("USD rate not found".to_string()))?
            .parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_binance(&self, text: &str) -> Result<f64> {
        #[derive(Deserialize)]
        struct BinanceResponse {
            price: String,
        }

        let response: BinanceResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("Binance parse error: {}", e)))?;
        
        response.price.parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_kraken(&self, text: &str) -> Result<f64> {
        #[derive(Deserialize)]
        struct KrakenResponse {
            result: HashMap<String, KrakenTicker>,
        }
        
        #[derive(Deserialize)]
        struct KrakenTicker {
            c: Vec<String>, // last trade closed array
        }

        let response: KrakenResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("Kraken parse error: {}", e)))?;
        
        let ticker = response.result.get("XXBTZUSD")
            .ok_or_else(|| BitStableError::PriceFeedError("XXBTZUSD pair not found".to_string()))?;
        
        ticker.c.first()
            .ok_or_else(|| BitStableError::PriceFeedError("No last price found".to_string()))?
            .parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_bitstamp(&self, text: &str) -> Result<f64> {
        #[derive(Deserialize)]
        struct BitstampResponse {
            last: String,
        }

        let response: BitstampResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("Bitstamp parse error: {}", e)))?;
        
        response.last.parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_coingecko(&self, text: &str) -> Result<f64> {
        #[derive(Deserialize)]
        struct CoinGeckoResponse {
            bitcoin: CoinGeckoPrice,
        }
        
        #[derive(Deserialize)]
        struct CoinGeckoPrice {
            usd: f64,
        }

        let response: CoinGeckoResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("CoinGecko parse error: {}", e)))?;
        
        Ok(response.bitcoin.usd)
    }
}

#[derive(Debug)]
pub struct OracleNetwork {
    oracles: Vec<Oracle>,
    config: ProtocolConfig,
    price_history: Vec<ConsensusPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusPrice {
    pub price_usd: f64,
    pub timestamp: DateTime<Utc>,
    pub participating_oracles: usize,
    pub total_oracles: usize,
}

impl OracleNetwork {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        let mut oracles = Vec::new();
        
        for endpoint in &config.oracle_endpoints {
            let pubkey = endpoint.pubkey.parse()
                .map_err(|e| BitStableError::InvalidConfig(format!("Invalid pubkey {}: {}", endpoint.pubkey, e)))?;
            
            oracles.push(Oracle::new(
                endpoint.name.clone(),
                endpoint.url.clone(),
                pubkey,
            ));
        }

        Ok(Self {
            oracles,
            config: config.clone(),
            price_history: Vec::new(),
        })
    }

    pub async fn get_consensus_price(&mut self) -> Result<f64> {
        let mut prices = Vec::new();
        let mut successful_oracles = 0;

        // Fetch prices from all oracles
        let mut results = Vec::new();
        for oracle in &mut self.oracles {
            results.push(oracle.fetch_price().await);
        }
        
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(price_data) => {
                    prices.push(price_data.price_usd);
                    successful_oracles += 1;
                    log::debug!("Oracle {} reported: ${}", self.oracles[i].name, price_data.price_usd);
                }
                Err(e) => {
                    log::warn!("Oracle {} failed: {}", self.oracles[i].name, e);
                }
            }
        }

        if successful_oracles < self.config.oracle_threshold {
            return Err(BitStableError::InsufficientOracleConsensus {
                got: successful_oracles,
                required: self.config.oracle_threshold,
            });
        }

        // Calculate median price for consensus
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let consensus_price = if prices.len() % 2 == 0 {
            (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / 2.0
        } else {
            prices[prices.len() / 2]
        };

        // Record consensus
        let consensus = ConsensusPrice {
            price_usd: consensus_price,
            timestamp: Utc::now(),
            participating_oracles: successful_oracles,
            total_oracles: self.oracles.len(),
        };

        self.price_history.push(consensus);
        
        // Keep only last 1000 price points
        if self.price_history.len() > 1000 {
            self.price_history.remove(0);
        }

        log::info!("Consensus price: ${} (from {}/{} oracles)", 
                  consensus_price, successful_oracles, self.oracles.len());

        Ok(consensus_price)
    }

    pub fn get_latest_consensus(&self) -> Option<&ConsensusPrice> {
        self.price_history.last()
    }

    pub fn get_price_history(&self, limit: usize) -> Vec<&ConsensusPrice> {
        let start = if self.price_history.len() > limit {
            self.price_history.len() - limit
        } else {
            0
        };
        self.price_history[start..].iter().collect()
    }

    pub fn validate_price_movement(&self, new_price: f64) -> bool {
        if let Some(last_price) = self.price_history.last() {
            let change_percent = ((new_price - last_price.price_usd) / last_price.price_usd).abs();
            // Reject prices that moved more than 20% in one update (circuit breaker)
            change_percent < 0.20
        } else {
            true // First price always valid
        }
    }
}

// TODO: Implement threshold signatures for oracle consensus
// This would use schnorr signatures where each oracle signs the price data
// and we aggregate signatures to create a threshold signature that proves
// consensus without revealing individual oracle signatures