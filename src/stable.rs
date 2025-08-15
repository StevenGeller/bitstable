use bitcoin::{PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result};
use crate::multi_currency::{Currency, MultiCurrencyPosition, ExchangeRates};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableTransfer {
    pub from: PublicKey,
    pub to: PublicKey,
    pub currency: Currency,
    pub amount: f64,
    pub positions_transferred: Vec<(Txid, f64)>, // (vault_id, amount)
    pub timestamp: DateTime<Utc>,
}

pub struct MultiCurrencyStableManager {
    positions: HashMap<PublicKey, MultiCurrencyPosition>,
    total_supply: HashMap<Currency, f64>,
    transfer_history: Vec<StableTransfer>,
}

impl MultiCurrencyStableManager {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            total_supply: HashMap::new(),
            transfer_history: Vec::new(),
        }
    }

    pub fn get_or_create_position(&mut self, holder: PublicKey) -> &mut MultiCurrencyPosition {
        self.positions.entry(holder).or_insert_with(|| MultiCurrencyPosition::new(holder))
    }

    pub fn mint_stable(&mut self, holder: PublicKey, currency: Currency, amount: f64, vault_id: Txid) -> Result<()> {
        let position = self.get_or_create_position(holder);
        position.mint_stable(currency.clone(), amount, vault_id)?;
        
        *self.total_supply.entry(currency.clone()).or_insert(0.0) += amount;
        
        log::info!("Minted {} {} stable value for {}", amount, currency.to_string(), holder);
        Ok(())
    }

    pub fn burn_stable(&mut self, holder: PublicKey, currency: Currency, amount: f64) -> Result<Vec<Txid>> {
        let position = self.positions.get_mut(&holder)
            .ok_or_else(|| BitStableError::InvalidConfig("Position not found".to_string()))?;
        
        let burned_vaults = position.burn_stable(currency.clone(), amount)?;
        
        if let Some(supply) = self.total_supply.get_mut(&currency) {
            *supply -= amount;
            if *supply == 0.0 {
                self.total_supply.remove(&currency);
            }
        }
        
        // Remove empty positions
        if position.positions.is_empty() {
            self.positions.remove(&holder);
        }
        
        log::info!("Burned {} {} stable value for {}", amount, currency.to_string(), holder);
        Ok(burned_vaults)
    }

    pub fn transfer_stable(
        &mut self,
        from: PublicKey,
        to: PublicKey,
        currency: Currency,
        amount: f64,
    ) -> Result<()> {
        // Validate sender has sufficient balance
        let from_balance = self.get_balance(from, &currency);
        if from_balance < amount {
            return Err(BitStableError::InvalidConfig(
                format!("Insufficient {} balance: {} available, {} requested", 
                    currency.to_string(), from_balance, amount)
            ));
        }

        // Burn from sender and track which vaults
        let burned_vaults = self.burn_stable(from, currency.clone(), amount)?;
        
        // Mint to receiver using the same vault backing
        let to_position = self.get_or_create_position(to);
        for vault_id in &burned_vaults {
            // For simplicity, distribute evenly across vaults
            let vault_amount = amount / burned_vaults.len() as f64;
            to_position.mint_stable(currency.clone(), vault_amount, *vault_id)?;
        }

        // Record transfer
        let transfer = StableTransfer {
            from,
            to,
            currency: currency.clone(),
            amount,
            positions_transferred: burned_vaults.iter()
                .map(|v| (*v, amount / burned_vaults.len() as f64))
                .collect(),
            timestamp: Utc::now(),
        };
        
        self.transfer_history.push(transfer);
        
        log::info!("Transferred {} {} stable value from {} to {}", 
                  amount, currency.to_string(), from, to);
        Ok(())
    }

    pub fn get_balance(&self, holder: PublicKey, currency: &Currency) -> f64 {
        self.positions.get(&holder)
            .map(|pos| pos.get_balance(currency))
            .unwrap_or(0.0)
    }

    pub fn get_all_balances(&self, holder: PublicKey) -> HashMap<Currency, f64> {
        self.positions.get(&holder)
            .map(|pos| pos.get_all_balances())
            .unwrap_or_default()
    }

    pub fn get_position(&self, holder: PublicKey) -> Option<&MultiCurrencyPosition> {
        self.positions.get(&holder)
    }

    pub fn get_total_supply(&self, currency: &Currency) -> f64 {
        self.total_supply.get(currency).copied().unwrap_or(0.0)
    }

    pub fn get_all_supplies(&self) -> &HashMap<Currency, f64> {
        &self.total_supply
    }

    pub fn get_holders(&self) -> Vec<PublicKey> {
        self.positions.keys().copied().collect()
    }

    pub fn get_transfer_history(&self, limit: Option<usize>) -> Vec<&StableTransfer> {
        let limit = limit.unwrap_or(self.transfer_history.len());
        let start = if self.transfer_history.len() > limit {
            self.transfer_history.len() - limit
        } else {
            0
        };
        self.transfer_history[start..].iter().collect()
    }

    pub fn calculate_collateral_backing(
        &self,
        exchange_rates: &ExchangeRates,
        vaults: &HashMap<Txid, crate::vault::Vault>,
    ) -> CollateralBacking {
        let mut total_collateral_value_usd = 0.0;
        let mut total_debt_usd = 0.0;
        let mut currency_breakdowns = HashMap::new();

        let btc_price_usd = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0);

        for position in self.positions.values() {
            for (currency, values) in &position.positions {
                let mut currency_collateral = 0.0;
                let mut currency_debt = 0.0;

                for value in values {
                    if let Some(vault) = vaults.get(&value.backed_by_vault) {
                        let vault_collateral_usd = vault.collateral_btc.to_btc() * btc_price_usd;
                        let vault_debt_usd = vault.debts.total_debt_in_usd(exchange_rates);
                        
                        // Proportional allocation based on this position's share
                        let position_ratio = value.amount / vault.debts.get_debt(currency);
                        currency_collateral += vault_collateral_usd * position_ratio;
                        currency_debt += value.amount * exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
                    }
                }

                currency_breakdowns.insert(currency.clone(), CurrencyBacking {
                    currency: currency.clone(),
                    total_supply: self.get_total_supply(currency),
                    collateral_value_usd: currency_collateral,
                    debt_value_usd: currency_debt,
                    collateral_ratio: if currency_debt > 0.0 { 
                        currency_collateral / currency_debt 
                    } else { 
                        f64::INFINITY 
                    },
                });

                total_collateral_value_usd += currency_collateral;
                total_debt_usd += currency_debt;
            }
        }

        let overall_ratio = if total_debt_usd > 0.0 {
            total_collateral_value_usd / total_debt_usd
        } else {
            f64::INFINITY
        };

        CollateralBacking {
            total_collateral_value_usd,
            total_stable_debt_usd: total_debt_usd,
            overall_collateral_ratio: overall_ratio,
            currency_breakdowns,
            backing_vault_count: vaults.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralBacking {
    pub total_collateral_value_usd: f64,
    pub total_stable_debt_usd: f64,
    pub overall_collateral_ratio: f64,
    pub currency_breakdowns: HashMap<Currency, CurrencyBacking>,
    pub backing_vault_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyBacking {
    pub currency: Currency,
    pub total_supply: f64,
    pub collateral_value_usd: f64,
    pub debt_value_usd: f64,
    pub collateral_ratio: f64,
}

impl Default for MultiCurrencyStableManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};
    use bitcoin::hashes::Hash;

    #[test]
    fn test_multi_currency_stable_manager() {
        let secp = Secp256k1::new();
        let secret_key1 = SecretKey::new(&mut rand::thread_rng());
        let holder1 = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key1, Network::Testnet));
        
        let secret_key2 = SecretKey::new(&mut rand::thread_rng());
        let holder2 = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key2, Network::Testnet));
        
        let mut manager = MultiCurrencyStableManager::new();
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        
        // Mint different currencies
        manager.mint_stable(holder1, Currency::USD, 1000.0, vault_id).unwrap();
        manager.mint_stable(holder1, Currency::EUR, 500.0, vault_id).unwrap();
        
        assert_eq!(manager.get_balance(holder1, &Currency::USD), 1000.0);
        assert_eq!(manager.get_balance(holder1, &Currency::EUR), 500.0);
        
        // Transfer
        manager.transfer_stable(holder1, holder2, Currency::USD, 300.0).unwrap();
        
        assert_eq!(manager.get_balance(holder1, &Currency::USD), 700.0);
        assert_eq!(manager.get_balance(holder2, &Currency::USD), 300.0);
        
        // Check total supply
        assert_eq!(manager.get_total_supply(&Currency::USD), 1000.0);
        assert_eq!(manager.get_total_supply(&Currency::EUR), 500.0);
    }
}
