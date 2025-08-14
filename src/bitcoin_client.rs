use bitcoin::{Address, Amount, Network, Transaction, Txid, BlockHash};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::{Deserialize, Serialize};
use crate::{BitStableError, Result};

/// Bitcoin network client for interacting with Bitcoin Core RPC
#[derive(Debug)]
pub struct BitcoinClient {
    client: Client,
    network: Network,
}

/// Transaction information from the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub txid: Txid,
    pub confirmations: u32,
    pub block_hash: Option<BlockHash>,
    pub fee: Option<Amount>,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
}

/// Input information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub previous_txid: Txid,
    pub vout: u32,
    pub value: Amount,
    #[serde(with = "address_option_serde")]
    pub address: Option<Address>,
}

/// Output information  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub vout: u32,
    pub value: Amount,
    #[serde(with = "address_option_serde")]
    pub address: Option<Address>,
    pub spent: bool,
}

/// UTXO (Unspent Transaction Output) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utxo {
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    #[serde(with = "address_serde")]
    pub address: Address,
    pub confirmations: u32,
    pub spendable: bool,
}

/// Bitcoin network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub block_height: u64,
    pub difficulty: f64,
    pub mempool_size: usize,
    pub estimated_fee_rate: f64, // sat/vB
}

impl BitcoinClient {
    /// Create a new Bitcoin client
    pub fn new(rpc_url: &str, auth: Auth, network: Network) -> Result<Self> {
        let client = Client::new(rpc_url, auth)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        Ok(Self { client, network })
    }

    /// Create client with default settings for testnet
    pub fn testnet(rpc_url: &str, username: &str, password: &str) -> Result<Self> {
        let auth = Auth::UserPass(username.to_string(), password.to_string());
        Self::new(rpc_url, auth, Network::Testnet)
    }

    /// Create client with default settings for mainnet
    pub fn mainnet(rpc_url: &str, username: &str, password: &str) -> Result<Self> {
        let auth = Auth::UserPass(username.to_string(), password.to_string());
        Self::new(rpc_url, auth, Network::Bitcoin)
    }

    /// Get blockchain info
    pub fn get_blockchain_info(&self) -> Result<NetworkStats> {
        let info = self.client.get_blockchain_info()
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        let mempool_info = self.client.get_mempool_info()
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        // Estimate fee for 6 block confirmation
        let fee_rate = self.client.estimate_smart_fee(6, None)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?
            .fee_rate
            .map(|rate| rate.to_btc() * 100_000_000.0) // Convert to sat/vB
            .unwrap_or(1.0);

        Ok(NetworkStats {
            block_height: info.blocks,
            difficulty: info.difficulty,
            mempool_size: mempool_info.size,
            estimated_fee_rate: fee_rate,
        })
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid> {
        let txid = self.client.send_raw_transaction(tx)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        log::info!("Broadcasted transaction: {}", txid);
        Ok(txid)
    }

    /// Get transaction information
    pub fn get_transaction(&self, txid: Txid) -> Result<TransactionInfo> {
        let tx_result = self.client.get_raw_transaction_info(&txid, None)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        let mut inputs = Vec::new();
        for input in &tx_result.vin {
            if let Some(prev_txid) = input.txid {
                // Get previous transaction to find input value
                if let Ok(prev_tx) = self.client.get_raw_transaction_info(&prev_txid, None) {
                    if let Some(prev_out) = prev_tx.vout.get(input.vout.unwrap_or(0) as usize) {
                        let prev_address = prev_out.script_pub_key.address.clone()
                            .map(|addr| addr.assume_checked());
                        
                        inputs.push(TxInput {
                            previous_txid: prev_txid,
                            vout: input.vout.unwrap_or(0),
                            value: prev_out.value,
                            address: prev_address,
                        });
                    }
                }
            }
        }

        let mut outputs = Vec::new();
        for (vout, output) in tx_result.vout.iter().enumerate() {
            let output_address = output.script_pub_key.address.clone()
                .map(|addr| addr.assume_checked());
                
            outputs.push(TxOutput {
                vout: vout as u32,
                value: output.value,
                address: output_address,
                spent: false, // Would need additional RPC call to check if spent
            });
        }

        Ok(TransactionInfo {
            txid,
            confirmations: tx_result.confirmations.unwrap_or(0),
            block_hash: tx_result.blockhash,
            fee: None, // Would need to calculate from inputs/outputs
            inputs,
            outputs,
        })
    }

    /// Check if a transaction is confirmed
    pub fn is_transaction_confirmed(&self, txid: Txid, min_confirmations: u32) -> Result<bool> {
        match self.get_transaction(txid) {
            Ok(tx_info) => Ok(tx_info.confirmations >= min_confirmations),
            Err(_) => Ok(false), // Transaction not found
        }
    }

    /// Get UTXOs for an address
    pub fn get_utxos(&self, address: &Address) -> Result<Vec<Utxo>> {
        let mut utxos = Vec::new();

        // Use listunspent RPC call
        let unspent_outputs = self.client.list_unspent(
            Some(1), // min confirmations
            None,    // max confirmations  
            Some(&[address]),
            None,    // include_unsafe
            None,    // query_options
        ).map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        for utxo in unspent_outputs {
            let utxo_address = utxo.address.clone()
                .map(|addr| addr.assume_checked())
                .unwrap_or_else(|| address.clone());
                
            utxos.push(Utxo {
                txid: utxo.txid,
                vout: utxo.vout,
                amount: utxo.amount,
                address: utxo_address,
                confirmations: utxo.confirmations,
                spendable: utxo.spendable,
            });
        }

        Ok(utxos)
    }

    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(
        &self,
        txid: Txid,
        min_confirmations: u32,
        timeout_seconds: u64,
    ) -> Result<bool> {
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_seconds);

        while start_time.elapsed() < timeout {
            if self.is_transaction_confirmed(txid, min_confirmations)? {
                log::info!(
                    "Transaction {} confirmed with {} confirmations",
                    txid,
                    min_confirmations
                );
                return Ok(true);
            }

            // Wait 30 seconds before checking again
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }

        log::warn!("Transaction {} confirmation timeout", txid);
        Ok(false)
    }

    /// Monitor address for incoming transactions
    pub async fn monitor_address(
        &self,
        address: &Address,
        callback: impl Fn(TransactionInfo) -> Result<()>,
    ) -> Result<()> {
        let mut known_txids = std::collections::HashSet::new();

        loop {
            // Get recent transactions for this address
            // Note: This is a simplified implementation
            // In production, you'd use ZMQ notifications or wallet imports
            
            let utxos = self.get_utxos(address)?;
            
            for utxo in utxos {
                if !known_txids.contains(&utxo.txid) {
                    known_txids.insert(utxo.txid);
                    
                    if let Ok(tx_info) = self.get_transaction(utxo.txid) {
                        callback(tx_info)?;
                    }
                }
            }

            // Check every minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    /// Get current block height
    pub fn get_block_height(&self) -> Result<u64> {
        let info = self.client.get_blockchain_info()
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;
        
        Ok(info.blocks)
    }

    /// Estimate transaction fee
    pub fn estimate_fee(&self, tx_size_bytes: usize, target_blocks: u16) -> Result<Amount> {
        let fee_rate_result = self.client.estimate_smart_fee(target_blocks, None)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;
            
        let fee_rate_btc = fee_rate_result.fee_rate
            .map(|rate| rate.to_btc())
            .unwrap_or(0.00001); // Default 1000 sat per KB fallback

        let fee_btc = fee_rate_btc * (tx_size_bytes as f64 / 1000.0); // fee is per KB
        
        Amount::from_btc(fee_btc)
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))
    }

    /// Check if the node is ready and synced
    pub fn is_ready(&self) -> Result<bool> {
        match self.client.get_blockchain_info() {
            Ok(info) => {
                // Consider ready if we're within 10 blocks of current time
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                // Estimate if we're caught up (10 minute block time average)
                let estimated_height = current_time / 600; // Very rough estimate
                let behind_blocks = estimated_height.saturating_sub(info.blocks);
                
                Ok(behind_blocks < 10)
            }
            Err(_) => Ok(false),
        }
    }

    /// Get network (testnet/mainnet)
    pub fn network(&self) -> Network {
        self.network
    }
}

/// Bitcoin network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinConfig {
    pub rpc_url: String,
    pub rpc_username: String,
    pub rpc_password: String,
    pub network: Network,
    pub min_confirmations: u32,
    pub fee_target_blocks: u16,
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

/// Custom serde module for Option<Address> serialization
mod address_option_serde {
    use serde::{Deserialize, Deserializer, Serializer, Serialize};
    use bitcoin::Address;

    pub fn serialize<S>(address: &Option<Address>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match address {
            Some(addr) => Some(addr.to_string()).serialize(serializer),
            None => None::<String>.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt_s = Option::<String>::deserialize(deserializer)?;
        match opt_s {
            Some(s) => s.parse::<Address<_>>()
                .map(|addr| Some(addr.assume_checked()))
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:18332".to_string(), // Testnet default
            rpc_username: "bitstable".to_string(),
            rpc_password: "password".to_string(),
            network: Network::Testnet,
            min_confirmations: 1,
            fee_target_blocks: 6,
        }
    }
}

impl BitcoinConfig {
    pub fn mainnet() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:8332".to_string(),
            network: Network::Bitcoin,
            ..Default::default()
        }
    }

    pub fn create_client(&self) -> Result<BitcoinClient> {
        let auth = Auth::UserPass(self.rpc_username.clone(), self.rpc_password.clone());
        BitcoinClient::new(&self.rpc_url, auth, self.network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitcoin_config_creation() {
        let config = BitcoinConfig::default();
        assert_eq!(config.network, Network::Testnet);
        assert_eq!(config.min_confirmations, 1);

        let mainnet_config = BitcoinConfig::mainnet();
        assert_eq!(mainnet_config.network, Network::Bitcoin);
    }

    // Note: Integration tests would require a running Bitcoin Core node
    #[test] 
    #[ignore] // Ignore by default since it requires Bitcoin Core
    fn test_bitcoin_client_connection() {
        let config = BitcoinConfig::default();
        let client = config.create_client();
        
        // This test would only pass with a running Bitcoin Core testnet node
        assert!(client.is_ok());
    }
}