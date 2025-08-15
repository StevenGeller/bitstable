// BitStable - Bitcoin-collateralized multi-currency stablecoin protocol
pub mod vault;
pub mod oracle;
pub mod liquidation;
pub mod stable;
pub mod error;
pub mod config;
pub mod network;
pub mod custody;
pub mod bitcoin_client;
pub mod crypto;
pub mod database;
pub mod multi_currency;
pub mod stability_controller;

use bitcoin::{Amount, PublicKey, Txid};
// Re-export for public use

pub use error::{BitStableError, Result};
pub use vault::{Vault, VaultState, VaultManager};
pub use oracle::{Oracle, MultiCurrencyOracleNetwork, PriceConsensus};
pub use liquidation::{LiquidationEngine, LiquidationOpportunity};
pub use stable::StableTransfer;
pub use config::ProtocolConfig;
pub use custody::{CustodyManager, EscrowContract, LiquidationSettlement};
pub use bitcoin_client::{BitcoinClient, BitcoinConfig};
pub use multi_currency::{Currency, CurrencyConfig, ExchangeRates, MultiCurrencyPosition};
pub use stability_controller::{StabilityController, RebalanceAction};

#[derive(Debug)]
pub struct BitStableProtocol {
    pub config: ProtocolConfig,
    pub vault_manager: VaultManager,
    pub oracle_network: MultiCurrencyOracleNetwork,
    pub liquidation_engine: LiquidationEngine,
    pub custody_manager: CustodyManager,
    pub stability_controller: StabilityController,
    pub bitcoin_client: Option<BitcoinClient>,
}

impl BitStableProtocol {
    pub fn new(config: ProtocolConfig) -> Result<Self> {
        Ok(Self {
            vault_manager: VaultManager::new(&config)?,
            oracle_network: MultiCurrencyOracleNetwork::new(&config)?,
            liquidation_engine: LiquidationEngine::new(&config)?,
            custody_manager: CustodyManager::new(&config)?,
            stability_controller: StabilityController::new(
                bitcoin::PublicKey::from_slice(&[2; 33]).unwrap(),
                Currency::USD,
                0.0
            ),
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
        currency: Currency,
        stable_amount: f64,
    ) -> Result<EscrowContract> {
        let exchange_rates = self.oracle_network.get_exchange_rates();
        
        // Create vault in the vault manager
        let vault_id = self.vault_manager.create_vault(
            owner,
            collateral,
            currency.clone(),
            stable_amount,
        ).await?;
        
        // Calculate liquidation threshold price
        let vault = self.vault_manager.get_vault(vault_id)?;
        let liquidation_price = vault.calculate_liquidation_price(&currency, &exchange_rates, 1.5);
        
        // Create escrow contract for Bitcoin custody
        let escrow_contract = self.custody_manager.create_vault_escrow(
            vault_id,
            owner,
            collateral,
            liquidation_price,
        )?;

        log::info!(
            "Created vault {} with escrow address {} for {} {:?}",
            vault_id,
            escrow_contract.multisig_address,
            stable_amount,
            currency
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
        let exchange_rates = self.oracle_network.get_exchange_rates();
        
        // Get vault information for liquidation calculation
        let vault = self.vault_manager.get_vault(vault_id)?;
        
        // Check if vault can be liquidated based on custody rules
        let btc_price = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0);
        if !self.custody_manager.can_liquidate_vault(vault_id, btc_price) {
            return Err(BitStableError::LiquidationNotPossible {
                ratio: vault.collateral_ratio(&exchange_rates)
            });
        }

        // Execute liquidation in the liquidation engine
        self.liquidation_engine.liquidate(vault_id, liquidator, btc_price).await?;
        
        // Calculate total debt in USD for liquidation settlement
        let total_debt_usd = vault.debts.total_debt_in_usd(&exchange_rates);
        
        // Create and sign liquidation settlement transaction
        let liquidation_tx = self.custody_manager.execute_liquidation(
            vault_id,
            liquidator,
            btc_price,
            total_debt_usd,
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
        let exchange_rates = self.oracle_network.get_exchange_rates();
        
        Ok(vault.collateral_ratio(&exchange_rates))
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

    /// Enable autopilot stability management for a user
    pub async fn set_stability_target(
        &mut self,
        user: PublicKey,
        currency: Currency,
        amount: f64,
    ) -> Result<()> {
        // This would need to be implemented with a collection of controllers
        log::info!("Set stability target for user {}: {} {:?}", user, amount, currency);
        Ok(())
    }

    /// Run stability controller rebalancing
    pub async fn run_stability_rebalancing(&mut self) -> Result<()> {
        let exchange_rates = self.oracle_network.get_exchange_rates();
        // This would need to be implemented with a collection of controllers
        let rebalances: Vec<(PublicKey, Vec<RebalanceAction>)> = Vec::new();
        
        for (user, actions) in rebalances {
            for action in actions {
                log::info!("Rebalancing for {}: {:?}", user, action);
                // Execute rebalancing actions through vault manager
                // This would involve minting/burning stable currencies as needed
            }
        }
        
        Ok(())
    }
}
