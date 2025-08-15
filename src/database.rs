/// Database persistence layer for BitStable protocol
/// Uses sled for embedded key-value storage

use sled::{Db, Tree};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use bitcoin::{Txid, PublicKey, Amount};
use crate::{BitStableError, Result, Vault, VaultState, Currency};
use std::path::Path;
use chrono::{DateTime, Utc};

/// Database manager for persistent storage
#[derive(Debug)]
pub struct DatabaseManager {
    db: Db,
    vaults_tree: Tree,
    liquidations_tree: Tree,
    settlements_tree: Tree,
    oracle_prices_tree: Tree,
    config_tree: Tree,
}

impl DatabaseManager {
    /// Create a new database manager
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open database: {}", e)))?;
        
        let vaults_tree = db.open_tree("vaults")
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open vaults tree: {}", e)))?;
        
        let liquidations_tree = db.open_tree("liquidations")
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open liquidations tree: {}", e)))?;
        
        let settlements_tree = db.open_tree("settlements")
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open settlements tree: {}", e)))?;
        
        let oracle_prices_tree = db.open_tree("oracle_prices")
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open oracle prices tree: {}", e)))?;
        
        let config_tree = db.open_tree("config")
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to open config tree: {}", e)))?;
        
        Ok(Self {
            db,
            vaults_tree,
            liquidations_tree,
            settlements_tree,
            oracle_prices_tree,
            config_tree,
        })
    }

    /// Save a vault to the database
    pub fn save_vault(&self, vault: &Vault) -> Result<()> {
        let key = vault.id.to_string();
        let value = serde_json::to_vec(vault)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to serialize vault: {}", e)))?;
        
        self.vaults_tree.insert(key, value)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to save vault: {}", e)))?;
        
        self.db.flush()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to flush database: {}", e)))?;
        
        log::debug!("Saved vault {} to database", vault.id);
        Ok(())
    }

    /// Load a vault from the database
    pub fn load_vault(&self, vault_id: Txid) -> Result<Vault> {
        let key = vault_id.to_string();
        
        let value = self.vaults_tree.get(key)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to read vault: {}", e)))?
            .ok_or_else(|| BitStableError::VaultNotFound(vault_id))?;
        
        let vault: Vault = serde_json::from_slice(&value)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to deserialize vault: {}", e)))?;
        
        Ok(vault)
    }

    /// List all vaults
    pub fn list_vaults(&self) -> Result<Vec<Vault>> {
        let mut vaults = Vec::new();
        
        for item in self.vaults_tree.iter() {
            let (_, value) = item
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to iterate vaults: {}", e)))?;
            
            let vault: Vault = serde_json::from_slice(&value)
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to deserialize vault: {}", e)))?;
            
            vaults.push(vault);
        }
        
        Ok(vaults)
    }

    /// Delete a vault
    pub fn delete_vault(&self, vault_id: Txid) -> Result<()> {
        let key = vault_id.to_string();
        
        self.vaults_tree.remove(key)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to delete vault: {}", e)))?;
        
        self.db.flush()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to flush database: {}", e)))?;
        
        log::debug!("Deleted vault {} from database", vault_id);
        Ok(())
    }

    /// Save liquidation record
    pub fn save_liquidation(&self, liquidation: &LiquidationRecord) -> Result<()> {
        let key = format!("{}:{}", liquidation.vault_id, liquidation.liquidated_at.timestamp());
        let value = serde_json::to_vec(liquidation)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to serialize liquidation: {}", e)))?;
        
        self.liquidations_tree.insert(key, value)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to save liquidation: {}", e)))?;
        
        self.db.flush()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to flush database: {}", e)))?;
        
        Ok(())
    }

    /// Get liquidation history
    pub fn get_liquidation_history(&self, limit: Option<usize>) -> Result<Vec<LiquidationRecord>> {
        let mut liquidations = Vec::new();
        
        for item in self.liquidations_tree.iter().rev() {
            let (_, value) = item
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to iterate liquidations: {}", e)))?;
            
            let liquidation: LiquidationRecord = serde_json::from_slice(&value)
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to deserialize liquidation: {}", e)))?;
            
            liquidations.push(liquidation);
            
            if let Some(limit) = limit {
                if liquidations.len() >= limit {
                    break;
                }
            }
        }
        
        Ok(liquidations)
    }

    /// Save oracle price data
    pub fn save_oracle_price(&self, price_data: &OraclePriceRecord) -> Result<()> {
        let key = format!("{}", price_data.timestamp.timestamp());
        let value = serde_json::to_vec(price_data)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to serialize price data: {}", e)))?;
        
        self.oracle_prices_tree.insert(key, value)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to save price data: {}", e)))?;
        
        // Keep only last 10000 price records
        let count = self.oracle_prices_tree.len();
        if count > 10000 {
            if let Some(Ok((key, _))) = self.oracle_prices_tree.iter().next() {
                self.oracle_prices_tree.remove(key)
                    .map_err(|e| BitStableError::InvalidConfig(format!("Failed to remove old price: {}", e)))?;
            }
        }
        
        Ok(())
    }

    /// Get oracle price history
    pub fn get_price_history(&self, limit: usize) -> Result<Vec<OraclePriceRecord>> {
        let mut prices = Vec::new();
        
        for item in self.oracle_prices_tree.iter().rev().take(limit) {
            let (_, value) = item
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to iterate prices: {}", e)))?;
            
            let price: OraclePriceRecord = serde_json::from_slice(&value)
                .map_err(|e| BitStableError::InvalidConfig(format!("Failed to deserialize price: {}", e)))?;
            
            prices.push(price);
        }
        
        prices.reverse(); // Return in chronological order
        Ok(prices)
    }

    /// Save configuration value
    pub fn save_config<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let serialized = serde_json::to_vec(value)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to serialize config: {}", e)))?;
        
        self.config_tree.insert(key, serialized)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to save config: {}", e)))?;
        
        self.db.flush()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to flush database: {}", e)))?;
        
        Ok(())
    }

    /// Load configuration value
    pub fn load_config<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.config_tree.get(key) {
            Ok(Some(value)) => {
                let deserialized = serde_json::from_slice(&value)
                    .map_err(|e| BitStableError::InvalidConfig(format!("Failed to deserialize config: {}", e)))?;
                Ok(Some(deserialized))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(BitStableError::InvalidConfig(format!("Failed to read config: {}", e))),
        }
    }

    /// Get database statistics
    pub fn get_stats(&self) -> DatabaseStats {
        DatabaseStats {
            total_vaults: self.vaults_tree.len(),
            total_liquidations: self.liquidations_tree.len(),
            total_settlements: self.settlements_tree.len(),
            total_price_records: self.oracle_prices_tree.len(),
            database_size_bytes: self.db.size_on_disk().unwrap_or(0),
        }
    }

    /// Clear all data (use with caution!)
    pub fn clear_all(&self) -> Result<()> {
        self.vaults_tree.clear()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to clear vaults: {}", e)))?;
        
        self.liquidations_tree.clear()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to clear liquidations: {}", e)))?;
        
        self.settlements_tree.clear()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to clear settlements: {}", e)))?;
        
        self.oracle_prices_tree.clear()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to clear prices: {}", e)))?;
        
        self.db.flush()
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to flush database: {}", e)))?;
        
        log::warn!("Cleared all database data");
        Ok(())
    }

    /// Backup database to a file
    pub fn backup<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Export all data to JSON
        let backup_data = DatabaseBackup {
            timestamp: Utc::now(),
            vaults: self.list_vaults()?,
            liquidations: self.get_liquidation_history(None)?,
            prices: self.get_price_history(1000)?,
        };
        
        let json = serde_json::to_string_pretty(&backup_data)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to serialize backup: {}", e)))?;
        
        std::fs::write(path, json)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to write backup: {}", e)))?;
        
        log::info!("Database backed up successfully");
        Ok(())
    }

    /// Restore database from a backup file
    pub fn restore<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to read backup: {}", e)))?;
        
        let backup_data: DatabaseBackup = serde_json::from_str(&json)
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to parse backup: {}", e)))?;
        
        // Clear existing data
        self.clear_all()?;
        
        // Restore vaults
        for vault in backup_data.vaults {
            self.save_vault(&vault)?;
        }
        
        // Restore liquidations
        for liquidation in backup_data.liquidations {
            self.save_liquidation(&liquidation)?;
        }
        
        // Restore prices
        for price in backup_data.prices {
            self.save_oracle_price(&price)?;
        }
        
        log::info!("Database restored from backup (timestamp: {})", backup_data.timestamp);
        Ok(())
    }
}

/// Liquidation record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationRecord {
    pub vault_id: Txid,
    pub liquidator: PublicKey,
    pub collateral_seized: Amount,
    pub debt_covered: f64,
    pub bonus_paid: Amount,
    pub liquidated_at: DateTime<Utc>,
    pub btc_price: f64,
}

/// Oracle price record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePriceRecord {
    pub price_usd: f64,
    pub timestamp: DateTime<Utc>,
    pub participating_oracles: usize,
    pub total_oracles: usize,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_vaults: usize,
    pub total_liquidations: usize,
    pub total_settlements: usize,
    pub total_price_records: usize,
    pub database_size_bytes: u64,
}

/// Database backup structure
#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseBackup {
    pub timestamp: DateTime<Utc>,
    pub vaults: Vec<Vault>,
    pub liquidations: Vec<LiquidationRecord>,
    pub prices: Vec<OraclePriceRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use bitcoin::hashes::Hash;
    
    #[test]
    fn test_database_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = DatabaseManager::new(temp_dir.path()).unwrap();
        
        // Create a test vault
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let owner = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap();
        let mut vault = Vault::new(vault_id, owner, Amount::from_btc(1.0).unwrap());
        vault.debts.add_debt(Currency::USD, 50000.0).unwrap();
        
        // Save vault
        db.save_vault(&vault).unwrap();
        
        // Load vault
        let loaded_vault = db.load_vault(vault.id).unwrap();
        assert_eq!(loaded_vault.id, vault.id);
        assert_eq!(loaded_vault.debts.get_debt(&Currency::USD), vault.debts.get_debt(&Currency::USD));
        
        // List vaults
        let vaults = db.list_vaults().unwrap();
        assert_eq!(vaults.len(), 1);
        
        // Delete vault
        db.delete_vault(vault.id).unwrap();
        let vaults = db.list_vaults().unwrap();
        assert_eq!(vaults.len(), 0);
    }
    
    #[test]
    fn test_price_history() {
        let temp_dir = TempDir::new().unwrap();
        let db = DatabaseManager::new(temp_dir.path()).unwrap();
        
        // Save multiple price records
        for i in 0..5 {
            let price = OraclePriceRecord {
                price_usd: 50000.0 + (i as f64 * 100.0),
                timestamp: Utc::now(),
                participating_oracles: 3,
                total_oracles: 5,
            };
            db.save_oracle_price(&price).unwrap();
        }
        
        // Get price history
        let history = db.get_price_history(3).unwrap();
        assert_eq!(history.len(), 3);
        
        // Check chronological order
        for i in 1..history.len() {
            assert!(history[i].timestamp >= history[i-1].timestamp);
        }
    }
    
    #[test]
    fn test_backup_restore() {
        let temp_dir = TempDir::new().unwrap();
        let db = DatabaseManager::new(temp_dir.path().join("db")).unwrap();
        
        // Create test data
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let owner = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap();
        let mut vault = Vault::new(vault_id, owner, Amount::from_btc(1.0).unwrap());
        vault.debts.add_debt(Currency::USD, 50000.0).unwrap();
        
        db.save_vault(&vault).unwrap();
        
        // Backup
        let backup_path = temp_dir.path().join("backup.json");
        db.backup(&backup_path).unwrap();
        
        // Clear database
        db.clear_all().unwrap();
        assert_eq!(db.list_vaults().unwrap().len(), 0);
        
        // Restore
        db.restore(&backup_path).unwrap();
        
        // Verify restoration
        let vaults = db.list_vaults().unwrap();
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].id, vault.id);
    }
}
