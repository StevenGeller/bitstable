use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig};
use crate::multi_currency::{Currency, MultiCurrencyDebt, ExchangeRates, CurrencyConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: Txid,
    pub owner: PublicKey,
    pub collateral_btc: Amount,
    pub debts: MultiCurrencyDebt,  // Changed from stable_debt_usd
    pub created_at: DateTime<Utc>,
    pub last_fee_update: DateTime<Utc>,
    pub state: VaultState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VaultState {
    Active,
    Liquidating,
    Liquidated,
    Closed,
}

impl Vault {
    pub fn new(
        id: Txid,
        owner: PublicKey,
        collateral: Amount,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            owner,
            collateral_btc: collateral,
            debts: MultiCurrencyDebt::new(),
            created_at: now,
            last_fee_update: now,
            state: VaultState::Active,
        }
    }

    /// Add debt in a specific currency
    pub fn mint_debt(&mut self, currency: Currency, amount: f64) -> Result<()> {
        self.debts.add_debt(currency, amount)
    }

    /// Remove debt in a specific currency
    pub fn burn_debt(&mut self, currency: Currency, amount: f64) -> Result<()> {
        self.debts.remove_debt(currency, amount)
    }

    /// Calculate collateral ratio using total debt in USD
    pub fn collateral_ratio(&self, exchange_rates: &ExchangeRates) -> f64 {
        let btc_price_usd = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0);
        let collateral_value_usd = self.collateral_btc.to_btc() * btc_price_usd;
        let total_debt_usd = self.debts.total_debt_in_usd(exchange_rates);
        
        if total_debt_usd == 0.0 {
            f64::INFINITY
        } else {
            collateral_value_usd / total_debt_usd
        }
    }

    /// Calculate collateral ratio for a specific currency
    pub fn collateral_ratio_for_currency(&self, currency: &Currency, exchange_rates: &ExchangeRates) -> f64 {
        let btc_price = exchange_rates.calculate_btc_price(currency, 
            exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0));
        let collateral_value = self.collateral_btc.to_btc() * btc_price;
        let debt = self.debts.get_debt(currency);
        
        if debt == 0.0 {
            f64::INFINITY
        } else {
            collateral_value / debt
        }
    }

    /// Check if vault is liquidatable based on worst currency ratio
    pub fn is_liquidatable(&self, exchange_rates: &ExchangeRates, currency_configs: &HashMap<Currency, CurrencyConfig>) -> bool {
        if self.state != VaultState::Active {
            return false;
        }

        // Check each currency's collateral ratio against its threshold
        for (currency, _) in self.debts.debts.iter() {
            let config = currency_configs.get(currency);
            if let Some(config) = config {
                let ratio = self.collateral_ratio_for_currency(currency, exchange_rates);
                if ratio < config.liquidation_threshold {
                    return true;
                }
            }
        }
        
        false
    }

    /// Calculate liquidation bonus
    pub fn liquidation_bonus(&self, exchange_rates: &ExchangeRates, penalty_rate: f64) -> Amount {
        let btc_price_usd = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0);
        let total_debt_usd = self.debts.total_debt_in_usd(exchange_rates);
        let debt_in_btc = total_debt_usd / btc_price_usd;
        let bonus = debt_in_btc * penalty_rate;
        Amount::from_btc(bonus).unwrap_or(Amount::ZERO)
    }

    /// Update stability fees for all currencies
    pub fn update_stability_fees(&mut self, currency_configs: &HashMap<Currency, CurrencyConfig>) -> Result<()> {
        let now = Utc::now();
        let time_diff = now.signed_duration_since(self.last_fee_update);
        let years = time_diff.num_seconds() as f64 / (365.25 * 24.0 * 3600.0);
        
        let mut new_debts = self.debts.clone();
        
        for (currency, debt_amount) in self.debts.debts.iter() {
            if let Some(config) = currency_configs.get(currency) {
                let fee = debt_amount * config.stability_fee_apr * years;
                new_debts.add_debt(currency.clone(), fee)?;
            }
        }
        
        self.debts = new_debts;
        self.last_fee_update = now;
        
        Ok(())
    }

    /// Calculate the correct liquidation price threshold
    /// P_liq = (total_debt × liquidation_threshold) / collateral_btc
    pub fn calculate_liquidation_price(&self, currency: &Currency, exchange_rates: &ExchangeRates, liquidation_threshold: f64) -> f64 {
        let debt_in_currency = self.debts.get_debt(currency);
        if debt_in_currency == 0.0 {
            return 0.0;
        }

        // Convert debt to USD if needed
        let debt_usd = if currency == &Currency::USD {
            debt_in_currency
        } else {
            let rate_to_usd = exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
            debt_in_currency * rate_to_usd
        };

        // P_liq = (debt × threshold) / collateral
        (debt_usd * liquidation_threshold) / self.collateral_btc.to_btc()
    }
}

#[derive(Debug)]
pub struct VaultManager {
    vaults: HashMap<Txid, Vault>,
    config: ProtocolConfig,
    currency_configs: HashMap<Currency, CurrencyConfig>,
    exchange_rates: ExchangeRates,
    db: sled::Db,
}

impl VaultManager {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        let db = sled::open(&config.database_path)?;
        
        // Initialize with default currency configurations
        let mut currency_configs = HashMap::new();
        currency_configs.insert(Currency::USD, CurrencyConfig::default());
        
        let mut manager = Self {
            vaults: HashMap::new(),
            config: config.clone(),
            currency_configs,
            exchange_rates: ExchangeRates::new(),
            db,
        };
        
        manager.load_vaults()?;
        Ok(manager)
    }

    /// Add support for a new currency
    pub fn add_currency(&mut self, currency: Currency, config: CurrencyConfig) {
        self.currency_configs.insert(currency, config);
    }

    /// Update exchange rates
    pub fn update_exchange_rates(&mut self, rates: ExchangeRates) {
        self.exchange_rates = rates;
    }

    pub async fn create_vault(
        &mut self,
        owner: PublicKey,
        collateral: Amount,
        currency: Currency,
        stable_amount: f64,
    ) -> Result<Txid> {
        // Get currency configuration
        let currency_config = self.currency_configs.get(&currency)
            .ok_or_else(|| BitStableError::InvalidConfig(format!("Currency {} not supported", currency.to_string())))?;

        // Check if currency is enabled
        if !currency_config.enabled {
            return Err(BitStableError::InvalidConfig(format!("Currency {} is disabled", currency.to_string())));
        }

        // Check minimum mint amount
        if stable_amount < currency_config.min_mint_amount {
            return Err(BitStableError::InvalidConfig(
                format!("Minimum mint amount for {} is {}", currency.to_string(), currency_config.min_mint_amount)
            ));
        }

        // Calculate required collateral
        let btc_price = self.exchange_rates.calculate_btc_price(&currency, 
            self.exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0));
        
        let collateral_value = collateral.to_btc() * btc_price;
        let required_collateral = stable_amount * currency_config.min_collateral_ratio;
        
        if collateral_value < required_collateral {
            return Err(BitStableError::InsufficientCollateral {
                required: required_collateral,
                provided: collateral_value,
            });
        }

        // Generate vault ID
        let vault_id = self.generate_vault_id();
        
        if self.vaults.contains_key(&vault_id) {
            return Err(BitStableError::VaultAlreadyExists(vault_id));
        }

        // Create vault with multi-currency support
        let mut vault = Vault::new(vault_id, owner, collateral);
        vault.mint_debt(currency.clone(), stable_amount)?;
        
        // Store in database
        self.store_vault(&vault)?;
        
        // Store in memory
        self.vaults.insert(vault_id, vault);
        
        log::info!("Created vault {} with {} BTC collateral for {} {}", 
                  vault_id, collateral.to_btc(), stable_amount, currency.to_string());
        
        Ok(vault_id)
    }

    /// Mint additional stable value in a specific currency
    pub async fn mint_additional(
        &mut self,
        vault_id: Txid,
        currency: Currency,
        amount: f64,
    ) -> Result<()> {
        // Check collateral ratio first without borrowing conflicts
        let currency_config = self.currency_configs.get(&currency)
            .ok_or_else(|| BitStableError::InvalidConfig(format!("Currency {} not supported", currency.to_string())))?
            .clone();
        let exchange_rates = self.exchange_rates.clone();
        
        {
            let vault = self.get_vault_mut(vault_id)?;
            
            // Check if minting would violate collateral ratio
            let mut test_vault = vault.clone();
            test_vault.mint_debt(currency.clone(), amount)?;
            
            let new_ratio = test_vault.collateral_ratio_for_currency(&currency, &exchange_rates);
            if new_ratio < currency_config.min_collateral_ratio {
                return Err(BitStableError::InsufficientCollateral {
                    required: currency_config.min_collateral_ratio,
                    provided: new_ratio,
                });
            }
            
            // Apply the mint
            vault.mint_debt(currency.clone(), amount)?;
        }
        
        // Store after releasing the mutable borrow
        let vault = self.get_vault(vault_id)?;
        self.store_vault(vault)?;
        
        Ok(())
    }

    /// Burn stable value in a specific currency
    pub async fn burn_stable(
        &mut self,
        vault_id: Txid,
        currency: Currency,
        amount: f64,
    ) -> Result<()> {
        {
            let vault = self.get_vault_mut(vault_id)?;
            vault.burn_debt(currency, amount)?;
        }
        
        // Store after releasing the mutable borrow
        let vault = self.get_vault(vault_id)?;
        self.store_vault(vault)?;
        Ok(())
    }

    pub fn get_vault(&self, vault_id: Txid) -> Result<&Vault> {
        self.vaults.get(&vault_id).ok_or(BitStableError::VaultNotFound(vault_id))
    }

    pub fn get_vault_mut(&mut self, vault_id: Txid) -> Result<&mut Vault> {
        self.vaults.get_mut(&vault_id).ok_or(BitStableError::VaultNotFound(vault_id))
    }

    pub fn list_vaults(&self) -> Vec<&Vault> {
        self.vaults.values().collect()
    }

    pub fn list_liquidatable_vaults(&self) -> Vec<&Vault> {
        self.vaults
            .values()
            .filter(|vault| vault.is_liquidatable(&self.exchange_rates, &self.currency_configs))
            .collect()
    }

    pub async fn liquidate_vault(&mut self, vault_id: Txid, liquidator: PublicKey) -> Result<()> {
        // Check liquidation conditions first without borrowing conflicts
        let exchange_rates = self.exchange_rates.clone();
        let currency_configs = self.currency_configs.clone();
        
        {
            let vault = self.get_vault_mut(vault_id)?;
            
            if !vault.is_liquidatable(&exchange_rates, &currency_configs) {
                return Err(BitStableError::LiquidationNotPossible {
                    ratio: vault.collateral_ratio(&exchange_rates)
                });
            }

            vault.state = VaultState::Liquidated;
        }
        
        // Store after releasing the mutable borrow
        let vault = self.get_vault(vault_id)?;
        self.store_vault(vault)?;
        
        log::info!("Vault {} liquidated by {}", vault_id, liquidator);
        
        Ok(())
    }

    pub async fn close_vault(&mut self, vault_id: Txid, owner: PublicKey) -> Result<Amount> {
        let collateral_to_return = {
            let vault = self.get_vault_mut(vault_id)?;
            
            if vault.owner != owner {
                return Err(BitStableError::InvalidConfig("Only vault owner can close vault".to_string()));
            }

            if vault.state != VaultState::Active {
                return Err(BitStableError::InvalidConfig("Vault is not active".to_string()));
            }

            if !vault.debts.is_empty() {
                return Err(BitStableError::InvalidConfig("Cannot close vault with outstanding debt".to_string()));
            }

            let collateral_to_return = vault.collateral_btc;
            vault.state = VaultState::Closed;
            vault.collateral_btc = Amount::ZERO;
            
            collateral_to_return
        };
        
        // Store the updated vault after releasing the mutable borrow
        let vault = self.get_vault(vault_id)?;
        self.store_vault(vault)?;
        
        Ok(collateral_to_return)
    }

    pub fn update_all_stability_fees(&mut self) -> Result<()> {
        let vault_ids: Vec<Txid> = self.vaults.keys().copied().collect();
        
        for vault_id in vault_ids {
            if let Some(vault) = self.vaults.get_mut(&vault_id) {
                if vault.state == VaultState::Active {
                    vault.update_stability_fees(&self.currency_configs)?;
                    let vault_clone = vault.clone();
                    self.store_vault(&vault_clone)?;
                }
            }
        }
        Ok(())
    }

    /// Get total debt across all vaults for a specific currency
    pub fn get_total_debt(&self, currency: &Currency) -> f64 {
        self.vaults.values()
            .filter(|v| v.state == VaultState::Active)
            .map(|v| v.debts.get_debt(currency))
            .sum()
    }

    /// Get total debt across all vaults in USD
    pub fn get_total_debt_usd(&self) -> f64 {
        self.vaults.values()
            .filter(|v| v.state == VaultState::Active)
            .map(|v| v.debts.total_debt_in_usd(&self.exchange_rates))
            .sum()
    }

    fn generate_vault_id(&self) -> Txid {
        use rand::RngCore;
        use bitcoin::hashes::{Hash, sha256d};
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        Txid::from_raw_hash(sha256d::Hash::from_byte_array(bytes))
    }

    fn store_vault(&self, vault: &Vault) -> Result<()> {
        let key = vault.id.to_string();
        let value = serde_json::to_vec(vault)?;
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    fn load_vaults(&mut self) -> Result<()> {
        for item in self.db.iter() {
            let (_, value) = item?;
            let vault: Vault = serde_json::from_slice(&value)?;
            self.vaults.insert(vault.id, vault);
        }
        log::info!("Loaded {} vaults from database", self.vaults.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multi_currency::{Currency, CurrencyConfig, ExchangeRates};
    use bitcoin::hashes::Hash;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};

    #[test]
    fn test_multi_currency_vault() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let owner = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let mut vault = Vault::new(vault_id, owner, Amount::from_btc(1.0).unwrap());
        
        vault.mint_debt(Currency::USD, 50000.0).unwrap();
        vault.mint_debt(Currency::EUR, 10000.0).unwrap();
        
        assert_eq!(vault.debts.get_debt(&Currency::USD), 50000.0);
        assert_eq!(vault.debts.get_debt(&Currency::EUR), 10000.0);
    }

    #[test]
    fn test_liquidation_price_calculation() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let owner = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let mut vault = Vault::new(vault_id, owner, Amount::from_btc(1.0).unwrap());
        
        vault.mint_debt(Currency::USD, 50000.0).unwrap();
        
        let mut exchange_rates = ExchangeRates::new();
        exchange_rates.update_btc_price(Currency::USD, 100000.0);
        
        // With 1 BTC collateral, 50000 USD debt, and 120% liquidation threshold
        // P_liq = (50000 × 1.2) / 1.0 = 60000
        let liq_price = vault.calculate_liquidation_price(&Currency::USD, &exchange_rates, 1.2);
        assert_eq!(liq_price, 60000.0);
    }
}
