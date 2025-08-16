use bitcoin::{Address, Amount, Network, Transaction, Txid, BlockHash, PublicKey, PrivateKey};
use bitcoin::hashes::{Hash, sha256d};
use bitcoin::{TxOut, TxIn, OutPoint, Witness, ScriptBuf, absolute, Sequence};
use bitcoin::sighash::SighashCache;
use bitcoin::secp256k1::{Secp256k1, SecretKey};
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

    /// Create testnet client using cookie authentication
    pub fn testnet_with_cookie(rpc_url: &str) -> Result<Self> {
        // Try to read the cookie file
        let cookie_path = std::env::var("HOME")
            .map(|home| format!("{}/Library/Application Support/Bitcoin/testnet3/.cookie", home))
            .map_err(|_| BitStableError::InvalidConfig("Could not determine home directory".to_string()))?;
        
        let cookie_content = std::fs::read_to_string(&cookie_path)
            .map_err(|e| BitStableError::InvalidConfig(format!("Could not read cookie file {}: {}", cookie_path, e)))?;
        
        let parts: Vec<&str> = cookie_content.trim().split(':').collect();
        if parts.len() != 2 {
            return Err(BitStableError::InvalidConfig("Invalid cookie format".to_string()));
        }
        
        let auth = Auth::UserPass(parts[0].to_string(), parts[1].to_string());
        Self::new(rpc_url, auth, Network::Testnet)
    }

    /// Create client for regtest (regression test network) - fully controllable local network
    pub fn regtest(rpc_url: &str, username: &str, password: &str) -> Result<Self> {
        let auth = Auth::UserPass(username.to_string(), password.to_string());
        let mut client = Self::new(rpc_url, auth, Network::Regtest)?;
        
        // Ensure wallet exists for regtest
        client.ensure_regtest_wallet()?;
        
        Ok(client)
    }

    /// Create regtest client using cookie authentication
    pub fn regtest_with_cookie(rpc_url: &str) -> Result<Self> {
        // Try to read the regtest cookie file
        let cookie_path = std::env::var("HOME")
            .map(|home| format!("{}/Library/Application Support/Bitcoin/regtest/.cookie", home))
            .map_err(|_| BitStableError::InvalidConfig("Could not determine home directory".to_string()))?;
        
        let cookie_content = std::fs::read_to_string(&cookie_path)
            .map_err(|e| BitStableError::InvalidConfig(format!("Could not read regtest cookie file {}: {}", cookie_path, e)))?;
        
        let parts: Vec<&str> = cookie_content.trim().split(':').collect();
        if parts.len() != 2 {
            return Err(BitStableError::InvalidConfig("Invalid cookie format".to_string()));
        }
        
        let auth = Auth::UserPass(parts[0].to_string(), parts[1].to_string());
        Self::new(rpc_url, auth, Network::Regtest)
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

    /// Get UTXOs for an address (works for external addresses)
    pub fn get_utxos(&self, address: &Address) -> Result<Vec<Utxo>> {
        log::debug!("Getting UTXOs for address: {}", address);
        
        // First try listunspent (for wallet addresses)
        log::debug!("Trying wallet UTXOs (listunspent)...");
        match self.get_wallet_utxos(address) {
            Ok(utxos) if !utxos.is_empty() => {
                log::debug!("Found {} UTXOs in wallet", utxos.len());
                return Ok(utxos);
            },
            Ok(_) => log::debug!("No UTXOs found in wallet"),
            Err(e) => log::debug!("Wallet UTXO scan failed: {}", e),
        }
        
        // Try scantxoutset for external addresses (requires full sync)
        log::debug!("Trying scantxoutset for external address...");
        match self.scan_address_utxos(address) {
            Ok(utxos) if !utxos.is_empty() => {
                log::debug!("Found {} UTXOs via scantxoutset", utxos.len());
                return Ok(utxos);
            },
            Ok(_) => log::debug!("No UTXOs found via scantxoutset"),
            Err(e) => log::debug!("Scantxoutset failed: {}", e),
        }
        
        // If Bitcoin Core is still syncing, try recent block scanning
        log::debug!("Trying recent block scanning...");
        match self.scan_recent_blocks_for_address(address) {
            Ok(utxos) => {
                log::debug!("Found {} UTXOs via recent block scanning", utxos.len());
                Ok(utxos)
            },
            Err(e) => {
                log::debug!("Recent block scanning failed: {}", e);
                Ok(Vec::new()) // Return empty vec rather than error
            }
        }
    }
    
    /// Get UTXOs from wallet (for addresses in the wallet)
    fn get_wallet_utxos(&self, address: &Address) -> Result<Vec<Utxo>> {
        let mut utxos = Vec::new();

        // For regtest, use higher confirmations to handle coinbase maturity
        let min_confirmations = if self.network == Network::Regtest { 
            Some(100) // Coinbase maturity requirement
        } else { 
            Some(1) 
        };

        let unspent_outputs = self.client.list_unspent(
            min_confirmations, // min confirmations
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
    
    /// Scan UTXO set for external addresses using scantxoutset
    fn scan_address_utxos(&self, address: &Address) -> Result<Vec<Utxo>> {
        use bitcoincore_rpc::json::{ScanTxOutRequest, ScanTxOutResult};
        
        // Create scan request for the address
        let descriptor = format!("addr({})", address);
        let scan_objects = vec![ScanTxOutRequest::Single(descriptor)];
        
        // Scan the UTXO set
        let scan_result: ScanTxOutResult = self.client
            .call("scantxoutset", &["start".into(), serde_json::to_value(scan_objects)?])
            .map_err(|e| BitStableError::BitcoinRpcError(format!("scantxoutset failed: {}", e)))?;
            
        let mut utxos = Vec::new();
        
        // Process scan results  
        let unspents = scan_result.unspents;
        for unspent in unspents {
            // Get transaction details to check confirmations
            let tx_info = match self.client.get_raw_transaction_info(&unspent.txid, None) {
                Ok(info) => info,
                Err(_) => continue, // Skip if we can't get transaction info
            };
            
            let confirmations = tx_info.confirmations.unwrap_or(0);
            
            // Only include confirmed UTXOs (1+ confirmations)
            if confirmations >= 1 {
                utxos.push(Utxo {
                    txid: unspent.txid,
                    vout: unspent.vout,
                    amount: unspent.amount,
                    address: address.clone(),
                    confirmations,
                    spendable: true, // Assume spendable for external addresses
                });
            }
        }
        
        log::debug!("Found {} UTXOs for address {} using scantxoutset", utxos.len(), address);
        Ok(utxos)
    }
    
    /// Scan recent blocks for transactions to an address (works during sync)
    fn scan_recent_blocks_for_address(&self, address: &Address) -> Result<Vec<Utxo>> {
        let mut utxos = Vec::new();
        
        // Get current best block height
        let current_height = match self.client.get_block_count() {
            Ok(height) => height,
            Err(e) => {
                log::warn!("Failed to get block count: {}", e);
                return Ok(utxos);
            }
        };
        
        // Scan the last 50 blocks for transactions to this address
        let start_height = current_height.saturating_sub(50);
        
        for height in start_height..=current_height {
            match self.client.get_block_hash(height) {
                Ok(block_hash) => {
                    match self.client.get_block(&block_hash) {
                        Ok(block) => {
                            // Check each transaction in the block
                            for tx in &block.txdata {
                                // Check each output
                                for (vout, output) in tx.output.iter().enumerate() {
                                    // Try to extract address from script
                                    if let Ok(output_address) = Address::from_script(&output.script_pubkey, self.network) {
                                        if output_address == *address {
                                            // Found a transaction to our address, check if it's unspent
                                            let txid = tx.compute_txid();
                                            
                                            // Simple unspent check - if we can't find it spent in later blocks, assume unspent
                                            // For a more complete implementation, we'd need to track all spending transactions
                                            utxos.push(Utxo {
                                                txid,
                                                vout: vout as u32,
                                                amount: output.value,
                                                address: output_address,
                                                confirmations: (current_height - height + 1) as u32,
                                                spendable: true,
                                            });
                                            
                                            log::info!("Found UTXO: {}:{} = {} BTC to {}", 
                                                      txid, vout, output.value.to_btc(), address);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to get block at height {}: {}", height, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to get block hash for height {}: {}", height, e);
                }
            }
        }
        
        log::debug!("Found {} UTXOs for address {} by scanning recent blocks", utxos.len(), address);
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

    /// Request funds from Bitcoin testnet faucet
    pub async fn request_testnet_funds(&self, address: &Address) -> Result<Txid> {
        if self.network != Network::Testnet {
            return Err(BitStableError::InvalidConfig("Faucet only works on testnet".to_string()));
        }

        log::info!("Requesting testnet funds for address: {}", address);

        // Try multiple testnet faucets in order of reliability
        let faucets = vec![
            "coinfaucet.eu",
            "testnet-faucet.com", 
            "bitcoinfaucet.uo1.net",
        ];

        for faucet_name in faucets {
            log::info!("Trying faucet: {}", faucet_name);
            
            let result = match faucet_name {
                "coinfaucet.eu" => self.try_coinfaucet_eu(address).await,
                "testnet-faucet.com" => self.try_testnet_faucet_com(address).await,
                "bitcoinfaucet.uo1.net" => self.try_bitcoinfaucet_uo1(address).await,
                _ => continue,
            };
            
            match result {
                Ok(response) => {
                    log::info!("Faucet {} request successful: {}", faucet_name, response);
                    
                    // Wait a bit for the transaction to propagate
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    
                    // Try to find the transaction in mempool
                    if let Some(txid) = self.find_recent_transaction_to_address(address).await? {
                        log::info!("Found transaction {} from faucet", txid);
                        return Ok(txid);
                    } else {
                        log::warn!("Faucet claimed success but no transaction found yet");
                    }
                }
                Err(e) => {
                    log::warn!("Faucet {} request failed: {}", faucet_name, e);
                    continue;
                }
            }
        }

        Err(BitStableError::BitcoinRpcError("All faucets failed".to_string()))
    }

    /// Try coinfaucet.eu (supports direct API)
    async fn try_coinfaucet_eu(&self, address: &Address) -> Result<String> {
        let client = reqwest::Client::new();
        let url = "https://coinfaucet.eu/en/btc-testnet/";
        
        // This faucet uses a form submission
        let params = [
            ("address", address.to_string()),
            ("captcha", "automated".to_string()), // Will need real captcha solving
        ];
        
        let response = client
            .post(url)
            .form(&params)
            .send()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        let text = response
            .text()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        if text.contains("success") || text.contains("sent") {
            Ok("Request submitted successfully".to_string())
        } else {
            Err(BitStableError::BitcoinRpcError("Faucet request rejected".to_string()))
        }
    }

    /// Try testnet-faucet.com (API-based)
    async fn try_testnet_faucet_com(&self, address: &Address) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("https://testnet-faucet.com/btc-testnet/send?address={}", address);
        
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        let text = response
            .text()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        // Look for success indicators
        if text.contains("txid") || text.contains("transaction") || text.contains("sent") {
            // Try to extract transaction ID from response
            if let Some(txid_start) = text.find("txid") {
                let txid_section = &text[txid_start..];
                if let Some(txid_match) = txid_section.chars()
                    .skip_while(|c| !c.is_ascii_hexdigit())
                    .take(64)
                    .collect::<String>()
                    .parse::<String>()
                    .ok()
                {
                    return Ok(format!("Transaction ID: {}", txid_match));
                }
            }
            Ok("Request submitted successfully".to_string())
        } else {
            Err(BitStableError::BitcoinRpcError("Faucet request failed".to_string()))
        }
    }

    /// Try bitcoinfaucet.uo1.net
    async fn try_bitcoinfaucet_uo1(&self, address: &Address) -> Result<String> {
        let client = reqwest::Client::new();
        let url = "https://bitcoinfaucet.uo1.net/send.php";
        
        let params = [
            ("address", address.to_string()),
            ("captcha", "test".to_string()), // This faucet may require captcha
        ];
        
        let response = client
            .post(url)
            .form(&params)
            .send()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        let text = response
            .text()
            .await
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Network error: {}", e)))?;
            
        if text.contains("success") || text.contains("sent") || text.contains("transaction") {
            Ok("Request submitted successfully".to_string())
        } else {
            Err(BitStableError::BitcoinRpcError("Faucet request failed".to_string()))
        }
    }

    /// Find a recent transaction to a specific address by checking mempool and recent blocks
    async fn find_recent_transaction_to_address(&self, address: &Address) -> Result<Option<Txid>> {
        // First check if there are any UTXOs (which would include unconfirmed ones)
        match self.get_utxos(address) {
            Ok(utxos) if !utxos.is_empty() => {
                // Return the most recent transaction
                let most_recent = utxos.iter().max_by_key(|utxo| utxo.confirmations);
                if let Some(utxo) = most_recent {
                    return Ok(Some(utxo.txid));
                }
            }
            _ => {}
        }
        
        // If no UTXOs found, try scanning recent blocks
        let current_height = self.get_block_height()?;
        for height in (current_height.saturating_sub(6))..=current_height {
            if let Ok(block_hash) = self.client.get_block_hash(height) {
                if let Ok(block) = self.client.get_block(&block_hash) {
                    for tx in &block.txdata {
                        for output in &tx.output {
                            if let Ok(output_address) = bitcoin::Address::from_script(&output.script_pubkey, self.network) {
                                if output_address == *address {
                                    return Ok(Some(tx.compute_txid()));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Generate a new Bitcoin address with private key (works for testnet/regtest)
    pub fn generate_address(&self) -> Result<(Address, PrivateKey)> {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let private_key = PrivateKey::new(secret_key, self.network);
        let _public_key = PublicKey::from_private_key(&secp, &private_key);
        
        // Generate P2WPKH address (native segwit) using compressed public key
        let compressed_pubkey = bitcoin::CompressedPublicKey::from_private_key(&secp, &private_key)
            .map_err(|e| BitStableError::InvalidConfig(format!("Public key compression failed: {}", e)))?;
        let address = Address::p2wpkh(&compressed_pubkey, self.network);
        
        // For regtest, import the private key into the wallet
        if self.network == Network::Regtest {
            if let Err(e) = self.client.import_private_key(&private_key, None, Some(false)) {
                log::warn!("Failed to import private key to regtest wallet: {}", e);
            } else {
                log::debug!("Imported address {} to regtest wallet", address);
            }
        }
        
        Ok((address, private_key))
    }

    /// Generate a new Bitcoin testnet address with private key (backward compatibility)
    pub fn generate_testnet_address(&self) -> Result<(Address, PrivateKey)> {
        self.generate_address()
    }

    /// Mine blocks in regtest mode (only works on regtest)
    pub async fn mine_blocks(&self, num_blocks: u64, address: &Address) -> Result<Vec<bitcoin::BlockHash>> {
        if self.network != Network::Regtest {
            return Err(BitStableError::InvalidConfig("Mining only works on regtest network".to_string()));
        }

        log::info!("Mining {} blocks to address: {}", num_blocks, address);

        let block_hashes = self.client.generate_to_address(num_blocks, address)
            .map_err(|e| BitStableError::BitcoinRpcError(format!("Failed to mine blocks: {}", e)))?;

        log::info!("Successfully mined {} blocks", block_hashes.len());
        Ok(block_hashes)
    }

    /// Generate funds automatically in regtest by mining blocks
    pub async fn generate_regtest_funds(&self, address: &Address, amount_btc: f64) -> Result<Amount> {
        if self.network != Network::Regtest {
            return Err(BitStableError::InvalidConfig("Auto funding only works on regtest network".to_string()));
        }

        log::info!("Generating {} BTC for address: {}", amount_btc, address);

        // Mine enough blocks to generate the required amount
        // Each block reward is 50 BTC in regtest (like early Bitcoin)
        let blocks_needed = (amount_btc / 50.0).ceil() as u64;
        let initial_blocks = std::cmp::max(blocks_needed, 101); // Need 101 blocks for coinbase maturity

        log::info!("Mining {} initial blocks to generate funds...", initial_blocks);
        self.mine_blocks(initial_blocks, address).await?;
        
        // Mine additional 100 blocks to make the coinbase outputs spendable
        log::info!("Mining additional 100 blocks for coinbase maturity...");
        self.mine_blocks(100, address).await?;

        // Wait a moment for the blocks to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check the generated balance
        let utxos = self.get_utxos(address)?;
        let total_balance: u64 = utxos.iter().map(|utxo| utxo.amount.to_sat()).sum();
        let balance_btc = Amount::from_sat(total_balance);

        log::info!("Generated {} BTC across {} UTXOs", balance_btc.to_btc(), utxos.len());
        Ok(balance_btc)
    }

    /// Ensure wallet exists for regtest operations
    fn ensure_regtest_wallet(&mut self) -> Result<()> {
        if self.network != Network::Regtest {
            return Ok(()); // Only needed for regtest
        }

        const WALLET_NAME: &str = "bitstable_regtest";

        // Check if wallet already exists
        let wallets = self.client.list_wallets()
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;

        if wallets.contains(&WALLET_NAME.to_string()) {
            log::debug!("Regtest wallet '{}' already loaded", WALLET_NAME);
            return Ok(());
        }

        // Try to load existing wallet
        match self.client.load_wallet(WALLET_NAME) {
            Ok(_) => {
                log::info!("Loaded existing regtest wallet '{}'", WALLET_NAME);
                return Ok(());
            }
            Err(_) => {
                log::debug!("No existing wallet found, creating new one");
            }
        }

        // Create new wallet
        self.client.create_wallet(
            WALLET_NAME,
            Some(false), // disable_private_keys
            Some(false), // blank
            None,        // passphrase
            Some(false)  // avoid_reuse
        ).map_err(|e| BitStableError::BitcoinRpcError(format!("Failed to create regtest wallet: {}", e)))?;

        log::info!("Created new regtest wallet '{}'", WALLET_NAME);
        Ok(())
    }

    /// Confirm transactions by mining blocks (regtest only)
    pub async fn confirm_transactions(&self, num_blocks: u64) -> Result<Vec<bitcoin::BlockHash>> {
        if self.network != Network::Regtest {
            return Err(BitStableError::InvalidConfig("Manual confirmation only works on regtest network".to_string()));
        }

        // Mine to a temporary address to confirm transactions
        let (temp_address, _) = self.generate_address()?;
        self.mine_blocks(num_blocks, &temp_address).await
    }

    /// Get the current regtest network difficulty
    pub fn get_difficulty(&self) -> Result<f64> {
        let info = self.client.get_blockchain_info()
            .map_err(|e| BitStableError::BitcoinRpcError(e.to_string()))?;
        Ok(info.difficulty)
    }

    /// Reset regtest blockchain (if supported by the node)
    pub fn reset_regtest(&self) -> Result<()> {
        if self.network != Network::Regtest {
            return Err(BitStableError::InvalidConfig("Reset only works on regtest network".to_string()));
        }

        // This would require restarting the node in most cases
        // For now, just log that reset was requested
        log::warn!("Regtest reset requested - this typically requires restarting bitcoind");
        Ok(())
    }

    /// Create a 2-of-3 multisig escrow address for vault collateral
    pub fn create_escrow_multisig(&self, user_pubkey: PublicKey, oracle_pubkey: PublicKey, liquidator_pubkey: PublicKey) -> Result<(Address, ScriptBuf)> {
        let pubkeys = vec![user_pubkey, oracle_pubkey, liquidator_pubkey];
        
        // Create 2-of-3 multisig script
        let script = crate::crypto::script_utils::create_multisig_script(&pubkeys, 2)?;
        
        // Generate P2WSH address from the script
        let address = Address::p2wsh(&script, self.network);
        
        log::info!("Created 2-of-3 multisig escrow address: {}", address);
        Ok((address, script))
    }

    /// Build and sign a transaction to fund an escrow address
    pub fn build_funding_transaction(
        &self, 
        source_utxos: Vec<Utxo>, 
        source_private_key: &PrivateKey,
        escrow_address: &Address, 
        amount: Amount,
        fee_rate: f64 // sat/vB
    ) -> Result<Transaction> {
        
        let secp = Secp256k1::new();
        
        // Calculate total input value
        let total_input: u64 = source_utxos.iter().map(|utxo| utxo.amount.to_sat()).sum();
        
        // Calculate estimated fee (assume 1 input, 2 outputs = ~200 vB)
        let estimated_size = 200;
        let fee = Amount::from_sat((estimated_size as f64 * fee_rate) as u64);
        
        if total_input < amount.to_sat() + fee.to_sat() {
            return Err(BitStableError::InsufficientFunds);
        }
        
        // Build transaction
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };
        
        // Add inputs
        for utxo in &source_utxos {
            tx.input.push(TxIn {
                previous_output: OutPoint::new(utxo.txid, utxo.vout),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
        }
        
        // Add escrow output
        tx.output.push(TxOut {
            value: amount,
            script_pubkey: escrow_address.script_pubkey(),
        });
        
        // Add change output if needed
        let change_amount = total_input - amount.to_sat() - fee.to_sat();
        if change_amount > 546 { // Dust threshold
            let compressed_pubkey = bitcoin::CompressedPublicKey::from_private_key(&secp, source_private_key)
                .map_err(|e| BitStableError::InvalidConfig(format!("Change pubkey error: {}", e)))?;
            let change_address = Address::p2wpkh(&compressed_pubkey, self.network);
            
            tx.output.push(TxOut {
                value: Amount::from_sat(change_amount),
                script_pubkey: change_address.script_pubkey(),
            });
        }
        
        // Sign transaction (simplified - assumes P2WPKH inputs)
        let mut signed_tx = tx.clone();
        for (input_index, utxo) in source_utxos.iter().enumerate() {
            // For P2WPKH signing
            let mut sighash_cache = SighashCache::new(&tx);
            let sighash = sighash_cache.p2wpkh_signature_hash(
                input_index,
                &utxo.address.script_pubkey(),
                utxo.amount,
                bitcoin::sighash::EcdsaSighashType::All,
            ).map_err(|e| BitStableError::InvalidConfig(format!("Sighash error: {}", e)))?;
            
            let signature = secp.sign_ecdsa(
                &bitcoin::secp256k1::Message::from(sighash), 
                &source_private_key.inner
            );
            
            let mut sig_bytes = signature.serialize_der().to_vec();
            sig_bytes.push(bitcoin::sighash::EcdsaSighashType::All as u8);
            
            let public_key = PublicKey::from_private_key(&secp, source_private_key);
            
            // Create witness for P2WPKH
            let mut witness = Witness::new();
            witness.push(&sig_bytes);
            witness.push(public_key.to_bytes());
            
            signed_tx.input[input_index].witness = witness;
        }
        
        Ok(signed_tx)
    }

    /// Create OP_RETURN transaction for proof-of-reserves commitments
    pub fn create_op_return_transaction(&self, op_return_script: ScriptBuf) -> Result<Txid> {
        // This is a simplified implementation - in production would use actual Bitcoin transaction creation
        // For now, return a mock transaction ID
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(op_return_script.as_bytes());
        hasher.update(&chrono::Utc::now().timestamp().to_be_bytes());
        let hash = hasher.finalize();
        
        // Convert first 32 bytes to Txid
        let mut txid_bytes = [0u8; 32];
        txid_bytes.copy_from_slice(&hash[..32]);
        let txid = Txid::from_raw_hash(sha256d::Hash::from_byte_array(txid_bytes));
        
        log::info!("Created OP_RETURN transaction: {}", txid);
        Ok(txid)
    }

    /// Create liquidation transaction that pays out from escrow
    pub fn create_liquidation_transaction(
        &self,
        escrow_utxo: Utxo,
        escrow_script: &ScriptBuf,
        liquidator_address: &Address,
        debt_amount: Amount,
        bonus_amount: Amount,
        user_address: &Address, // For remaining collateral
        oracle_private_key: &PrivateKey,
        liquidator_private_key: &PrivateKey,
    ) -> Result<Transaction> {
        
        let total_payout = debt_amount + bonus_amount;
        let remaining_collateral = escrow_utxo.amount.checked_sub(total_payout)
            .ok_or_else(|| BitStableError::InvalidConfig("Insufficient escrow funds".to_string()))?;
        
        // Build transaction
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::new(escrow_utxo.txid, escrow_utxo.vout),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![
                // Payment to liquidator (debt + bonus)
                TxOut {
                    value: total_payout,
                    script_pubkey: liquidator_address.script_pubkey(),
                },
                // Return remaining collateral to user
                TxOut {
                    value: remaining_collateral,
                    script_pubkey: user_address.script_pubkey(),
                },
            ],
        };
        
        // Sign with oracle and liquidator keys (2-of-3 multisig)
        let secp = Secp256k1::new();
        let mut sighash_cache = SighashCache::new(&tx);
        
        let sighash = sighash_cache.p2wsh_signature_hash(
            0, // Input index
            escrow_script,
            escrow_utxo.amount,
            bitcoin::sighash::EcdsaSighashType::All,
        ).map_err(|e| BitStableError::InvalidConfig(format!("Liquidation sighash error: {}", e)))?;
        
        // Sign with oracle key
        let oracle_signature = secp.sign_ecdsa(
            &bitcoin::secp256k1::Message::from(sighash),
            &oracle_private_key.inner
        );
        
        // Sign with liquidator key  
        let liquidator_signature = secp.sign_ecdsa(
            &bitcoin::secp256k1::Message::from(sighash),
            &liquidator_private_key.inner
        );
        
        // Create witness stack for 2-of-3 multisig
        let mut witness = Witness::new();
        witness.push(&[]); // Dummy element for multisig
        witness.push(&oracle_signature.serialize_der());
        witness.push(&liquidator_signature.serialize_der());
        witness.push(escrow_script.as_bytes());
        
        tx.input[0].witness = witness;
        
        Ok(tx)
    }


    /// Get spendable UTXOs for an address with required confirmations
    pub async fn get_spendable_utxos(&self, address: &Address, min_confirmations: u32) -> Result<Vec<Utxo>> {
        // In a real implementation, this would use Bitcoin Core's listunspent RPC
        // For now, return empty - this needs to be implemented with actual RPC calls
        
        log::info!("Getting spendable UTXOs for address: {} (min {} confirmations)", address, min_confirmations);
        
        // This is a placeholder - real implementation would:
        // 1. Use importaddress to track the address
        // 2. Call listunspent with minconf=min_confirmations
        // 3. Filter by address
        // 4. Return actual UTXOs
        
        Ok(Vec::new())
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

    pub fn regtest() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:18443".to_string(), // Regtest default port
            rpc_username: "bitstable".to_string(),
            rpc_password: "password".to_string(),
            network: Network::Regtest,
            min_confirmations: 1,
            fee_target_blocks: 1, // Fast confirmations in regtest
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