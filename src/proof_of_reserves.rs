use bitcoin::{Amount, Txid, ScriptBuf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use crate::{BitStableError, Result, Vault, Currency};

/// Proof-of-reserves system for real-time transparency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfReservesSystem {
    pub current_commitment: Option<ReservesCommitment>,
    pub commitment_history: Vec<ReservesCommitment>,
    pub pending_commitments: Vec<PendingCommitment>,
    pub verification_cache: HashMap<String, MerkleProof>,
    pub last_bitcoin_block: u64,
}

/// Complete system state commitment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservesCommitment {
    pub merkle_root: String,
    pub block_height: u64,
    pub commitment_timestamp: DateTime<Utc>,
    pub system_state: SystemStateSnapshot,
    pub bitcoin_transaction: Option<Txid>,
    pub total_vaults: usize,
    pub total_collateral_btc: Amount,
    pub total_debt_usd: f64,
}

/// Individual vault state for Merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultState {
    pub vault_id: Txid,
    pub owner_hash: String,  // Hash of owner pubkey for privacy
    pub collateral_btc: Amount,
    pub debt_balances: HashMap<Currency, f64>,
    pub collateral_ratio: f64,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// System-wide state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStateSnapshot {
    pub system_collateral_ratio: f64,
    pub total_debt_all_currencies: f64,
    pub total_collateral_btc: Amount,
    pub oracle_health: f64,
    pub insurance_balance: Amount,
    pub active_oracles: usize,
    pub emergency_state: bool,
}

/// Merkle proof for vault inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub vault_state: VaultState,
    pub proof_path: Vec<String>,
    pub merkle_root: String,
    pub block_height: u64,
    pub proof_valid: bool,
}

/// Pending commitment awaiting Bitcoin confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCommitment {
    pub merkle_root: String,
    pub system_state: SystemStateSnapshot,
    pub created_at: DateTime<Utc>,
    pub bitcoin_tx_pending: Option<Txid>,
}

/// Fraud proof for under-collateralized vaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudProof {
    pub vault_state: VaultState,
    pub oracle_prices: HashMap<Currency, f64>,
    pub merkle_proof: MerkleProof,
    pub violation_type: FraudType,
    pub calculated_cr: f64,
    pub minimum_required_cr: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FraudType {
    UnderCollateralized,
    InvalidDebt,
    TimestampFraud,
    InvalidSignature,
}

impl Default for ProofOfReservesSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofOfReservesSystem {
    pub fn new() -> Self {
        Self {
            current_commitment: None,
            commitment_history: Vec::new(),
            pending_commitments: Vec::new(),
            verification_cache: HashMap::new(),
            last_bitcoin_block: 0,
        }
    }

    /// Generate Merkle tree commitment for all vault states
    pub fn generate_commitment(
        &mut self,
        vaults: &[&Vault],
        system_state: SystemStateSnapshot,
        block_height: u64,
    ) -> Result<ReservesCommitment> {
        // Convert vaults to VaultState objects
        let vault_states: Result<Vec<VaultState>> = vaults
            .iter()
            .map(|vault| self.vault_to_state(vault))
            .collect();
        let vault_states = vault_states?;

        // Build Merkle tree
        let merkle_root = self.build_merkle_tree(&vault_states)?;

        // Calculate totals
        let total_collateral_btc = vaults.iter()
            .map(|v| v.collateral_btc)
            .sum::<Amount>();
        
        let total_debt_usd = vaults.iter()
            .map(|v| v.debts.total_debt_in_usd(&crate::multi_currency::ExchangeRates::new()))
            .sum::<f64>();

        let commitment = ReservesCommitment {
            merkle_root: merkle_root.clone(),
            block_height,
            commitment_timestamp: Utc::now(),
            system_state,
            bitcoin_transaction: None, // Will be set when broadcast to Bitcoin
            total_vaults: vaults.len(),
            total_collateral_btc,
            total_debt_usd,
        };

        // Store current commitment
        self.current_commitment = Some(commitment.clone());
        self.commitment_history.push(commitment.clone());

        // Keep only last 10,000 commitments for efficiency
        if self.commitment_history.len() > 10_000 {
            self.commitment_history.remove(0);
        }

        log::info!(
            "Generated proof-of-reserves commitment for {} vaults at block {}",
            vaults.len(),
            block_height
        );

        Ok(commitment)
    }

    /// Convert vault to anonymized state for Merkle tree
    fn vault_to_state(&self, vault: &Vault) -> Result<VaultState> {
        // Hash owner pubkey for privacy while maintaining verifiability
        let owner_hash = self.hash_pubkey(vault.owner);
        
        // Create signature of vault state (simplified - would use real cryptographic signature)
        let signature = self.sign_vault_state(vault)?;

        // Calculate exchange rates for CR (simplified)
        let exchange_rates = crate::multi_currency::ExchangeRates::new();
        let collateral_ratio = vault.collateral_ratio(&exchange_rates);

        Ok(VaultState {
            vault_id: vault.id,
            owner_hash,
            collateral_btc: vault.collateral_btc,
            debt_balances: vault.debts.debts.clone(),
            collateral_ratio,
            timestamp: Utc::now(),
            signature,
        })
    }

    /// Build Merkle tree from vault states
    fn build_merkle_tree(&self, vault_states: &[VaultState]) -> Result<String> {
        if vault_states.is_empty() {
            return Ok("0".repeat(64)); // Empty tree root
        }

        // Hash each vault state
        let mut hashes: Vec<String> = vault_states
            .iter()
            .map(|state| self.hash_vault_state(state))
            .collect();

        // Build tree bottom-up
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..hashes.len()).step_by(2) {
                let left = &hashes[i];
                let right = if i + 1 < hashes.len() {
                    &hashes[i + 1]
                } else {
                    left // Duplicate if odd number
                };
                
                let combined = format!("{}{}", left, right);
                let hash = self.hash_string(&combined);
                next_level.push(hash);
            }
            
            hashes = next_level;
        }

        Ok(hashes[0].clone())
    }

    /// Generate Merkle proof for specific vault
    pub fn generate_merkle_proof(
        &mut self,
        vault_id: Txid,
        vault_states: &[VaultState],
    ) -> Result<MerkleProof> {
        // Find vault in states
        let vault_index = vault_states
            .iter()
            .position(|state| state.vault_id == vault_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Vault not found".to_string()))?;

        let vault_state = vault_states[vault_index].clone();

        // Build proof path
        let proof_path = self.build_proof_path(vault_index, vault_states)?;
        let merkle_root = self.build_merkle_tree(vault_states)?;

        let proof = MerkleProof {
            vault_state,
            proof_path,
            merkle_root: merkle_root.clone(),
            block_height: self.current_commitment
                .as_ref()
                .map(|c| c.block_height)
                .unwrap_or(0),
            proof_valid: true,
        };

        // Cache proof for future verification
        self.verification_cache.insert(vault_id.to_string(), proof.clone());

        Ok(proof)
    }

    /// Build Merkle proof path for vault at given index
    fn build_proof_path(&self, vault_index: usize, vault_states: &[VaultState]) -> Result<Vec<String>> {
        let mut proof_path = Vec::new();
        let mut hashes: Vec<String> = vault_states
            .iter()
            .map(|state| self.hash_vault_state(state))
            .collect();
        
        let mut current_index = vault_index;

        while hashes.len() > 1 {
            // Find sibling
            let sibling_index = if current_index % 2 == 0 {
                if current_index + 1 < hashes.len() {
                    current_index + 1
                } else {
                    current_index // Self if odd number
                }
            } else {
                current_index - 1
            };

            proof_path.push(hashes[sibling_index].clone());

            // Move to next level
            let mut next_level = Vec::new();
            for i in (0..hashes.len()).step_by(2) {
                let left = &hashes[i];
                let right = if i + 1 < hashes.len() {
                    &hashes[i + 1]
                } else {
                    left
                };
                let combined = format!("{}{}", left, right);
                next_level.push(self.hash_string(&combined));
            }

            current_index /= 2;
            hashes = next_level;
        }

        Ok(proof_path)
    }

    /// Verify Merkle proof for vault inclusion
    pub fn verify_merkle_proof(&self, proof: &MerkleProof) -> bool {
        let mut current_hash = self.hash_vault_state(&proof.vault_state);
        
        for sibling_hash in &proof.proof_path {
            let combined = format!("{}{}", current_hash, sibling_hash);
            current_hash = self.hash_string(&combined);
        }

        current_hash == proof.merkle_root
    }

    /// Submit commitment to Bitcoin blockchain via OP_RETURN
    pub fn submit_to_bitcoin(
        &mut self,
        commitment: &ReservesCommitment,
        bitcoin_client: &crate::bitcoin_client::BitcoinClient,
    ) -> Result<Txid> {
        // Create OP_RETURN script with commitment data
        let _commitment_data = format!(
            "{}:{}:{}",
            commitment.merkle_root,
            commitment.block_height,
            commitment.system_state.system_collateral_ratio
        );

        // Simplified OP_RETURN for now - in production would use proper script building
        let op_return_script = ScriptBuf::new();
        
        // Create and broadcast Bitcoin transaction
        let txid = bitcoin_client.create_op_return_transaction(op_return_script)?;
        
        // Update commitment with Bitcoin transaction ID
        if let Some(current) = &mut self.current_commitment {
            current.bitcoin_transaction = Some(txid);
        }

        log::info!(
            "Submitted proof-of-reserves commitment to Bitcoin: {}",
            txid
        );

        Ok(txid)
    }

    /// Validate fraud proof
    pub fn validate_fraud_proof(&self, fraud_proof: &FraudProof) -> Result<bool> {
        // Verify Merkle proof first
        if !self.verify_merkle_proof(&fraud_proof.merkle_proof) {
            return Ok(false);
        }

        // Verify collateral ratio calculation
        let vault_state = &fraud_proof.vault_state;
        let mut total_debt_usd = 0.0;

        for (currency, debt) in &vault_state.debt_balances {
            if let Some(price) = fraud_proof.oracle_prices.get(currency) {
                total_debt_usd += debt * price;
            }
        }

        // Get BTC price from oracle prices
        let btc_price = fraud_proof.oracle_prices.get(&Currency::USD)
            .ok_or_else(|| BitStableError::PriceFeedError("BTC/USD price not provided".to_string()))?;

        let collateral_value_usd = vault_state.collateral_btc.to_btc() * btc_price;
        let calculated_cr = if total_debt_usd > 0.0 {
            collateral_value_usd / total_debt_usd
        } else {
            f64::INFINITY
        };

        // Check if vault is actually under-collateralized
        let is_under_collateralized = calculated_cr < fraud_proof.minimum_required_cr;

        log::info!(
            "Fraud proof validation: CR={:.4}, Required={:.4}, Valid={}",
            calculated_cr,
            fraud_proof.minimum_required_cr,
            is_under_collateralized
        );

        Ok(is_under_collateralized)
    }

    /// Get proof-of-reserves statistics
    pub fn get_statistics(&self) -> ProofOfReservesStats {
        let total_commitments = self.commitment_history.len();
        let last_commitment = self.current_commitment.as_ref();
        
        let avg_vault_count = if total_commitments > 0 {
            self.commitment_history.iter()
                .map(|c| c.total_vaults)
                .sum::<usize>() as f64 / total_commitments as f64
        } else {
            0.0
        };

        ProofOfReservesStats {
            total_commitments,
            last_commitment_block: last_commitment.map(|c| c.block_height).unwrap_or(0),
            last_commitment_time: last_commitment.map(|c| c.commitment_timestamp),
            average_vault_count: avg_vault_count,
            current_system_cr: last_commitment
                .map(|c| c.system_state.system_collateral_ratio)
                .unwrap_or(0.0),
            total_collateral_btc: last_commitment
                .map(|c| c.total_collateral_btc)
                .unwrap_or(Amount::ZERO),
            pending_commitments: self.pending_commitments.len(),
            cache_size: self.verification_cache.len(),
        }
    }

    // Helper functions
    fn hash_pubkey(&self, pubkey: bitcoin::PublicKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(pubkey.inner.serialize());
        hex::encode(hasher.finalize())
    }

    fn hash_vault_state(&self, state: &VaultState) -> String {
        let data = format!(
            "{}:{}:{}:{}:{}",
            state.vault_id,
            state.owner_hash,
            state.collateral_btc.to_sat(),
            serde_json::to_string(&state.debt_balances).unwrap_or_default(),
            state.timestamp.timestamp()
        );
        self.hash_string(&data)
    }

    fn hash_string(&self, data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn sign_vault_state(&self, _vault: &Vault) -> Result<String> {
        // Simplified signature - in production would use real cryptographic signature
        // This would involve the vault owner's private key
        Ok("mock_signature".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfReservesStats {
    pub total_commitments: usize,
    pub last_commitment_block: u64,
    pub last_commitment_time: Option<DateTime<Utc>>,
    pub average_vault_count: f64,
    pub current_system_cr: f64,
    pub total_collateral_btc: Amount,
    pub pending_commitments: usize,
    pub cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_merkle_tree_generation() {
        let mut por_system = ProofOfReservesSystem::new();
        
        // Create mock vault states
        let vault_states = vec![
            VaultState {
                vault_id: Txid::from_str("1111111111111111111111111111111111111111111111111111111111111111").unwrap(),
                owner_hash: "hash1".to_string(),
                collateral_btc: Amount::from_btc(1.0).unwrap(),
                debt_balances: HashMap::new(),
                collateral_ratio: 2.0,
                timestamp: Utc::now(),
                signature: "sig1".to_string(),
            },
            VaultState {
                vault_id: Txid::from_str("2222222222222222222222222222222222222222222222222222222222222222").unwrap(),
                owner_hash: "hash2".to_string(),
                collateral_btc: Amount::from_btc(2.0).unwrap(),
                debt_balances: HashMap::new(),
                collateral_ratio: 1.8,
                timestamp: Utc::now(),
                signature: "sig2".to_string(),
            },
        ];

        let merkle_root = por_system.build_merkle_tree(&vault_states).unwrap();
        assert_eq!(merkle_root.len(), 64); // SHA256 hash length in hex

        // Test Merkle proof generation and verification
        let proof = por_system.generate_merkle_proof(vault_states[0].vault_id, &vault_states).unwrap();
        assert!(por_system.verify_merkle_proof(&proof));
    }

    #[test]
    fn test_fraud_proof_validation() {
        let mut por_system = ProofOfReservesSystem::new();
        
        // Create fraud proof for under-collateralized vault
        let vault_state = VaultState {
            vault_id: Txid::from_str("1111111111111111111111111111111111111111111111111111111111111111").unwrap(),
            owner_hash: "hash1".to_string(),
            collateral_btc: Amount::from_btc(1.0).unwrap(), // 1 BTC
            debt_balances: {
                let mut debts = HashMap::new();
                debts.insert(Currency::USD, 60000.0); // $60k debt
                debts
            },
            collateral_ratio: 1.0, // Under-collateralized at 100%
            timestamp: Utc::now(),
            signature: "sig1".to_string(),
        };

        let oracle_prices = {
            let mut prices = HashMap::new();
            prices.insert(Currency::USD, 50000.0); // BTC = $50k
            prices
        };

        // Create a proper Merkle proof using the actual system
        let vault_states = vec![vault_state.clone()];
        let merkle_proof = por_system.generate_merkle_proof(vault_state.vault_id, &vault_states).unwrap();

        let fraud_proof = FraudProof {
            vault_state,
            oracle_prices,
            merkle_proof,
            violation_type: FraudType::UnderCollateralized,
            calculated_cr: 0.833, // $50k collateral / $60k debt
            minimum_required_cr: 1.25,
            timestamp: Utc::now(),
        };

        // This should detect the under-collateralization
        assert!(por_system.validate_fraud_proof(&fraud_proof).unwrap_or(false));
    }
}