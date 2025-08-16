use bitcoin::Amount;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc, Duration};
use bitcoin::secp256k1::PublicKey;
use crate::{BitStableError, Result, ProtocolConfig};
use crate::multi_currency::{Currency, ExchangeRates};

/// Types of oracle slashing offenses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlashType {
    PriceDeviation,  // >5% deviation from consensus
    Downtime,        // >1 hour offline
    Manipulation,    // Evidence of manipulation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub prices: HashMap<Currency, f64>,  // BTC price in each currency
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub signature: Option<String>,
}

/// Graduated circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub tier1_threshold: f64,        // 10% requires 5/7 oracles
    pub tier2_threshold: f64,        // 20% requires 7/7 oracles  
    pub tier3_threshold: f64,        // 30% emergency governance override
    pub min_oracles_tier1: usize,   // Minimum oracles for tier 1
    pub min_oracles_tier2: usize,   // Minimum oracles for tier 2
    pub emergency_override: bool,    // Governance can override
    pub cooldown_minutes: u64,       // Cooldown between large moves
}

/// Time-weighted average price tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWeightedPrice {
    pub prices: VecDeque<(DateTime<Utc>, f64)>,
    pub window_hours: u64,
    pub last_twap: f64,
}

/// Oracle bonding and slashing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleBond {
    pub bond_amount: Amount,           // 2 × max_daily_volume × P_BTC / 365
    pub slashed_amount: Amount,        // Total amount slashed
    pub last_slash_timestamp: Option<DateTime<Utc>>,
    pub bond_locked: bool,
    pub withdrawal_request: Option<DateTime<Utc>>,
}

/// Oracle reputation and quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleMetrics {
    pub uptime_percentage: f64,
    pub response_time_ms: u64,
    pub price_deviation_score: f64,
    pub accuracy_score: f64,           // Historical accuracy
    pub reputation_score: f64,         // Overall reputation 0-1
    pub last_error: Option<String>,
    pub total_failures: u64,
    pub successful_submissions: u64,
    pub bond: OracleBond,
    pub last_price_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Oracle {
    pub name: String,
    pub urls: HashMap<Currency, String>,  // URLs for each currency pair
    pub pubkey: PublicKey,
    pub last_prices: HashMap<Currency, PriceData>,
    pub client: reqwest::Client,
    pub twap_data: HashMap<Currency, TimeWeightedPrice>,
    pub metrics: OracleMetrics,
    pub quality_score: f64,
    pub is_bonded: bool,
    pub backup_urls: HashMap<Currency, Vec<String>>, // Backup price sources
}

impl Oracle {
    pub fn new(name: String, pubkey: PublicKey) -> Self {
        Self {
            name,
            urls: HashMap::new(),
            pubkey,
            last_prices: HashMap::new(),
            client: reqwest::Client::new(),
            twap_data: HashMap::new(),
            metrics: OracleMetrics {
                uptime_percentage: 100.0,
                response_time_ms: 0,
                price_deviation_score: 0.0,
                accuracy_score: 1.0,
                reputation_score: 1.0,
                last_error: None,
                total_failures: 0,
                successful_submissions: 0,
                bond: OracleBond {
                    bond_amount: Amount::ZERO,
                    slashed_amount: Amount::ZERO,
                    last_slash_timestamp: None,
                    bond_locked: false,
                    withdrawal_request: None,
                },
                last_price_timestamp: None,
            },
            quality_score: 1.0,
            is_bonded: false,
            backup_urls: HashMap::new(),
        }
    }

    pub fn add_price_feed(&mut self, currency: Currency, url: String) {
        self.urls.insert(currency, url);
    }

    pub async fn fetch_prices(&mut self) -> Result<HashMap<Currency, f64>> {
        let start_time = std::time::Instant::now();
        let mut prices = HashMap::new();

        // Clone the URLs to avoid borrowing issues
        let urls = self.urls.clone();
        
        for (currency, url) in urls {
            match self.fetch_single_price(&url, &currency).await {
                Ok(price) => {
                    prices.insert(currency.clone(), price);
                    self.update_twap(currency.clone(), price);
                    log::debug!("Oracle {} fetched {}/{}: {}", self.name, "BTC", currency.to_string(), price);
                }
                Err(e) => {
                    self.metrics.total_failures += 1;
                    self.metrics.last_error = Some(e.to_string());
                    log::warn!("Oracle {} failed to fetch {}: {}", self.name, currency.to_string(), e);
                }
            }
        }

        // Update response time metric
        self.metrics.response_time_ms = start_time.elapsed().as_millis() as u64;
        
        // Update quality score based on success rate and reputation
        let success_rate = prices.len() as f64 / self.urls.len() as f64;
        let failure_penalty = (self.metrics.total_failures as f64 / 100.0).min(1.0);
        let accuracy_factor = self.metrics.accuracy_score;
        let reputation_factor = self.metrics.reputation_score;
        
        self.quality_score = success_rate * 0.4 + 
                           accuracy_factor * 0.3 + 
                           reputation_factor * 0.2 + 
                           (1.0 - failure_penalty) * 0.1;

        if prices.is_empty() {
            return Err(BitStableError::PriceFeedError("No prices fetched".to_string()));
        }

        Ok(prices)
    }

    /// Update TWAP data for a currency
    fn update_twap(&mut self, currency: Currency, price: f64) {
        let twap = self.twap_data.entry(currency).or_insert_with(|| TimeWeightedPrice {
            prices: VecDeque::new(),
            window_hours: 24, // 24-hour TWAP
            last_twap: price,
        });
        
        let now = Utc::now();
        twap.prices.push_back((now, price));
        
        // Remove prices older than window
        let cutoff = now - Duration::hours(twap.window_hours as i64);
        while let Some((timestamp, _)) = twap.prices.front() {
            if *timestamp < cutoff {
                twap.prices.pop_front();
            } else {
                break;
            }
        }
        
        // Calculate TWAP
        if !twap.prices.is_empty() {
            let total_weight: i64 = twap.prices.iter()
                .zip(twap.prices.iter().skip(1))
                .map(|((t1, _), (t2, _))| (t2.timestamp() - t1.timestamp()))
                .sum();
            
            if total_weight > 0 {
                let weighted_sum: f64 = twap.prices.iter()
                    .zip(twap.prices.iter().skip(1))
                    .map(|((t1, p1), (t2, _))| *p1 * (t2.timestamp() - t1.timestamp()) as f64)
                    .sum();
                
                twap.last_twap = weighted_sum / total_weight as f64;
            }
        }
    }

    /// Get TWAP for a currency
    pub fn get_twap(&self, currency: &Currency) -> Option<f64> {
        self.twap_data.get(currency).map(|twap| twap.last_twap)
    }

    /// Submit oracle bond (2 × max_daily_volume × P_BTC / 365)
    pub fn submit_bond(&mut self, bond_amount: Amount, btc_price: f64, max_daily_volume: f64) -> Result<()> {
        let required_bond = Amount::from_btc(2.0 * max_daily_volume * btc_price / 365.0)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid bond amount: {}", e)))?;
        
        if bond_amount < required_bond {
            return Err(BitStableError::InsufficientCollateral {
                required: required_bond.to_btc(),
                provided: bond_amount.to_btc(),
            });
        }
        
        self.metrics.bond.bond_amount = bond_amount;
        self.metrics.bond.bond_locked = true;
        self.is_bonded = true;
        
        log::info!("Oracle {} bonded with {} BTC (required: {} BTC)", 
                  self.name, bond_amount.to_btc(), required_bond.to_btc());
        
        Ok(())
    }

    /// Slash oracle bond for misbehavior
    pub fn slash_bond(&mut self, slash_type: SlashType, _btc_price: f64) -> Result<Amount> {
        if !self.is_bonded {
            return Err(BitStableError::InvalidConfig("Oracle not bonded".to_string()));
        }
        
        let slash_percentage = match slash_type {
            SlashType::PriceDeviation => 0.10,  // 10% for >5% deviation
            SlashType::Downtime => 0.05,        // 5% for >1 hour offline
            SlashType::Manipulation => 1.00,    // 100% for manipulation evidence
        };
        
        let slash_amount = Amount::from_btc(self.metrics.bond.bond_amount.to_btc() * slash_percentage)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid slash amount: {}", e)))?;
        
        self.metrics.bond.slashed_amount += slash_amount;
        self.metrics.bond.bond_amount -= slash_amount;
        self.metrics.bond.last_slash_timestamp = Some(Utc::now());
        
        // Update reputation score
        self.metrics.reputation_score *= 1.0 - slash_percentage * 0.5;
        
        // Unbond if fully slashed
        if slash_percentage >= 1.0 {
            self.is_bonded = false;
            self.metrics.bond.bond_locked = false;
        }
        
        log::warn!("Oracle {} slashed {} BTC for {:?} (remaining bond: {} BTC)", 
                  self.name, slash_amount.to_btc(), slash_type, self.metrics.bond.bond_amount.to_btc());
        
        Ok(slash_amount)
    }

    /// Check if price data is fresh (< 30 seconds old)
    pub fn is_price_fresh(&self, currency: &Currency) -> bool {
        if let Some(last_price) = self.last_prices.get(currency) {
            let age = Utc::now().signed_duration_since(last_price.timestamp);
            age.num_seconds() < 30
        } else {
            false
        }
    }

    /// Add backup URL for automatic failover
    pub fn add_backup_url(&mut self, currency: Currency, backup_url: String) {
        self.backup_urls.entry(currency).or_insert_with(Vec::new).push(backup_url);
    }

    /// Fetch price with automatic failover to backup sources
    pub async fn fetch_price_with_failover(&mut self, currency: &Currency) -> Result<f64> {
        // Try primary URL first
        if let Some(primary_url) = self.urls.get(currency) {
            match self.fetch_single_price(primary_url, currency).await {
                Ok(price) => {
                    self.metrics.successful_submissions += 1;
                    self.metrics.last_price_timestamp = Some(Utc::now());
                    return Ok(price);
                }
                Err(e) => {
                    log::warn!("Primary oracle {} failed for {}: {}", self.name, currency.to_string(), e);
                }
            }
        }
        
        // Try backup URLs
        if let Some(backup_urls) = self.backup_urls.get(currency) {
            for backup_url in backup_urls {
                match self.fetch_single_price(backup_url, currency).await {
                    Ok(price) => {
                        self.metrics.successful_submissions += 1;
                        self.metrics.last_price_timestamp = Some(Utc::now());
                        log::info!("Oracle {} failed over to backup for {}", self.name, currency.to_string());
                        return Ok(price);
                    }
                    Err(e) => {
                        log::warn!("Backup oracle {} failed for {}: {}", self.name, currency.to_string(), e);
                    }
                }
            }
        }
        
        self.metrics.total_failures += 1;
        Err(BitStableError::PriceFeedError(format!("All oracle sources failed for {}", currency.to_string())))
    }

    async fn fetch_single_price(&self, url: &str, currency: &Currency) -> Result<f64> {
        let response = self.client
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let text = response.text().await?;
        self.parse_price_response(&text, currency)
    }

    fn parse_price_response(&self, text: &str, currency: &Currency) -> Result<f64> {
        // Parse different exchange formats based on oracle name and currency
        match self.name.as_str() {
            "Coinbase" => self.parse_coinbase(text, currency),
            "Binance" => self.parse_binance(text, currency),
            "Kraken" => self.parse_kraken(text, currency),
            "CoinGecko" => self.parse_coingecko(text, currency),
            _ => Err(BitStableError::PriceFeedError(format!("Unknown oracle: {}", self.name))),
        }
    }

    fn parse_coinbase(&self, text: &str, currency: &Currency) -> Result<f64> {
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
        
        response.data.rates.get(&currency.to_string())
            .ok_or_else(|| BitStableError::PriceFeedError(format!("{} rate not found", currency.to_string())))?
            .parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_binance(&self, text: &str, _currency: &Currency) -> Result<f64> {
        #[derive(Deserialize)]
        struct BinanceResponse {
            price: String,
        }

        let response: BinanceResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("Binance parse error: {}", e)))?;
        
        response.price.parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_kraken(&self, text: &str, currency: &Currency) -> Result<f64> {
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
        
        // Map currency to Kraken pair format
        let pair = match currency {
            Currency::USD => "XXBTZUSD",
            Currency::EUR => "XXBTZEUR",
            Currency::GBP => "XXBTZGBP",
            Currency::JPY => "XXBTZJPY",
            Currency::CAD => "XXBTZCAD",
            Currency::AUD => "XXBTZAUD",
            _ => return Err(BitStableError::PriceFeedError(format!("Unsupported currency for Kraken: {}", currency.to_string()))),
        };
        
        let ticker = response.result.get(pair)
            .ok_or_else(|| BitStableError::PriceFeedError(format!("{} pair not found", pair)))?;
        
        ticker.c.first()
            .ok_or_else(|| BitStableError::PriceFeedError("No last price found".to_string()))?
            .parse()
            .map_err(|e| BitStableError::PriceFeedError(format!("Price parse error: {}", e)))
    }

    fn parse_coingecko(&self, text: &str, currency: &Currency) -> Result<f64> {
        #[derive(Deserialize)]
        struct CoinGeckoResponse {
            bitcoin: HashMap<String, f64>,
        }

        let response: CoinGeckoResponse = serde_json::from_str(text)
            .map_err(|e| BitStableError::PriceFeedError(format!("CoinGecko parse error: {}", e)))?;
        
        let currency_key = currency.to_string().to_lowercase();
        response.bitcoin.get(&currency_key)
            .copied()
            .ok_or_else(|| BitStableError::PriceFeedError(format!("{} price not found", currency.to_string())))
    }
}

#[derive(Debug)]
pub struct MultiCurrencyOracleNetwork {
    oracles: Vec<Oracle>,
    config: ProtocolConfig,
    price_history: Vec<ConsensusPrices>,
    exchange_rates: ExchangeRates,
    circuit_breaker: CircuitBreakerConfig,
    last_price_update: HashMap<Currency, DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusPrices {
    pub btc_prices: HashMap<Currency, f64>,
    pub exchange_rates: HashMap<Currency, f64>,  // Rates to USD
    pub timestamp: DateTime<Utc>,
    pub participating_oracles: usize,
    pub total_oracles: usize,
}

impl MultiCurrencyOracleNetwork {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        let mut oracles = Vec::new();
        
        // Initialize oracles with configured endpoints
        for endpoint in &config.oracle_endpoints {
            let pubkey = endpoint.pubkey.parse()
                .map_err(|e| BitStableError::InvalidConfig(format!("Invalid pubkey {}: {}", endpoint.pubkey, e)))?;
            
            let mut oracle = Oracle::new(endpoint.name.clone(), pubkey);
            
            // Add default USD feed
            oracle.add_price_feed(Currency::USD, endpoint.url.clone());
            
            // Add other currency feeds based on oracle capabilities
            if endpoint.name == "CoinGecko" {
                // CoinGecko supports many currencies
                oracle.add_price_feed(Currency::EUR, format!("{}&vs_currencies=eur", endpoint.url));
                oracle.add_price_feed(Currency::GBP, format!("{}&vs_currencies=gbp", endpoint.url));
                oracle.add_price_feed(Currency::JPY, format!("{}&vs_currencies=jpy", endpoint.url));
                oracle.add_price_feed(Currency::NGN, format!("{}&vs_currencies=ngn", endpoint.url));
                oracle.add_price_feed(Currency::MXN, format!("{}&vs_currencies=mxn", endpoint.url));
            }
            
            oracles.push(oracle);
        }

        Ok(Self {
            oracles,
            config: config.clone(),
            price_history: Vec::new(),
            exchange_rates: ExchangeRates::new(),
            circuit_breaker: CircuitBreakerConfig {
                tier1_threshold: 0.10,        // 10% requires 15/20+ oracles
                tier2_threshold: 0.20,        // 20% requires 18/20+ oracles  
                tier3_threshold: 0.30,        // 30% emergency governance override
                min_oracles_tier1: 15,        // Increased from 5 to 15
                min_oracles_tier2: 18,        // Increased from 7 to 18
                emergency_override: false,
                cooldown_minutes: 15,
            },
            last_price_update: HashMap::new(),
        })
    }

    pub async fn get_consensus_prices(&mut self) -> Result<ExchangeRates> {
        let mut all_prices: HashMap<Currency, Vec<f64>> = HashMap::new();
        let mut successful_bonded_oracles = 0;
        let mut total_bonded_oracles = 0;

        // Only use bonded oracles for consensus
        for oracle in &mut self.oracles {
            if oracle.is_bonded {
                total_bonded_oracles += 1;
                
                // Check price freshness and use failover if needed
                let mut oracle_prices = HashMap::new();
                // Collect currencies first to avoid borrowing conflicts
                let currencies: Vec<Currency> = oracle.urls.keys().cloned().collect();
                for currency in currencies {
                    if oracle.is_price_fresh(&currency) {
                        if let Some(last_price) = oracle.last_prices.get(&currency) {
                            oracle_prices.insert(currency.clone(), last_price.prices.get(&currency).copied().unwrap_or(0.0));
                        }
                    } else {
                        // Price stale, fetch new with failover
                        match oracle.fetch_price_with_failover(&currency).await {
                            Ok(price) => {
                                oracle_prices.insert(currency.clone(), price);
                            }
                            Err(e) => {
                                log::warn!("Oracle {} failed for {}: {}", oracle.name, currency.to_string(), e);
                            }
                        }
                    }
                }
                
                if !oracle_prices.is_empty() {
                    successful_bonded_oracles += 1;
                    for (currency, price) in oracle_prices {
                        all_prices.entry(currency).or_insert_with(Vec::new).push(price);
                    }
                }
            }
        }

        // Check minimum bonded oracle threshold
        if total_bonded_oracles < 20 {
            return Err(BitStableError::InsufficientOracleConsensus {
                got: total_bonded_oracles,
                required: 20,
            });
        }
        
        if successful_bonded_oracles < self.config.oracle_threshold {
            return Err(BitStableError::InsufficientOracleConsensus {
                got: successful_bonded_oracles,
                required: self.config.oracle_threshold,
            });
        }

        // Calculate median prices for each currency
        let mut consensus_prices = HashMap::new();
        for (currency, mut prices) in all_prices {
            if !prices.is_empty() {
                prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median = if prices.len() % 2 == 0 {
                    (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / 2.0
                } else {
                    prices[prices.len() / 2]
                };
                
                // Apply graduated circuit breaker with bonded oracle count
                if self.validate_price_movement(&currency, median, successful_bonded_oracles) {
                    consensus_prices.insert(currency.clone(), median);
                    
                    // Check for price deviation and slash oracles if needed
                    self.check_price_deviations(&currency, median, &prices).await?;
                } else {
                    log::warn!("Price movement for {} rejected by circuit breaker", currency.to_string());
                }
            }
        }

        // Update exchange rates
        self.update_exchange_rates(&consensus_prices)?;

        // Record consensus
        let consensus = ConsensusPrices {
            btc_prices: consensus_prices.clone(),
            exchange_rates: self.exchange_rates.to_usd_rates.clone(),
            timestamp: Utc::now(),
            participating_oracles: successful_bonded_oracles,
            total_oracles: self.oracles.len(),
        };

        self.price_history.push(consensus);
        
        // Keep only last 1000 price points
        if self.price_history.len() > 1000 {
            self.price_history.remove(0);
        }

        log::info!("Consensus prices from {}/{} bonded oracles", successful_bonded_oracles, total_bonded_oracles);

        Ok(self.exchange_rates.clone())
    }

    fn update_exchange_rates(&mut self, btc_prices: &HashMap<Currency, f64>) -> Result<()> {
        // Update BTC prices
        for (currency, price) in btc_prices {
            self.exchange_rates.update_btc_price(currency.clone(), *price);
        }

        // Calculate exchange rates to USD
        if let Some(btc_usd) = btc_prices.get(&Currency::USD) {
            for (currency, btc_price) in btc_prices {
                if currency != &Currency::USD {
                    // If BTC/USD = 100000 and BTC/EUR = 95000
                    // Then EUR/USD = 100000/95000 = 1.0526
                    let rate_to_usd = btc_usd / btc_price;
                    self.exchange_rates.update_exchange_rate(currency.clone(), rate_to_usd);
                }
            }
        }

        Ok(())
    }

    /// Enhanced graduated circuit breaker validation
    pub fn validate_price_movement(&mut self, currency: &Currency, new_price: f64, successful_oracles: usize) -> bool {
        if let Some(last_consensus) = self.price_history.last() {
            if let Some(last_price) = last_consensus.btc_prices.get(currency) {
                let change_percent = ((new_price - last_price) / last_price).abs();
                
                // Check cooldown period for large moves
                if let Some(last_update) = self.last_price_update.get(currency) {
                    let cooldown = Duration::minutes(self.circuit_breaker.cooldown_minutes as i64);
                    if Utc::now().signed_duration_since(*last_update) < cooldown && change_percent > 0.05 {
                        log::warn!("Price update for {} in cooldown period", currency.to_string());
                        return false;
                    }
                }
                
                // Graduated circuit breaker logic
                if change_percent > self.circuit_breaker.tier3_threshold {
                    // Emergency threshold - requires governance override
                    if !self.circuit_breaker.emergency_override {
                        log::error!("Price movement for {} ({:.2}%) exceeds emergency threshold, requires governance override", 
                                  currency.to_string(), change_percent * 100.0);
                        return false;
                    }
                } else if change_percent > self.circuit_breaker.tier2_threshold {
                    // Tier 2: 20%+ requires 7/7 oracles
                    if successful_oracles < self.circuit_breaker.min_oracles_tier2 {
                        log::warn!("Price movement for {} ({:.2}%) requires {} oracles, only {} available", 
                                 currency.to_string(), change_percent * 100.0, 
                                 self.circuit_breaker.min_oracles_tier2, successful_oracles);
                        return false;
                    }
                } else if change_percent > self.circuit_breaker.tier1_threshold {
                    // Tier 1: 10%+ requires 5/7 oracles
                    if successful_oracles < self.circuit_breaker.min_oracles_tier1 {
                        log::warn!("Price movement for {} ({:.2}%) requires {} oracles, only {} available", 
                                 currency.to_string(), change_percent * 100.0, 
                                 self.circuit_breaker.min_oracles_tier1, successful_oracles);
                        return false;
                    }
                }
                
                // Update last price update timestamp
                self.last_price_update.insert(currency.clone(), Utc::now());
                
                log::info!("Price movement for {} validated: {:.2}% with {} oracles", 
                         currency.to_string(), change_percent * 100.0, successful_oracles);
                return true;
            }
        }
        true // First price always valid
    }
    
    /// Enable emergency override for governance
    pub fn enable_emergency_override(&mut self, enabled: bool) {
        self.circuit_breaker.emergency_override = enabled;
        log::info!("Emergency circuit breaker override: {}", enabled);
    }
    
    /// Get current circuit breaker status
    pub fn get_circuit_breaker_status(&self) -> &CircuitBreakerConfig {
        &self.circuit_breaker
    }

    pub fn get_latest_consensus(&self) -> Option<&ConsensusPrices> {
        self.price_history.last()
    }

    pub fn get_exchange_rates(&self) -> &ExchangeRates {
        &self.exchange_rates
    }

    pub fn get_price_history(&self, limit: usize) -> Vec<&ConsensusPrices> {
        let start = if self.price_history.len() > limit {
            self.price_history.len() - limit
        } else {
            0
        };
        self.price_history[start..].iter().collect()
    }

    /// Check and slash oracle for downtime or other offenses
    pub async fn check_and_slash_oracle(&mut self, oracle: &mut Oracle, slash_type: SlashType) -> Result<()> {
        if !oracle.is_bonded {
            return Ok(()); // Can't slash unbonded oracle
        }
        
        let should_slash = match slash_type {
            SlashType::Downtime => {
                // Check if oracle has been offline > 1 hour
                if let Some(last_timestamp) = oracle.metrics.last_price_timestamp {
                    let offline_duration = Utc::now().signed_duration_since(last_timestamp);
                    offline_duration.num_hours() > 1
                } else {
                    true // Never submitted prices
                }
            }
            SlashType::PriceDeviation => {
                // This is checked separately in check_price_deviations
                false
            }
            SlashType::Manipulation => {
                // Would require evidence analysis (simplified here)
                false
            }
        };
        
        if should_slash {
            // Get current BTC price for slashing calculation
            let btc_price = self.get_latest_consensus()
                .and_then(|c| c.btc_prices.get(&Currency::USD))
                .copied()
                .unwrap_or(50000.0);
            
            oracle.slash_bond(slash_type, btc_price)?;
        }
        
        Ok(())
    }
    
    /// Check for significant price deviations and slash oracles
    async fn check_price_deviations(&mut self, currency: &Currency, consensus_price: f64, _all_prices: &[f64]) -> Result<()> {
        let btc_price = consensus_price; // Assuming this is BTC price
        
        // Check each oracle's price against consensus
        for oracle in &mut self.oracles {
            if oracle.is_bonded {
                if let Some(oracle_price_data) = oracle.last_prices.get(currency) {
                    if let Some(oracle_price) = oracle_price_data.prices.get(currency) {
                        let deviation = (oracle_price - consensus_price).abs() / consensus_price;
                        
                        // Slash if deviation > 5%
                        if deviation > 0.05 {
                            oracle.slash_bond(SlashType::PriceDeviation, btc_price)?;
                            log::warn!("Oracle {} slashed for {:.2}% price deviation on {}", 
                                     oracle.name, deviation * 100.0, currency.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

/// Price consensus implementation (renamed from ThresholdSignature)
pub struct PriceConsensus {
    pub aggregated_hash: String,  // XOR of price data for verification
    pub participating_oracles: Vec<String>,
    pub consensus_prices: HashMap<Currency, f64>,
    pub timestamp: DateTime<Utc>,
}

impl PriceConsensus {
    /// Create a price consensus from multiple oracle data
    pub fn aggregate_prices(
        oracle_data: Vec<(String, HashMap<Currency, f64>)>, // (oracle_name, prices)
        threshold: usize,
    ) -> Result<Self> {
        if oracle_data.len() < threshold {
            return Err(BitStableError::InsufficientOracleConsensus {
                got: oracle_data.len(),
                required: threshold,
            });
        }

        // Calculate consensus prices (median per currency)
        let mut all_prices: HashMap<Currency, Vec<f64>> = HashMap::new();
        
        for (_, prices) in &oracle_data {
            for (currency, price) in prices {
                all_prices.entry(currency.clone()).or_insert_with(Vec::new).push(*price);
            }
        }

        let mut consensus_prices = HashMap::new();
        for (currency, mut prices) in all_prices {
            prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = if prices.len() % 2 == 0 {
                (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / 2.0
            } else {
                prices[prices.len() / 2]
            };
            consensus_prices.insert(currency, median);
        }

        // Create aggregated hash for verification (simplified)
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        for (oracle_name, _) in &oracle_data {
            hasher.update(oracle_name.as_bytes());
        }
        let hash = hasher.finalize();
        let aggregated_hash = hex::encode(hash);

        let participating_oracles: Vec<String> = oracle_data
            .iter()
            .map(|(name, _)| name.clone())
            .collect();

        Ok(PriceConsensus {
            aggregated_hash,
            participating_oracles,
            consensus_prices,
            timestamp: Utc::now(),
        })
    }

    /// Verify the price consensus
    pub fn verify(&self, expected_prices: &HashMap<Currency, f64>, tolerance: f64) -> bool {
        // Verify prices are within tolerance
        for (currency, expected_price) in expected_prices {
            if let Some(consensus_price) = self.consensus_prices.get(currency) {
                let price_diff = (consensus_price - expected_price).abs() / expected_price;
                if price_diff > tolerance {
                    return false;
                }
            }
        }

        // Verify we have enough participating oracles
        if self.participating_oracles.len() < 3 {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_consensus() {
        let mut oracle_data = Vec::new();
        
        let mut prices1 = HashMap::new();
        prices1.insert(Currency::USD, 100000.0);
        prices1.insert(Currency::EUR, 95000.0);
        oracle_data.push(("Oracle1".to_string(), prices1));
        
        let mut prices2 = HashMap::new();
        prices2.insert(Currency::USD, 100500.0);
        prices2.insert(Currency::EUR, 95500.0);
        oracle_data.push(("Oracle2".to_string(), prices2));
        
        let mut prices3 = HashMap::new();
        prices3.insert(Currency::USD, 99500.0);
        prices3.insert(Currency::EUR, 94500.0);
        oracle_data.push(("Oracle3".to_string(), prices3));
        
        let consensus = PriceConsensus::aggregate_prices(oracle_data, 3).unwrap();
        
        // Median should be middle value
        assert_eq!(consensus.consensus_prices.get(&Currency::USD), Some(&100000.0));
        assert_eq!(consensus.consensus_prices.get(&Currency::EUR), Some(&95000.0));
    }
}


/// Statistics for bonded oracle network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BondedOracleStats {
    pub total_oracles: usize,
    pub bonded_oracles: usize,
    pub bonding_percentage: f64,
    pub total_bonds: Amount,
    pub total_slashed: Amount,
    pub average_reputation: f64,
    pub minimum_required_bonded: usize,
}
