use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: Txid,
    pub owner: PublicKey,
    pub collateral_btc: Amount,
    pub stable_debt_usd: f64,
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
        stable_debt: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            owner,
            collateral_btc: collateral,
            stable_debt_usd: stable_debt,
            created_at: now,
            last_fee_update: now,
            state: VaultState::Active,
        }
    }

    pub fn collateral_ratio(&self, btc_price_usd: f64) -> f64 {
        let collateral_value = self.collateral_btc.to_btc() * btc_price_usd;
        if self.stable_debt_usd == 0.0 {
            f64::INFINITY
        } else {
            collateral_value / self.stable_debt_usd
        }
    }

    pub fn is_liquidatable(&self, btc_price_usd: f64, threshold: f64) -> bool {
        self.state == VaultState::Active && self.collateral_ratio(btc_price_usd) < threshold
    }

    pub fn liquidation_bonus(&self, btc_price_usd: f64, penalty_rate: f64) -> Amount {
        let debt_in_btc = self.stable_debt_usd / btc_price_usd;
        let bonus = debt_in_btc * penalty_rate;
        Amount::from_btc(bonus).unwrap_or(Amount::ZERO)
    }

    pub fn update_stability_fees(&mut self, apr: f64) -> Result<()> {
        let now = Utc::now();
        let time_diff = now.signed_duration_since(self.last_fee_update);
        let years = time_diff.num_seconds() as f64 / (365.25 * 24.0 * 3600.0);
        
        let fee = self.stable_debt_usd * apr * years;
        self.stable_debt_usd += fee;
        self.last_fee_update = now;
        
        Ok(())
    }
}

#[derive(Debug)]
pub struct VaultManager {
    vaults: HashMap<Txid, Vault>,
    config: ProtocolConfig,
    db: sled::Db,
}

impl VaultManager {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        let db = sled::open(&config.database_path)?;
        let mut manager = Self {
            vaults: HashMap::new(),
            config: config.clone(),
            db,
        };
        
        manager.load_vaults()?;
        Ok(manager)
    }

    pub async fn create_vault(
        &mut self,
        owner: PublicKey,
        collateral: Amount,
        stable_amount: f64,
        btc_price: f64,
    ) -> Result<Txid> {
        // Validate collateral ratio
        let collateral_value = collateral.to_btc() * btc_price;
        let required_collateral = stable_amount * self.config.min_collateral_ratio;
        
        if collateral_value < required_collateral {
            return Err(BitStableError::InsufficientCollateral {
                required: required_collateral,
                provided: collateral_value,
            });
        }

        // Generate vault ID (in real implementation, this would be a transaction hash)
        let vault_id = self.generate_vault_id();
        
        if self.vaults.contains_key(&vault_id) {
            return Err(BitStableError::VaultAlreadyExists(vault_id));
        }

        let vault = Vault::new(vault_id, owner, collateral, stable_amount);
        
        // Store in database
        self.store_vault(&vault)?;
        
        // Store in memory
        self.vaults.insert(vault_id, vault);
        
        log::info!("Created vault {} with {} BTC collateral for {} USD", 
                  vault_id, collateral.to_btc(), stable_amount);
        
        Ok(vault_id)
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

    pub fn list_liquidatable_vaults(&self, btc_price: f64) -> Vec<&Vault> {
        self.vaults
            .values()
            .filter(|vault| vault.is_liquidatable(btc_price, self.config.liquidation_threshold))
            .collect()
    }

    pub async fn liquidate_vault(&mut self, vault_id: Txid, liquidator: PublicKey, btc_price: f64) -> Result<()> {
        let liquidation_threshold = self.config.liquidation_threshold;
        
        let vault = self.get_vault_mut(vault_id)?;
        
        if !vault.is_liquidatable(btc_price, liquidation_threshold) {
            return Err(BitStableError::LiquidationNotPossible {
                ratio: vault.collateral_ratio(btc_price)
            });
        }

        vault.state = VaultState::Liquidated;
        let vault_clone = vault.clone();
        self.store_vault(&vault_clone)?;
        
        log::info!("Vault {} liquidated by {}", vault_id, liquidator);
        
        Ok(())
    }

    pub async fn close_vault(&mut self, vault_id: Txid, owner: PublicKey) -> Result<Amount> {
        let vault = self.get_vault_mut(vault_id)?;
        
        if vault.owner != owner {
            return Err(BitStableError::InvalidConfig("Only vault owner can close vault".to_string()));
        }

        if vault.state != VaultState::Active {
            return Err(BitStableError::InvalidConfig("Vault is not active".to_string()));
        }

        let collateral_to_return = vault.collateral_btc;
        vault.state = VaultState::Closed;
        vault.stable_debt_usd = 0.0;
        vault.collateral_btc = Amount::ZERO;
        
        let vault_clone = vault.clone();
        self.store_vault(&vault_clone)?;
        
        Ok(collateral_to_return)
    }

    pub fn update_all_stability_fees(&mut self) -> Result<()> {
        let vault_ids: Vec<Txid> = self.vaults.keys().copied().collect();
        
        for vault_id in vault_ids {
            if let Some(vault) = self.vaults.get_mut(&vault_id) {
                if vault.state == VaultState::Active {
                    vault.update_stability_fees(self.config.stability_fee_apr)?;
                    let vault_clone = vault.clone();
                    self.store_vault(&vault_clone)?;
                }
            }
        }
        Ok(())
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