// Multi-currency support for BitStable
use bitcoin::{PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result};

/// Supported currency codes
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Currency {
    USD,
    EUR,
    GBP,
    JPY,
    CHF,
    CAD,
    AUD,
    CNY,
    INR,
    MXN,
    NGN,
    BRL,
    Custom(String),
}

impl Currency {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "USD" => Currency::USD,
            "EUR" => Currency::EUR,
            "GBP" => Currency::GBP,
            "JPY" => Currency::JPY,
            "CHF" => Currency::CHF,
            "CAD" => Currency::CAD,
            "AUD" => Currency::AUD,
            "CNY" => Currency::CNY,
            "INR" => Currency::INR,
            "MXN" => Currency::MXN,
            "NGN" => Currency::NGN,
            "BRL" => Currency::BRL,
            other => Currency::Custom(other.to_string()),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Currency::USD => "USD".to_string(),
            Currency::EUR => "EUR".to_string(),
            Currency::GBP => "GBP".to_string(),
            Currency::JPY => "JPY".to_string(),
            Currency::CHF => "CHF".to_string(),
            Currency::CAD => "CAD".to_string(),
            Currency::AUD => "AUD".to_string(),
            Currency::CNY => "CNY".to_string(),
            Currency::INR => "INR".to_string(),
            Currency::MXN => "MXN".to_string(),
            Currency::NGN => "NGN".to_string(),
            Currency::BRL => "BRL".to_string(),
            Currency::Custom(s) => s.clone(),
        }
    }
}

/// Multi-currency vault debt tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyDebt {
    pub debts: HashMap<Currency, f64>,
    pub last_updated: DateTime<Utc>,
}

impl MultiCurrencyDebt {
    pub fn new() -> Self {
        Self {
            debts: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    pub fn add_debt(&mut self, currency: Currency, amount: f64) -> Result<()> {
        if amount < 0.0 {
            return Err(BitStableError::InvalidConfig("Debt amount must be positive".to_string()));
        }
        
        *self.debts.entry(currency).or_insert(0.0) += amount;
        self.last_updated = Utc::now();
        Ok(())
    }

    pub fn remove_debt(&mut self, currency: Currency, amount: f64) -> Result<()> {
        if amount < 0.0 {
            return Err(BitStableError::InvalidConfig("Debt amount must be positive".to_string()));
        }

        let current_debt = self.debts.get(&currency).copied().unwrap_or(0.0);
        if amount > current_debt {
            return Err(BitStableError::InvalidConfig(
                format!("Cannot remove {} {} debt, only {} available", 
                    amount, currency.to_string(), current_debt)
            ));
        }

        if amount == current_debt {
            self.debts.remove(&currency);
        } else {
            self.debts.insert(currency, current_debt - amount);
        }
        
        self.last_updated = Utc::now();
        Ok(())
    }

    pub fn get_debt(&self, currency: &Currency) -> f64 {
        self.debts.get(currency).copied().unwrap_or(0.0)
    }

    pub fn total_debt_in_usd(&self, exchange_rates: &ExchangeRates) -> f64 {
        self.debts.iter()
            .map(|(currency, amount)| {
                let rate = exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
                amount * rate
            })
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.debts.is_empty()
    }
}

/// Exchange rate tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRates {
    /// BTC price in each currency
    pub btc_prices: HashMap<Currency, f64>,
    /// Exchange rates to USD (e.g., EUR/USD)
    pub to_usd_rates: HashMap<Currency, f64>,
    pub timestamp: DateTime<Utc>,
}

impl ExchangeRates {
    pub fn new() -> Self {
        let mut to_usd_rates = HashMap::new();
        to_usd_rates.insert(Currency::USD, 1.0); // USD/USD = 1.0
        
        Self {
            btc_prices: HashMap::new(),
            to_usd_rates,
            timestamp: Utc::now(),
        }
    }

    pub fn update_btc_price(&mut self, currency: Currency, price: f64) {
        self.btc_prices.insert(currency, price);
        self.timestamp = Utc::now();
    }

    pub fn update_exchange_rate(&mut self, currency: Currency, rate_to_usd: f64) {
        self.to_usd_rates.insert(currency, rate_to_usd);
        self.timestamp = Utc::now();
    }

    pub fn get_btc_price(&self, currency: &Currency) -> Option<f64> {
        self.btc_prices.get(currency).copied()
    }

    pub fn get_rate_to_usd(&self, currency: &Currency) -> Option<f64> {
        self.to_usd_rates.get(currency).copied()
    }

    /// Calculate BTC price in a currency from USD price and exchange rate
    pub fn calculate_btc_price(&self, currency: &Currency, btc_usd_price: f64) -> f64 {
        if currency == &Currency::USD {
            return btc_usd_price;
        }

        // If we have direct BTC price in this currency, use it
        if let Some(price) = self.btc_prices.get(currency) {
            return *price;
        }

        // Otherwise calculate from USD price and exchange rate
        // BTC/EUR = BTC/USD * USD/EUR = BTC/USD / (EUR/USD)
        if let Some(rate_to_usd) = self.to_usd_rates.get(currency) {
            btc_usd_price / rate_to_usd
        } else {
            btc_usd_price // Fallback to USD price if no rate available
        }
    }
}

/// Multi-currency stable value position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyStableValue {
    pub amount: f64,
    pub currency: Currency,
    pub backed_by_vault: Txid,
    pub created_at: DateTime<Utc>,
    pub holder: PublicKey,
}

/// Multi-currency stable position for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyPosition {
    pub holder: PublicKey,
    pub positions: HashMap<Currency, Vec<MultiCurrencyStableValue>>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

impl MultiCurrencyPosition {
    pub fn new(holder: PublicKey) -> Self {
        let now = Utc::now();
        Self {
            holder,
            positions: HashMap::new(),
            created_at: now,
            last_updated: now,
        }
    }

    pub fn mint_stable(&mut self, currency: Currency, amount: f64, vault_id: Txid) -> Result<()> {
        if amount <= 0.0 {
            return Err(BitStableError::InvalidConfig("Amount must be positive".to_string()));
        }

        let stable_value = MultiCurrencyStableValue {
            amount,
            currency: currency.clone(),
            backed_by_vault: vault_id,
            created_at: Utc::now(),
            holder: self.holder,
        };

        self.positions
            .entry(currency)
            .or_insert_with(Vec::new)
            .push(stable_value);
        
        self.last_updated = Utc::now();
        Ok(())
    }

    pub fn burn_stable(&mut self, currency: Currency, amount: f64) -> Result<Vec<Txid>> {
        if amount <= 0.0 {
            return Err(BitStableError::InvalidConfig("Amount must be positive".to_string()));
        }

        let positions = self.positions.get_mut(&currency)
            .ok_or_else(|| BitStableError::InvalidConfig(
                format!("No {} positions to burn", currency.to_string())
            ))?;

        let total_available: f64 = positions.iter().map(|p| p.amount).sum();
        if amount > total_available {
            return Err(BitStableError::InvalidConfig(
                format!("Insufficient {} balance: {} requested, {} available", 
                    currency.to_string(), amount, total_available)
            ));
        }

        let mut remaining_to_burn = amount;
        let mut burned_vaults = Vec::new();
        let mut i = 0;

        // FIFO burning
        while remaining_to_burn > 0.0 && i < positions.len() {
            let position = &mut positions[i];
            
            if position.amount <= remaining_to_burn {
                // Burn entire position
                remaining_to_burn -= position.amount;
                burned_vaults.push(position.backed_by_vault);
                positions.remove(i);
            } else {
                // Partial burn
                position.amount -= remaining_to_burn;
                burned_vaults.push(position.backed_by_vault);
                remaining_to_burn = 0.0;
                i += 1;
            }
        }

        // Remove empty currency entry
        if positions.is_empty() {
            self.positions.remove(&currency);
        }

        self.last_updated = Utc::now();
        Ok(burned_vaults)
    }

    pub fn get_balance(&self, currency: &Currency) -> f64 {
        self.positions.get(currency)
            .map(|positions| positions.iter().map(|p| p.amount).sum())
            .unwrap_or(0.0)
    }

    pub fn get_all_balances(&self) -> HashMap<Currency, f64> {
        self.positions.iter()
            .map(|(currency, positions)| {
                let total: f64 = positions.iter().map(|p| p.amount).sum();
                (currency.clone(), total)
            })
            .collect()
    }

    pub fn total_value_in_usd(&self, exchange_rates: &ExchangeRates) -> f64 {
        self.positions.iter()
            .map(|(currency, positions)| {
                let total: f64 = positions.iter().map(|p| p.amount).sum();
                let rate = exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
                total * rate
            })
            .sum()
    }
}

/// Configuration for per-currency parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    pub stability_fee_apr: f64,
    pub liquidation_penalty: f64,
    pub min_collateral_ratio: f64,
    pub liquidation_threshold: f64,
    pub min_mint_amount: f64,
    pub enabled: bool,
}

impl Default for CurrencyConfig {
    fn default() -> Self {
        Self {
            stability_fee_apr: 0.02,      // 2% APR (matches whitepaper)
            liquidation_penalty: 0.05,     // 5% penalty (matches whitepaper)
            min_collateral_ratio: 1.5,     // 150% minimum (matches whitepaper)
            liquidation_threshold: 1.1,    // 110% liquidation (matches whitepaper)
            min_mint_amount: 10.0,         // Minimum 10 units
            enabled: true,
        }
    }
}

/// Multi-currency protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyConfig {
    pub currencies: HashMap<Currency, CurrencyConfig>,
    pub default_config: CurrencyConfig,
}

impl MultiCurrencyConfig {
    pub fn new() -> Self {
        let mut currencies = HashMap::new();
        
        // Set up default configurations for major currencies
        currencies.insert(Currency::USD, CurrencyConfig::default());
        currencies.insert(Currency::EUR, CurrencyConfig {
            stability_fee_apr: 0.025,  // Slightly higher fee for EUR
            ..Default::default()
        });
        currencies.insert(Currency::GBP, CurrencyConfig {
            stability_fee_apr: 0.03,   // Moderate fee for GBP
            ..Default::default()
        });
        currencies.insert(Currency::NGN, CurrencyConfig {
            stability_fee_apr: 0.08,  // Higher fee for emerging market currency
            liquidation_penalty: 0.08, // Higher penalty for volatile currency
            min_collateral_ratio: 1.75,  // Higher collateral requirement
            liquidation_threshold: 1.25,  // Higher liquidation threshold for risk
            ..Default::default()
        });

        Self {
            currencies,
            default_config: CurrencyConfig::default(),
        }
    }

    pub fn get_config(&self, currency: &Currency) -> &CurrencyConfig {
        self.currencies.get(currency).unwrap_or(&self.default_config)
    }

    pub fn is_currency_enabled(&self, currency: &Currency) -> bool {
        self.get_config(currency).enabled
    }

    pub fn enable_currency(&mut self, currency: Currency, config: Option<CurrencyConfig>) {
        let config = config.unwrap_or_default();
        self.currencies.insert(currency, config);
    }

    pub fn disable_currency(&mut self, currency: Currency) {
        if let Some(config) = self.currencies.get_mut(&currency) {
            config.enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};
    use bitcoin::hashes::Hash;

    #[test]
    fn test_multi_currency_debt() {
        let mut debt = MultiCurrencyDebt::new();
        
        debt.add_debt(Currency::USD, 1000.0).unwrap();
        debt.add_debt(Currency::EUR, 500.0).unwrap();
        
        assert_eq!(debt.get_debt(&Currency::USD), 1000.0);
        assert_eq!(debt.get_debt(&Currency::EUR), 500.0);
        
        debt.remove_debt(Currency::USD, 300.0).unwrap();
        assert_eq!(debt.get_debt(&Currency::USD), 700.0);
    }

    #[test]
    fn test_exchange_rates() {
        let mut rates = ExchangeRates::new();
        
        rates.update_btc_price(Currency::USD, 100000.0);
        rates.update_exchange_rate(Currency::EUR, 0.85); // 1 EUR = 0.85 USD
        
        let btc_eur = rates.calculate_btc_price(&Currency::EUR, 100000.0);
        assert_eq!(btc_eur, 100000.0 / 0.85); // ~117,647 EUR
    }

    #[test]
    fn test_multi_currency_position() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let holder = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        let mut position = MultiCurrencyPosition::new(holder);
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        
        position.mint_stable(Currency::USD, 1000.0, vault_id).unwrap();
        position.mint_stable(Currency::EUR, 500.0, vault_id).unwrap();
        
        assert_eq!(position.get_balance(&Currency::USD), 1000.0);
        assert_eq!(position.get_balance(&Currency::EUR), 500.0);
        
        let burned = position.burn_stable(Currency::USD, 300.0).unwrap();
        assert_eq!(burned.len(), 1);
        assert_eq!(position.get_balance(&Currency::USD), 700.0);
    }
}
