pub mod vault;
pub mod oracle;
pub mod liquidation;
pub mod stable;
pub mod error;
pub mod config;
pub mod network;
pub mod custody;
pub mod bitcoin_client;

use bitcoin::{Amount, PublicKey, Txid};

pub use error::{BitStableError, Result};
pub use vault::{Vault, VaultState, VaultManager};
pub use oracle::{Oracle, OracleNetwork, PriceData};
pub use liquidation::{LiquidationEngine, LiquidationOpportunity};
pub use stable::{StablePosition, StableValue};
pub use config::ProtocolConfig;
pub use custody::{CustodyManager, EscrowContract, LiquidationSettlement};
pub use bitcoin_client::{BitcoinClient, BitcoinConfig};

#[derive(Debug)]
pub struct BitStableProtocol {
    pub config: ProtocolConfig,
    pub vault_manager: VaultManager,
    pub oracle_network: OracleNetwork,
    pub liquidation_engine: LiquidationEngine,
    pub custody_manager: CustodyManager,
    pub bitcoin_client: Option<BitcoinClient>,
}

impl BitStableProtocol {
    pub fn new(config: ProtocolConfig) -> Result<Self> {
        Ok(Self {
            vault_manager: VaultManager::new(&config)?,
            oracle_network: OracleNetwork::new(&config)?,
            liquidation_engine: LiquidationEngine::new(&config)?,
            custody_manager: CustodyManager::new(&config)?,
            bitcoin_client: None,
            config,
        })
    }

    /// Initialize with Bitcoin client for on-chain operations
    pub fn with_bitcoin_client(mut self, bitcoin_config: BitcoinConfig) -> Result<Self> {
        self.bitcoin_client = Some(bitcoin_config.create_client()?);
        Ok(self)
    }

    pub async fn open_vault(
        &mut self,
        owner: PublicKey,
        collateral: Amount,
        stable_amount: f64,
    ) -> Result<EscrowContract> {
        let price = self.oracle_network.get_consensus_price().await?;
        
        // Create vault in the vault manager
        let vault_id = self.vault_manager.create_vault(owner, collateral, stable_amount, price).await?;
        
        // Calculate liquidation threshold price (110% of current debt ratio)
        let liquidation_price = price * self.config.liquidation_threshold;
        
        // Create escrow contract for Bitcoin custody
        let escrow_contract = self.custody_manager.create_vault_escrow(
            vault_id,
            owner,
            collateral,
            liquidation_price,
        )?;

        log::info!(
            "Created vault {} with escrow address {}",
            vault_id,
            escrow_contract.multisig_address
        );

        Ok(escrow_contract)
    }

    /// Fund a vault's escrow contract with actual Bitcoin
    pub async fn fund_vault_escrow(
        &mut self,
        vault_id: Txid,
        funding_txid: Txid,
        vout: u32,
        amount: Amount,
    ) -> Result<()> {
        // Verify the funding transaction if we have a Bitcoin client
        if let Some(bitcoin_client) = &self.bitcoin_client {
            let tx_info = bitcoin_client.get_transaction(funding_txid)?;
            
            // Verify the transaction has the expected output
            if let Some(output) = tx_info.outputs.get(vout as usize) {
                if output.value != amount {
                    return Err(BitStableError::InvalidConfig(
                        "Funding amount doesn't match expected amount".to_string()
                    ));
                }
            } else {
                return Err(BitStableError::InvalidConfig("Invalid funding transaction output".to_string()));
            }
        }

        // Record the funding in the custody manager
        self.custody_manager.process_vault_funding(vault_id, funding_txid, vout, amount)?;
        
        log::info!("Vault {} funded with {} BTC", vault_id, amount.to_btc());
        Ok(())
    }

    pub async fn liquidate_vault(&mut self, vault_id: Txid, liquidator: PublicKey) -> Result<Txid> {
        let price = self.oracle_network.get_consensus_price().await?;
        
        // Get vault information for liquidation calculation
        let vault = self.vault_manager.get_vault(vault_id)?;
        
        // Check if vault can be liquidated based on custody rules
        if !self.custody_manager.can_liquidate_vault(vault_id, price) {
            return Err(BitStableError::LiquidationNotPossible {
                ratio: vault.collateral_ratio(price)
            });
        }

        // Execute liquidation in the liquidation engine
        self.liquidation_engine.liquidate(vault_id, liquidator, price).await?;
        
        // Create and sign liquidation settlement transaction
        let liquidation_tx = self.custody_manager.execute_liquidation(
            vault_id,
            liquidator,
            price,
            vault.stable_debt_usd,
        )?;

        // Broadcast the transaction if we have a Bitcoin client
        if let Some(bitcoin_client) = &self.bitcoin_client {
            let txid = bitcoin_client.broadcast_transaction(&liquidation_tx)?;
            self.custody_manager.mark_transaction_broadcast(txid)?;
            
            log::info!("Liquidation transaction broadcast: {}", txid);
            Ok(txid)
        } else {
            Ok(liquidation_tx.compute_txid())
        }
    }

    /// Close a vault and return collateral to owner (when debt is repaid)
    pub async fn close_vault(&mut self, vault_id: Txid, owner: PublicKey) -> Result<Txid> {
        // Close vault in the vault manager
        let returned_collateral = self.vault_manager.close_vault(vault_id, owner).await?;
        
        // Create vault closure transaction
        let closure_tx = self.custody_manager.create_vault_closure_transaction(vault_id)?;
        
        // Broadcast the transaction if we have a Bitcoin client
        if let Some(bitcoin_client) = &self.bitcoin_client {
            let txid = bitcoin_client.broadcast_transaction(&closure_tx)?;
            
            log::info!(
                "Vault {} closed, returned {} BTC to owner in transaction {}",
                vault_id,
                returned_collateral.to_btc(),
                txid
            );
            
            Ok(txid)
        } else {
            Ok(closure_tx.compute_txid())
        }
    }

    pub async fn get_vault_health(&mut self, vault_id: Txid) -> Result<f64> {
        let vault = self.vault_manager.get_vault(vault_id)?;
        let price = self.oracle_network.get_consensus_price().await?;
        
        Ok(vault.collateral_ratio(price))
    }

    /// Get escrow contract information for a vault
    pub fn get_vault_escrow(&self, vault_id: Txid) -> Option<&EscrowContract> {
        self.custody_manager.get_escrow_contract(vault_id)
    }

    /// Get custody system statistics
    pub fn get_custody_stats(&self) -> custody::CustodyStats {
        self.custody_manager.get_custody_stats()
    }

    /// Monitor Bitcoin network for vault funding transactions
    pub async fn monitor_vault_funding(&self, vault_id: Txid) -> Result<bool> {
        if let Some(bitcoin_client) = &self.bitcoin_client {
            if let Some(contract) = self.custody_manager.get_escrow_contract(vault_id) {
                // Check if the multisig address has received funds
                let utxos = bitcoin_client.get_utxos(&contract.multisig_address)?;
                
                for utxo in utxos {
                    if utxo.amount >= contract.collateral_amount {
                        log::info!(
                            "Vault {} escrow funded with {} BTC in transaction {}:{}",
                            vault_id,
                            utxo.amount.to_btc(),
                            utxo.txid,
                            utxo.vout
                        );
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}