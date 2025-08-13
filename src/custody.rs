use bitcoin::{
    Address, Amount, Network, PrivateKey, PublicKey, ScriptBuf, Transaction, TxIn, TxOut, Txid,
    OutPoint, Witness, absolute::LockTime, transaction::Version, Sequence
};
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::script::Builder;
use bitcoin::opcodes::all::{OP_CHECKMULTISIG, OP_PUSHNUM_2, OP_PUSHNUM_3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig};

/// Bitcoin custody manager that handles trustless collateral locking and liquidation settlements
#[derive(Debug)]
pub struct CustodyManager {
    config: ProtocolConfig,
    network: Network,
    
    // Protocol keys for multisig
    protocol_keys: Vec<PublicKey>,
    protocol_privkey: Option<PrivateKey>,
    
    // Active escrow contracts
    escrow_contracts: HashMap<Txid, EscrowContract>,
    
    // Pending transactions
    pending_txs: HashMap<Txid, PendingTransaction>,
    
    // Liquidation settlements
    settlements: HashMap<Txid, LiquidationSettlement>,
}

/// Represents a multisig escrow contract for vault collateral
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowContract {
    pub vault_id: Txid,
    pub owner_pubkey: PublicKey,
    pub collateral_amount: Amount,
    #[serde(with = "address_serde")]
    pub multisig_address: Address,
    pub redeem_script: ScriptBuf,
    pub funding_txid: Txid,
    pub funding_vout: u32,
    pub created_at: DateTime<Utc>,
    pub liquidation_threshold_price: f64,
    pub required_sigs: u8,
    pub protocol_pubkeys: Vec<PublicKey>,
}

/// Custom serde module for Address serialization
mod address_serde {
    use serde::{Deserialize, Deserializer, Serializer, Serialize};
    use bitcoin::Address;

    pub fn serialize<S>(address: &Address, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        address.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Address<_>>()
            .map(|addr| addr.assume_checked())
            .map_err(serde::de::Error::custom)
    }
}

/// Transaction pending broadcast or confirmation
#[derive(Debug, Clone)]
pub struct PendingTransaction {
    pub tx: Transaction,
    pub vault_id: Txid,
    pub tx_type: TransactionType,
    pub created_at: DateTime<Utc>,
    pub broadcast: bool,
}

/// Types of Bitcoin transactions in the custody system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    VaultFunding,
    Liquidation,
    VaultClosure,
    EmergencySettlement,
}

/// Liquidation settlement tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationSettlement {
    pub vault_id: Txid,
    pub liquidator: PublicKey,
    pub settlement_txid: Txid,
    pub collateral_seized: Amount,
    pub liquidator_bonus: Amount,
    pub protocol_fee: Amount,
    pub settled_at: DateTime<Utc>,
}

impl CustodyManager {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        let network = config.network;
        
        // Generate protocol keys for multisig (in production, these would be pre-generated and distributed)
        let secp = Secp256k1::new();
        let protocol_keys = Self::generate_protocol_keys(&secp)?;
        
        Ok(Self {
            config: config.clone(),
            network,
            protocol_keys,
            protocol_privkey: None,
            escrow_contracts: HashMap::new(),
            pending_txs: HashMap::new(),
            settlements: HashMap::new(),
        })
    }

    /// Initialize with a protocol private key for signing
    pub fn with_protocol_key(mut self, privkey: PrivateKey) -> Self {
        self.protocol_privkey = Some(privkey);
        self
    }

    /// Generate the protocol's multisig keys (3-of-5 setup)
    fn generate_protocol_keys(secp: &Secp256k1<bitcoin::secp256k1::All>) -> Result<Vec<PublicKey>> {
        let mut keys = Vec::new();
        
        // In production, these would be well-known public keys from protocol governance
        for _ in 0..5 {
            let secret_key = SecretKey::new(&mut rand::thread_rng());
            let public_key = PublicKey::from_private_key(secp, &PrivateKey::new(secret_key, Network::Testnet));
            keys.push(public_key);
        }
        
        Ok(keys)
    }

    /// Create a new vault escrow contract
    pub fn create_vault_escrow(
        &mut self,
        vault_id: Txid,
        owner_pubkey: PublicKey,
        collateral_amount: Amount,
        liquidation_price: f64,
    ) -> Result<EscrowContract> {
        // Create 2-of-3 multisig: vault owner + 2 protocol keys
        let mut script_pubkeys = vec![owner_pubkey];
        script_pubkeys.extend_from_slice(&self.protocol_keys[0..2]);
        
        let redeem_script = self.create_multisig_script(&script_pubkeys, 2)?;
        let multisig_address = Address::p2wsh(&redeem_script, self.network);

        let contract = EscrowContract {
            vault_id,
            owner_pubkey,
            collateral_amount,
            multisig_address: multisig_address.clone(),
            redeem_script,
            funding_txid: Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros()), // Will be set when funded
            funding_vout: 0,
            created_at: Utc::now(),
            liquidation_threshold_price: liquidation_price,
            required_sigs: 2,
            protocol_pubkeys: self.protocol_keys[0..2].to_vec(),
        };

        self.escrow_contracts.insert(vault_id, contract.clone());
        
        log::info!(
            "Created escrow contract for vault {} with address {}",
            vault_id,
            multisig_address
        );

        Ok(contract)
    }

    /// Create a multisig redeem script
    fn create_multisig_script(&self, pubkeys: &[PublicKey], required_sigs: u8) -> Result<ScriptBuf> {
        if pubkeys.len() > 15 || required_sigs == 0 || required_sigs as usize > pubkeys.len() {
            return Err(BitStableError::InvalidConfig("Invalid multisig parameters".to_string()));
        }

        let mut builder = Builder::new();
        
        // Push required signatures count
        builder = match required_sigs {
            1 => builder.push_opcode(bitcoin::opcodes::all::OP_PUSHNUM_1),
            2 => builder.push_opcode(OP_PUSHNUM_2),
            3 => builder.push_opcode(OP_PUSHNUM_3),
            n => builder.push_int(n as i64),
        };

        // Push all public keys
        for pubkey in pubkeys {
            builder = builder.push_key(pubkey);
        }

        // Push total key count and OP_CHECKMULTISIG
        builder = match pubkeys.len() {
            2 => builder.push_opcode(OP_PUSHNUM_2),
            3 => builder.push_opcode(OP_PUSHNUM_3),
            n => builder.push_int(n as i64),
        };
        
        builder = builder.push_opcode(OP_CHECKMULTISIG);

        Ok(builder.into_script())
    }

    /// Process vault funding transaction
    pub fn process_vault_funding(
        &mut self,
        vault_id: Txid,
        funding_txid: Txid,
        vout: u32,
        amount: Amount,
    ) -> Result<()> {
        let contract = self.escrow_contracts.get_mut(&vault_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Escrow contract not found".to_string()))?;

        if amount < contract.collateral_amount {
            return Err(BitStableError::InsufficientCollateral {
                required: contract.collateral_amount.to_btc() as f64,
                provided: amount.to_btc() as f64,
            });
        }

        contract.funding_txid = funding_txid;
        contract.funding_vout = vout;

        log::info!(
            "Vault {} funded with {} BTC in transaction {}:{}",
            vault_id,
            amount.to_btc(),
            funding_txid,
            vout
        );

        Ok(())
    }

    /// Execute liquidation settlement
    pub fn execute_liquidation(
        &mut self,
        vault_id: Txid,
        liquidator: PublicKey,
        btc_price: f64,
        debt_amount: f64,
    ) -> Result<Transaction> {
        let contract = self.escrow_contracts.get(&vault_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Escrow contract not found".to_string()))?;

        // Calculate liquidation amounts
        let debt_in_btc = debt_amount / btc_price;
        let liquidation_bonus = debt_in_btc * self.config.liquidation_penalty;
        let total_seized = Amount::from_btc(debt_in_btc + liquidation_bonus)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid amount: {}", e)))?;
        
        // Protocol fee (1% of liquidated amount)
        let protocol_fee = Amount::from_btc(debt_in_btc * 0.01)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid amount: {}", e)))?;

        // Create liquidation transaction
        let liquidation_tx = self.create_liquidation_transaction(
            contract,
            liquidator,
            total_seized,
            protocol_fee,
        )?;

        // Record settlement
        let settlement = LiquidationSettlement {
            vault_id,
            liquidator,
            settlement_txid: liquidation_tx.compute_txid(),
            collateral_seized: total_seized,
            liquidator_bonus: Amount::from_btc(liquidation_bonus)
                .map_err(|e| BitStableError::InvalidConfig(format!("Invalid amount: {}", e)))?,
            protocol_fee,
            settled_at: Utc::now(),
        };

        self.settlements.insert(vault_id, settlement);

        // Add to pending transactions
        let pending = PendingTransaction {
            tx: liquidation_tx.clone(),
            vault_id,
            tx_type: TransactionType::Liquidation,
            created_at: Utc::now(),
            broadcast: false,
        };

        self.pending_txs.insert(liquidation_tx.compute_txid(), pending);

        log::info!(
            "Created liquidation transaction for vault {} seizing {} BTC",
            vault_id,
            total_seized.to_btc()
        );

        Ok(liquidation_tx)
    }

    /// Create a liquidation transaction that spends from the multisig escrow
    fn create_liquidation_transaction(
        &self,
        contract: &EscrowContract,
        liquidator: PublicKey,
        amount_to_liquidator: Amount,
        protocol_fee: Amount,
    ) -> Result<Transaction> {
        // Create transaction input from the escrow
        let input = TxIn {
            previous_output: OutPoint {
                txid: contract.funding_txid,
                vout: contract.funding_vout,
            },
            script_sig: bitcoin::ScriptBuf::new(), // Will be filled with signatures
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(), // For witness transactions
        };

        // Create outputs
        let mut outputs = Vec::new();

        // Create P2PK script for liquidator (simpler than P2WPKH)
        let liquidator_script = bitcoin::ScriptBuf::new_p2pk(&liquidator);
        
        outputs.push(TxOut {
            value: amount_to_liquidator,
            script_pubkey: liquidator_script,
        });

        // Output for protocol fee (if any)
        if protocol_fee > Amount::ZERO {
            // Protocol treasury script
            let protocol_script = bitcoin::ScriptBuf::new_p2pk(&self.protocol_keys[0]);
            
            outputs.push(TxOut {
                value: protocol_fee,
                script_pubkey: protocol_script,
            });
        }

        // Return remaining collateral to vault owner (if any)
        let total_used = amount_to_liquidator + protocol_fee;
        if contract.collateral_amount > total_used {
            let remaining = contract.collateral_amount - total_used;
            let owner_script = bitcoin::ScriptBuf::new_p2pk(&contract.owner_pubkey);
            
            outputs.push(TxOut {
                value: remaining,
                script_pubkey: owner_script,
            });
        }

        Ok(Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![input],
            output: outputs,
        })
    }

    /// Create vault closure transaction (when debt is repaid)
    pub fn create_vault_closure_transaction(
        &self,
        vault_id: Txid,
    ) -> Result<Transaction> {
        let contract = self.escrow_contracts.get(&vault_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Escrow contract not found".to_string()))?;

        // Create transaction that returns all collateral to owner
        let input = TxIn {
            previous_output: OutPoint {
                txid: contract.funding_txid,
                vout: contract.funding_vout,
            },
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let owner_script = bitcoin::ScriptBuf::new_p2pk(&contract.owner_pubkey);

        // Small fee deduction (0.0001 BTC)
        let fee = Amount::from_sat(10000);
        let return_amount = contract.collateral_amount - fee;

        let output = TxOut {
            value: return_amount,
            script_pubkey: owner_script,
        };

        Ok(Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![input],
            output: vec![output],
        })
    }

    /// Sign a transaction using protocol private key
    pub fn sign_transaction(&self, tx: &mut Transaction, input_index: usize, vault_id: Txid) -> Result<()> {
        use bitcoin::sighash::{Prevouts, SighashCache};
        use bitcoin::ecdsa::Signature;
        use bitcoin::secp256k1::{Message, Secp256k1};

        let privkey = self.protocol_privkey
            .ok_or_else(|| BitStableError::InvalidConfig("Protocol private key not available".to_string()))?;

        let contract = self.escrow_contracts.get(&vault_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Escrow contract not found".to_string()))?;

        if input_index >= tx.input.len() {
            return Err(BitStableError::InvalidConfig("Input index out of bounds".to_string()));
        }

        // For multisig P2WSH, we need to create the signature hash
        let secp = Secp256k1::new();
        
        // Create prevouts for sighash calculation
        let prevouts = vec![TxOut {
            value: contract.collateral_amount,
            script_pubkey: contract.multisig_address.script_pubkey(),
        }];
        let _prevouts = Prevouts::All(&prevouts);

        // Create sighash cache and calculate the signature hash
        let mut sighash_cache = SighashCache::new(&*tx);
        let sighash = sighash_cache
            .p2wsh_signature_hash(
                input_index,
                &contract.redeem_script,
                contract.collateral_amount,
                bitcoin::EcdsaSighashType::All,
            )
            .map_err(|e| BitStableError::InvalidConfig(format!("Sighash calculation failed: {}", e)))?;

        // Sign the sighash  
        let message = Message::from_digest_slice(&sighash[..])
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid message: {}", e)))?;
        
        let signature = secp.sign_ecdsa(&message, &privkey.inner);
        let bitcoin_signature = Signature {
            signature,
            sighash_type: bitcoin::EcdsaSighashType::All,
        };

        // For P2WSH multisig, the witness stack should be:
        // [0] [sig1] [sig2] [redeem_script]
        let witness_stack = vec![
            vec![], // OP_0 for multisig bug
            bitcoin_signature.to_vec(), // First signature (protocol key)
            vec![], // Placeholder for second signature (owner)
            contract.redeem_script.to_bytes(), // Redeem script
        ];

        // Update the witness for this input
        tx.input[input_index].witness = bitcoin::Witness::from_slice(&witness_stack);

        log::info!(
            "Signed transaction {} for vault {} with protocol key",
            tx.compute_txid(),
            vault_id
        );
        
        Ok(())
    }

    /// Get escrow contract for a vault
    pub fn get_escrow_contract(&self, vault_id: Txid) -> Option<&EscrowContract> {
        self.escrow_contracts.get(&vault_id)
    }

    /// Get liquidation settlement info
    pub fn get_settlement(&self, vault_id: Txid) -> Option<&LiquidationSettlement> {
        self.settlements.get(&vault_id)
    }

    /// Get all pending transactions
    pub fn get_pending_transactions(&self) -> Vec<&PendingTransaction> {
        self.pending_txs.values().collect()
    }

    /// Mark transaction as broadcast
    pub fn mark_transaction_broadcast(&mut self, txid: Txid) -> Result<()> {
        if let Some(pending) = self.pending_txs.get_mut(&txid) {
            pending.broadcast = true;
            log::info!("Marked transaction {} as broadcast", txid);
        }
        Ok(())
    }

    /// Check if vault can be liquidated based on escrow state
    pub fn can_liquidate_vault(&self, vault_id: Txid, current_btc_price: f64) -> bool {
        if let Some(contract) = self.escrow_contracts.get(&vault_id) {
            current_btc_price <= contract.liquidation_threshold_price
        } else {
            false
        }
    }

    /// Calculate total protocol collateral under management
    pub fn total_collateral_managed(&self) -> Amount {
        self.escrow_contracts.values()
            .map(|contract| contract.collateral_amount)
            .sum()
    }

    /// Get custody statistics
    pub fn get_custody_stats(&self) -> CustodyStats {
        let active_vaults = self.escrow_contracts.len();
        let total_collateral = self.total_collateral_managed();
        let pending_liquidations = self.pending_txs.values()
            .filter(|tx| matches!(tx.tx_type, TransactionType::Liquidation))
            .count();
        let total_settlements = self.settlements.len();

        CustodyStats {
            active_escrow_contracts: active_vaults,
            total_collateral_btc: total_collateral,
            pending_liquidations,
            completed_settlements: total_settlements,
            protocol_fees_collected: self.settlements.values()
                .map(|s| s.protocol_fee)
                .sum(),
        }
    }
}

/// Custody system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodyStats {
    pub active_escrow_contracts: usize,
    pub total_collateral_btc: Amount,
    pub pending_liquidations: usize,
    pub completed_settlements: usize,
    pub protocol_fees_collected: Amount,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;

    #[test]
    fn test_multisig_script_creation() {
        let config = ProtocolConfig::testnet();
        let custody = CustodyManager::new(&config).unwrap();
        
        let secp = Secp256k1::new();
        let keys: Vec<PublicKey> = (0..3).map(|_| {
            let secret_key = SecretKey::new(&mut rand::thread_rng());
            PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet))
        }).collect();

        let script = custody.create_multisig_script(&keys, 2).unwrap();
        assert!(!script.is_empty());
    }

    #[test]
    fn test_escrow_contract_creation() {
        let config = ProtocolConfig::testnet();
        let mut custody = CustodyManager::new(&config).unwrap();
        
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let owner_key = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        let contract = custody.create_vault_escrow(
            vault_id,
            owner_key,
            Amount::from_btc(1.0).unwrap(),
            100000.0,
        ).unwrap();

        assert_eq!(contract.vault_id, vault_id);
        assert_eq!(contract.owner_pubkey, owner_key);
        assert_eq!(contract.required_sigs, 2);
    }
}