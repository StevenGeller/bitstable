/// Cryptographic utilities for BitStable protocol
/// Provides secure key management and threshold signatures

use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey, Message};
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::PrivateKey;
use bitcoin::Network;
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};
use crate::{BitStableError, Result};
use std::collections::HashMap;

/// Oracle key manager for secure key storage and signing
#[derive(Debug)]
pub struct OracleKeyManager {
    oracle_keys: HashMap<String, OracleKeyPair>,
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

/// Oracle key pair with metadata
#[derive(Debug, Clone)]
pub struct OracleKeyPair {
    pub name: String,
    pub public_key: PublicKey,
    private_key: SecretKey,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl OracleKeyManager {
    /// Create a new oracle key manager
    pub fn new() -> Self {
        Self {
            oracle_keys: HashMap::new(),
            secp: Secp256k1::new(),
        }
    }

    /// Generate a new oracle key pair
    pub fn generate_oracle_key(&mut self, oracle_name: &str) -> Result<PublicKey> {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let public_key = PublicKey::from_secret_key(&self.secp, &secret_key);
        
        let key_pair = OracleKeyPair {
            name: oracle_name.to_string(),
            public_key,
            private_key: secret_key,
            created_at: chrono::Utc::now(),
        };
        
        self.oracle_keys.insert(oracle_name.to_string(), key_pair);
        
        log::info!("Generated new oracle key for {}", oracle_name);
        Ok(public_key)
    }

    /// Import an existing oracle private key
    pub fn import_oracle_key(&mut self, oracle_name: &str, private_key_hex: &str) -> Result<PublicKey> {
        let key_bytes = hex::decode(private_key_hex)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid hex key: {}", e)))?;
        
        let secret_key = SecretKey::from_slice(&key_bytes)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid private key: {}", e)))?;
        
        let public_key = PublicKey::from_secret_key(&self.secp, &secret_key);
        
        let key_pair = OracleKeyPair {
            name: oracle_name.to_string(),
            public_key,
            private_key: secret_key,
            created_at: chrono::Utc::now(),
        };
        
        self.oracle_keys.insert(oracle_name.to_string(), key_pair);
        
        log::info!("Imported oracle key for {}", oracle_name);
        Ok(public_key)
    }

    /// Sign price data with an oracle's private key
    pub fn sign_price_data(&self, oracle_name: &str, price: f64, timestamp: i64) -> Result<OracleSignature> {
        let key_pair = self.oracle_keys.get(oracle_name)
            .ok_or_else(|| BitStableError::InvalidConfig(format!("Oracle key not found: {}", oracle_name)))?;
        
        // Create deterministic message from price and timestamp
        let message_data = format!("{}:{}:{}", oracle_name, price, timestamp);
        
        // Hash the message
        let mut hasher = Sha256::new();
        hasher.update(message_data.as_bytes());
        let hash = hasher.finalize();
        
        // Create secp256k1 message
        let message = Message::from_digest_slice(&hash)
            .map_err(|e| BitStableError::OracleSignatureVerificationFailed)?;
        
        // Sign the message
        let signature = self.secp.sign_ecdsa(&message, &key_pair.private_key);
        
        Ok(OracleSignature {
            oracle_name: oracle_name.to_string(),
            price,
            timestamp,
            signature: hex::encode(signature.serialize_compact()),
            public_key: hex::encode(key_pair.public_key.serialize()),
        })
    }

    /// Verify an oracle signature
    pub fn verify_oracle_signature(&self, signature: &OracleSignature) -> Result<bool> {
        // Recreate the message
        let message_data = format!("{}:{}:{}", signature.oracle_name, signature.price, signature.timestamp);
        
        let mut hasher = Sha256::new();
        hasher.update(message_data.as_bytes());
        let hash = hasher.finalize();
        
        let message = Message::from_digest_slice(&hash)
            .map_err(|e| BitStableError::OracleSignatureVerificationFailed)?;
        
        // Parse signature and public key
        let sig_bytes = hex::decode(&signature.signature)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid signature hex: {}", e)))?;
        
        let sig = Signature::from_compact(&sig_bytes)
            .map_err(|e| BitStableError::OracleSignatureVerificationFailed)?;
        
        let pubkey_bytes = hex::decode(&signature.public_key)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid public key hex: {}", e)))?;
        
        let pubkey = PublicKey::from_slice(&pubkey_bytes)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid public key: {}", e)))?;
        
        // Verify the signature
        Ok(self.secp.verify_ecdsa(&message, &sig, &pubkey).is_ok())
    }

    /// Get public key for an oracle
    pub fn get_oracle_public_key(&self, oracle_name: &str) -> Option<PublicKey> {
        self.oracle_keys.get(oracle_name).map(|kp| kp.public_key)
    }

    /// List all oracle names
    pub fn list_oracles(&self) -> Vec<String> {
        self.oracle_keys.keys().cloned().collect()
    }
}

/// Oracle signature with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSignature {
    pub oracle_name: String,
    pub price: f64,
    pub timestamp: i64,
    pub signature: String,
    pub public_key: String,
}

/// Threshold signature scheme for oracle consensus
/// This is a simplified implementation - production would use FROST or similar
pub struct ThresholdSignatureScheme {
    threshold: usize,
    total_oracles: usize,
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl ThresholdSignatureScheme {
    /// Create a new threshold signature scheme
    pub fn new(threshold: usize, total_oracles: usize) -> Result<Self> {
        if threshold == 0 || threshold > total_oracles {
            return Err(BitStableError::InvalidConfig(
                format!("Invalid threshold: {} of {}", threshold, total_oracles)
            ));
        }
        
        Ok(Self {
            threshold,
            total_oracles,
            secp: Secp256k1::new(),
        })
    }

    /// Aggregate oracle signatures into a threshold signature
    pub fn aggregate_signatures(&self, signatures: Vec<OracleSignature>) -> Result<AggregatedSignature> {
        if signatures.len() < self.threshold {
            return Err(BitStableError::InsufficientOracleConsensus {
                got: signatures.len(),
                required: self.threshold,
            });
        }
        
        // Calculate consensus price (median)
        let mut prices: Vec<f64> = signatures.iter().map(|s| s.price).collect();
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let consensus_price = if prices.len() % 2 == 0 {
            (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / 2.0
        } else {
            prices[prices.len() / 2]
        };
        
        // Find the most recent timestamp
        let consensus_timestamp = signatures.iter()
            .map(|s| s.timestamp)
            .max()
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        
        // Collect participating oracles
        let participating_oracles: Vec<String> = signatures.iter()
            .map(|s| s.oracle_name.clone())
            .collect();
        
        // Create aggregated signature hash (simplified - real implementation would use Schnorr aggregation)
        let mut hasher = Sha256::new();
        for sig in &signatures {
            hasher.update(sig.signature.as_bytes());
        }
        let aggregated_hash = hasher.finalize();
        
        Ok(AggregatedSignature {
            consensus_price,
            consensus_timestamp,
            participating_oracles,
            threshold: self.threshold,
            total_oracles: self.total_oracles,
            aggregated_signature: hex::encode(aggregated_hash),
            individual_signatures: signatures,
        })
    }

    /// Verify an aggregated signature
    pub fn verify_aggregated_signature(&self, agg_sig: &AggregatedSignature) -> Result<bool> {
        // Check threshold requirement
        if agg_sig.participating_oracles.len() < self.threshold {
            return Ok(false);
        }
        
        // Verify each individual signature
        let key_manager = OracleKeyManager::new();
        for sig in &agg_sig.individual_signatures {
            if !key_manager.verify_oracle_signature(sig)? {
                return Ok(false);
            }
        }
        
        // Check price consensus (all prices should be within 5% of consensus)
        for sig in &agg_sig.individual_signatures {
            let price_diff = (sig.price - agg_sig.consensus_price).abs() / agg_sig.consensus_price;
            if price_diff > 0.05 {
                log::warn!("Oracle {} price deviation: {:.2}%", sig.oracle_name, price_diff * 100.0);
                return Ok(false);
            }
        }
        
        Ok(true)
    }
}

/// Aggregated signature from multiple oracles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSignature {
    pub consensus_price: f64,
    pub consensus_timestamp: i64,
    pub participating_oracles: Vec<String>,
    pub threshold: usize,
    pub total_oracles: usize,
    pub aggregated_signature: String,
    pub individual_signatures: Vec<OracleSignature>,
}

/// Bitcoin script utilities for multisig operations
pub mod script_utils {
    use bitcoin::{PublicKey, ScriptBuf};
    use bitcoin::script::Builder;
    use bitcoin::opcodes::all::{OP_CHECKMULTISIG, OP_PUSHNUM_1, OP_PUSHNUM_2, OP_PUSHNUM_3};
    use crate::Result;
    
    /// Create a P2WSH multisig script
    pub fn create_multisig_script(pubkeys: &[PublicKey], required_sigs: u8) -> Result<ScriptBuf> {
        if pubkeys.len() > 15 || required_sigs == 0 || required_sigs as usize > pubkeys.len() {
            return Err(crate::BitStableError::InvalidConfig("Invalid multisig parameters".to_string()));
        }
        
        let mut builder = Builder::new();
        
        // Push required signatures count
        builder = match required_sigs {
            1 => builder.push_opcode(OP_PUSHNUM_1),
            2 => builder.push_opcode(OP_PUSHNUM_2),
            3 => builder.push_opcode(OP_PUSHNUM_3),
            n => builder.push_int(n as i64),
        };
        
        // Push all public keys
        for pubkey in pubkeys {
            builder = builder.push_key(pubkey);
        }
        
        // Push total key count
        builder = match pubkeys.len() {
            1 => builder.push_opcode(OP_PUSHNUM_1),
            2 => builder.push_opcode(OP_PUSHNUM_2),
            3 => builder.push_opcode(OP_PUSHNUM_3),
            n => builder.push_int(n as i64),
        };
        
        // Add OP_CHECKMULTISIG
        builder = builder.push_opcode(OP_CHECKMULTISIG);
        
        Ok(builder.into_script())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oracle_key_generation() {
        let mut key_manager = OracleKeyManager::new();
        
        let pubkey1 = key_manager.generate_oracle_key("oracle1").unwrap();
        let pubkey2 = key_manager.generate_oracle_key("oracle2").unwrap();
        
        assert_ne!(pubkey1, pubkey2);
        assert_eq!(key_manager.list_oracles().len(), 2);
    }
    
    #[test]
    fn test_oracle_signature() {
        let mut key_manager = OracleKeyManager::new();
        key_manager.generate_oracle_key("test_oracle").unwrap();
        
        let price = 50000.0;
        let timestamp = chrono::Utc::now().timestamp();
        
        let signature = key_manager.sign_price_data("test_oracle", price, timestamp).unwrap();
        
        assert_eq!(signature.oracle_name, "test_oracle");
        assert_eq!(signature.price, price);
        assert_eq!(signature.timestamp, timestamp);
        
        // Verify the signature
        assert!(key_manager.verify_oracle_signature(&signature).unwrap());
    }
    
    #[test]
    fn test_threshold_signatures() {
        let mut key_manager = OracleKeyManager::new();
        
        // Generate keys for 5 oracles
        for i in 1..=5 {
            key_manager.generate_oracle_key(&format!("oracle{}", i)).unwrap();
        }
        
        // Create signatures from 3 oracles (threshold)
        let timestamp = chrono::Utc::now().timestamp();
        let mut signatures = Vec::new();
        
        for i in 1..=3 {
            let price = 50000.0 + (i as f64 * 10.0); // Slight price variations
            let sig = key_manager.sign_price_data(&format!("oracle{}", i), price, timestamp).unwrap();
            signatures.push(sig);
        }
        
        // Create threshold scheme (3 of 5)
        let threshold_scheme = ThresholdSignatureScheme::new(3, 5).unwrap();
        
        // Aggregate signatures
        let agg_sig = threshold_scheme.aggregate_signatures(signatures).unwrap();
        
        assert_eq!(agg_sig.participating_oracles.len(), 3);
        assert_eq!(agg_sig.threshold, 3);
        assert_eq!(agg_sig.total_oracles, 5);
        assert_eq!(agg_sig.consensus_price, 50010.0); // Median of 50010, 50020, 50030
    }
    
    #[test]
    fn test_multisig_script_creation() {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};
        use bitcoin::{PrivateKey, Network, PublicKey as BitcoinPublicKey};
        
        let secp = Secp256k1::new();
        let mut pubkeys = Vec::new();
        
        for _ in 0..3 {
            let secret_key = SecretKey::new(&mut rand::thread_rng());
            let private_key = PrivateKey::new(secret_key, Network::Testnet);
            let public_key = BitcoinPublicKey::from_private_key(&secp, &private_key);
            pubkeys.push(public_key);
        }
        
        let script = script_utils::create_multisig_script(&pubkeys, 2).unwrap();
        assert!(!script.is_empty());
    }
}
