use bitcoin::{PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableValue {
    pub amount_usd: f64,
    pub backed_by_vault: Txid,
    pub created_at: DateTime<Utc>,
    pub holder: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablePosition {
    pub id: Txid,
    pub holder: PublicKey,
    pub total_stable_usd: f64,
    pub positions: Vec<StableValue>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

impl StablePosition {
    pub fn new(holder: PublicKey) -> Self {
        let now = Utc::now();
        Self {
            id: Self::generate_position_id(),
            holder,
            total_stable_usd: 0.0,
            positions: Vec::new(),
            created_at: now,
            last_updated: now,
        }
    }

    pub fn mint_stable(&mut self, amount_usd: f64, vault_id: Txid) -> Result<()> {
        if amount_usd <= 0.0 {
            return Err(BitStableError::InvalidConfig("Amount must be positive".to_string()));
        }

        let stable_value = StableValue {
            amount_usd,
            backed_by_vault: vault_id,
            created_at: Utc::now(),
            holder: self.holder,
        };

        self.positions.push(stable_value);
        self.total_stable_usd += amount_usd;
        self.last_updated = Utc::now();

        Ok(())
    }

    pub fn burn_stable(&mut self, amount_usd: f64) -> Result<Vec<Txid>> {
        if amount_usd <= 0.0 {
            return Err(BitStableError::InvalidConfig("Amount must be positive".to_string()));
        }

        if amount_usd > self.total_stable_usd {
            return Err(BitStableError::InvalidConfig("Insufficient stable balance".to_string()));
        }

        let mut remaining_to_burn = amount_usd;
        let mut burned_vaults = Vec::new();
        let mut i = 0;

        while remaining_to_burn > 0.0 && i < self.positions.len() {
            let position = &mut self.positions[i];
            
            if position.amount_usd <= remaining_to_burn {
                // Burn entire position
                remaining_to_burn -= position.amount_usd;
                burned_vaults.push(position.backed_by_vault);
                self.positions.remove(i);
            } else {
                // Partial burn
                position.amount_usd -= remaining_to_burn;
                burned_vaults.push(position.backed_by_vault);
                remaining_to_burn = 0.0;
                i += 1;
            }
        }

        self.total_stable_usd -= amount_usd;
        self.last_updated = Utc::now();

        Ok(burned_vaults)
    }

    pub fn transfer_stable(&mut self, to: PublicKey, amount_usd: f64) -> Result<StableTransfer> {
        if amount_usd <= 0.0 {
            return Err(BitStableError::InvalidConfig("Amount must be positive".to_string()));
        }

        if amount_usd > self.total_stable_usd {
            return Err(BitStableError::InvalidConfig("Insufficient stable balance".to_string()));
        }

        // Find positions to transfer (FIFO)
        let mut remaining_to_transfer = amount_usd;
        let mut transferred_positions = Vec::new();
        let mut i = 0;

        while remaining_to_transfer > 0.0 && i < self.positions.len() {
            let position = &mut self.positions[i];
            
            if position.amount_usd <= remaining_to_transfer {
                // Transfer entire position
                remaining_to_transfer -= position.amount_usd;
                let mut transferred_position = position.clone();
                transferred_position.holder = to;
                transferred_positions.push(transferred_position);
                self.positions.remove(i);
            } else {
                // Partial transfer
                let transfer_amount = remaining_to_transfer;
                position.amount_usd -= transfer_amount;
                
                let transferred_position = StableValue {
                    amount_usd: transfer_amount,
                    backed_by_vault: position.backed_by_vault,
                    created_at: position.created_at,
                    holder: to,
                };
                transferred_positions.push(transferred_position);
                remaining_to_transfer = 0.0;
                i += 1;
            }
        }

        self.total_stable_usd -= amount_usd;
        self.last_updated = Utc::now();

        Ok(StableTransfer {
            from: self.holder,
            to,
            amount_usd,
            positions: transferred_positions,
            timestamp: Utc::now(),
        })
    }

    fn generate_position_id() -> Txid {
        use rand::RngCore;
        use bitcoin::hashes::{Hash, sha256d};
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        Txid::from_raw_hash(sha256d::Hash::from_byte_array(bytes))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableTransfer {
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount_usd: f64,
    pub positions: Vec<StableValue>,
    pub timestamp: DateTime<Utc>,
}

pub struct StableManager {
    positions: HashMap<PublicKey, StablePosition>,
    total_supply: f64,
    transfer_history: Vec<StableTransfer>,
}

impl StableManager {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            total_supply: 0.0,
            transfer_history: Vec::new(),
        }
    }

    pub fn get_or_create_position(&mut self, holder: PublicKey) -> &mut StablePosition {
        self.positions.entry(holder).or_insert_with(|| StablePosition::new(holder))
    }

    pub fn mint_stable(&mut self, holder: PublicKey, amount_usd: f64, vault_id: Txid) -> Result<()> {
        let position = self.get_or_create_position(holder);
        position.mint_stable(amount_usd, vault_id)?;
        self.total_supply += amount_usd;
        
        log::info!("Minted {} USD stable value for {}", amount_usd, holder);
        Ok(())
    }

    pub fn burn_stable(&mut self, holder: PublicKey, amount_usd: f64) -> Result<Vec<Txid>> {
        let position = self.positions.get_mut(&holder)
            .ok_or_else(|| BitStableError::InvalidConfig("Position not found".to_string()))?;
        
        let burned_vaults = position.burn_stable(amount_usd)?;
        self.total_supply -= amount_usd;
        
        // Remove empty positions
        if position.total_stable_usd == 0.0 {
            self.positions.remove(&holder);
        }
        
        log::info!("Burned {} USD stable value for {}", amount_usd, holder);
        Ok(burned_vaults)
    }

    pub fn transfer_stable(
        &mut self,
        from: PublicKey,
        to: PublicKey,
        amount_usd: f64,
    ) -> Result<()> {
        // Execute transfer from sender
        let transfer = {
            let from_position = self.positions.get_mut(&from)
                .ok_or_else(|| BitStableError::InvalidConfig("Sender position not found".to_string()))?;
            from_position.transfer_stable(to, amount_usd)?
        };

        // Add positions to receiver
        let to_position = self.get_or_create_position(to);
        for position in transfer.positions.iter() {
            to_position.mint_stable(position.amount_usd, position.backed_by_vault)?;
        }

        // Remove empty sender position
        if let Some(from_position) = self.positions.get(&from) {
            if from_position.total_stable_usd == 0.0 {
                self.positions.remove(&from);
            }
        }

        // Record transfer
        self.transfer_history.push(transfer);
        
        log::info!("Transferred {} USD stable value from {} to {}", amount_usd, from, to);
        Ok(())
    }

    pub fn get_balance(&self, holder: PublicKey) -> f64 {
        self.positions.get(&holder)
            .map(|pos| pos.total_stable_usd)
            .unwrap_or(0.0)
    }

    pub fn get_position(&self, holder: PublicKey) -> Option<&StablePosition> {
        self.positions.get(&holder)
    }

    pub fn get_total_supply(&self) -> f64 {
        self.total_supply
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

    pub fn calculate_collateral_backing(&self, btc_price: f64, vaults: &HashMap<Txid, crate::Vault>) -> CollateralBacking {
        let mut total_collateral_value = 0.0;
        let mut total_debt = 0.0;
        let mut vault_count = 0;

        for position in self.positions.values() {
            for stable_value in &position.positions {
                if let Some(vault) = vaults.get(&stable_value.backed_by_vault) {
                    total_collateral_value += vault.collateral_btc.to_btc() * btc_price;
                    total_debt += vault.stable_debt_usd;
                    vault_count += 1;
                }
            }
        }

        let collateral_ratio = if total_debt > 0.0 {
            total_collateral_value / total_debt
        } else {
            f64::INFINITY
        };

        CollateralBacking {
            total_collateral_value_usd: total_collateral_value,
            total_stable_debt_usd: total_debt,
            overall_collateral_ratio: collateral_ratio,
            backing_vault_count: vault_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralBacking {
    pub total_collateral_value_usd: f64,
    pub total_stable_debt_usd: f64,
    pub overall_collateral_ratio: f64,
    pub backing_vault_count: usize,
}

impl Default for StableManager {
    fn default() -> Self {
        Self::new()
    }
}