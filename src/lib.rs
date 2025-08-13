pub mod vault;
pub mod oracle;
pub mod liquidation;
pub mod stable;
pub mod error;
pub mod config;
pub mod network;

use bitcoin::{Amount, PublicKey, Txid};

pub use error::{BitStableError, Result};
pub use vault::{Vault, VaultState, VaultManager};
pub use oracle::{Oracle, OracleNetwork, PriceData};
pub use liquidation::{LiquidationEngine, LiquidationOpportunity};
pub use stable::{StablePosition, StableValue};
pub use config::ProtocolConfig;

#[derive(Debug)]
pub struct BitStableProtocol {
    pub config: ProtocolConfig,
    pub vault_manager: VaultManager,
    pub oracle_network: OracleNetwork,
    pub liquidation_engine: LiquidationEngine,
}

impl BitStableProtocol {
    pub fn new(config: ProtocolConfig) -> Result<Self> {
        Ok(Self {
            vault_manager: VaultManager::new(&config)?,
            oracle_network: OracleNetwork::new(&config)?,
            liquidation_engine: LiquidationEngine::new(&config)?,
            config,
        })
    }

    pub async fn open_vault(
        &mut self,
        owner: PublicKey,
        collateral: Amount,
        stable_amount: f64,
    ) -> Result<Txid> {
        let price = self.oracle_network.get_consensus_price().await?;
        
        self.vault_manager.create_vault(owner, collateral, stable_amount, price).await
    }

    pub async fn liquidate_vault(&mut self, vault_id: Txid, liquidator: PublicKey) -> Result<()> {
        let price = self.oracle_network.get_consensus_price().await?;
        
        self.liquidation_engine.liquidate(vault_id, liquidator, price).await?;
        Ok(())
    }

    pub async fn get_vault_health(&mut self, vault_id: Txid) -> Result<f64> {
        let vault = self.vault_manager.get_vault(vault_id)?;
        let price = self.oracle_network.get_consensus_price().await?;
        
        Ok(vault.collateral_ratio(price))
    }
}