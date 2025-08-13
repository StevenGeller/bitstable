use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use bitcoin::PublicKey;
use crate::{BitStableError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pubkey: PublicKey,
    pub address: String,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub services: Vec<ServiceType>,
    pub reputation_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceType {
    Oracle,
    Liquidator,
    VaultProvider,
    StableHolder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub message_type: MessageType,
    pub sender: PublicKey,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: MessageData,
    pub signature: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub enum MessageType {
    PriceUpdate,
    LiquidationAlert,
    VaultCreated,
    VaultLiquidated,
    PeerAnnouncement,
    StableTransfer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageData {
    PriceUpdate {
        price_usd: f64,
        oracle_source: String,
        signature: Vec<u8>,
    },
    LiquidationAlert {
        vault_id: bitcoin::Txid,
        collateral_ratio: f64,
        potential_bonus: bitcoin::Amount,
    },
    VaultCreated {
        vault_id: bitcoin::Txid,
        collateral_amount: bitcoin::Amount,
        stable_debt: f64,
    },
    VaultLiquidated {
        vault_id: bitcoin::Txid,
        liquidator: PublicKey,
        bonus_earned: bitcoin::Amount,
    },
    PeerAnnouncement {
        services: Vec<ServiceType>,
        endpoint: String,
    },
    StableTransfer {
        from: PublicKey,
        to: PublicKey,
        amount_usd: f64,
    },
}

pub struct BitStableNetwork {
    local_pubkey: PublicKey,
    peers: HashMap<PublicKey, PeerInfo>,
    message_handlers: HashMap<MessageType, Box<dyn Fn(&NetworkMessage) -> Result<()>>>,
    connection_pool: ConnectionPool,
}

pub struct ConnectionPool {
    connections: HashMap<PublicKey, Connection>,
    max_connections: usize,
}

pub struct Connection {
    peer: PublicKey,
    stream: Option<tokio::net::TcpStream>,
    last_activity: chrono::DateTime<chrono::Utc>,
    is_connected: bool,
}

impl BitStableNetwork {
    pub fn new(local_pubkey: PublicKey, max_connections: usize) -> Self {
        Self {
            local_pubkey,
            peers: HashMap::new(),
            message_handlers: HashMap::new(),
            connection_pool: ConnectionPool {
                connections: HashMap::new(),
                max_connections,
            },
        }
    }

    pub async fn start(&mut self, bind_address: &str) -> Result<()> {
        // Start listening for incoming connections
        let listener = tokio::net::TcpListener::bind(bind_address).await
            .map_err(|e| BitStableError::InvalidConfig(format!("Failed to bind: {}", e)))?;

        log::info!("BitStable network node started on {}", bind_address);

        // Start peer discovery
        self.start_peer_discovery().await?;

        // Main network loop
        loop {
            tokio::select! {
                // Handle incoming connections
                Ok((stream, addr)) = listener.accept() => {
                    log::debug!("New connection from {}", addr);
                    self.handle_incoming_connection(stream).await?;
                }
                
                // Periodic maintenance
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    self.maintenance_cycle().await?;
                }
            }
        }
    }

    async fn start_peer_discovery(&mut self) -> Result<()> {
        // Connect to bootstrap nodes
        let bootstrap_nodes = vec![
            // In a real implementation, these would be well-known BitStable nodes
            ("127.0.0.1:8333", "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"),
            ("127.0.0.1:8334", "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
        ];

        for (addr, pubkey_str) in bootstrap_nodes {
            if let Ok(pubkey) = pubkey_str.parse() {
                if let Err(e) = self.connect_to_peer(addr, pubkey).await {
                    log::warn!("Failed to connect to bootstrap node {}: {}", addr, e);
                }
            }
        }

        Ok(())
    }

    pub async fn connect_to_peer(&mut self, address: &str, pubkey: PublicKey) -> Result<()> {
        if self.connection_pool.connections.len() >= self.connection_pool.max_connections {
            return Err(BitStableError::InvalidConfig("Max connections reached".to_string()));
        }

        let stream = tokio::net::TcpStream::connect(address).await
            .map_err(|e| BitStableError::InvalidConfig(format!("Connection failed: {}", e)))?;

        let connection = Connection {
            peer: pubkey,
            stream: Some(stream),
            last_activity: chrono::Utc::now(),
            is_connected: true,
        };

        self.connection_pool.connections.insert(pubkey, connection);

        // Send peer announcement
        self.send_peer_announcement(pubkey).await?;

        log::info!("Connected to peer: {}", pubkey);
        Ok(())
    }

    async fn handle_incoming_connection(&mut self, stream: tokio::net::TcpStream) -> Result<()> {
        // In a real implementation, this would:
        // 1. Perform handshake to identify peer
        // 2. Verify peer identity
        // 3. Add to connection pool
        // 4. Start message handling loop

        log::debug!("Handling incoming connection");
        Ok(())
    }

    pub async fn broadcast_message(&self, message: NetworkMessage) -> Result<()> {
        let serialized = serde_json::to_vec(&message)?;
        
        for (peer_pubkey, connection) in &self.connection_pool.connections {
            if connection.is_connected && *peer_pubkey != self.local_pubkey {
                // In a real implementation, send over the TCP stream
                log::debug!("Broadcasting message to peer: {}", peer_pubkey);
            }
        }

        Ok(())
    }

    pub async fn send_price_update(&self, price: f64, source: String, signature: Vec<u8>) -> Result<()> {
        let message = NetworkMessage {
            message_type: MessageType::PriceUpdate,
            sender: self.local_pubkey,
            timestamp: chrono::Utc::now(),
            data: MessageData::PriceUpdate {
                price_usd: price,
                oracle_source: source,
                signature,
            },
            signature: None,
        };

        self.broadcast_message(message).await
    }

    pub async fn send_liquidation_alert(
        &self,
        vault_id: bitcoin::Txid,
        collateral_ratio: f64,
        bonus: bitcoin::Amount,
    ) -> Result<()> {
        let message = NetworkMessage {
            message_type: MessageType::LiquidationAlert,
            sender: self.local_pubkey,
            timestamp: chrono::Utc::now(),
            data: MessageData::LiquidationAlert {
                vault_id,
                collateral_ratio,
                potential_bonus: bonus,
            },
            signature: None,
        };

        self.broadcast_message(message).await
    }

    pub async fn announce_vault_creation(
        &self,
        vault_id: bitcoin::Txid,
        collateral: bitcoin::Amount,
        debt: f64,
    ) -> Result<()> {
        let message = NetworkMessage {
            message_type: MessageType::VaultCreated,
            sender: self.local_pubkey,
            timestamp: chrono::Utc::now(),
            data: MessageData::VaultCreated {
                vault_id,
                collateral_amount: collateral,
                stable_debt: debt,
            },
            signature: None,
        };

        self.broadcast_message(message).await
    }

    async fn send_peer_announcement(&self, target_peer: PublicKey) -> Result<()> {
        let message = NetworkMessage {
            message_type: MessageType::PeerAnnouncement,
            sender: self.local_pubkey,
            timestamp: chrono::Utc::now(),
            data: MessageData::PeerAnnouncement {
                services: vec![ServiceType::VaultProvider, ServiceType::StableHolder],
                endpoint: "127.0.0.1:8335".to_string(), // Our listening address
            },
            signature: None,
        };

        // Send directly to specific peer instead of broadcasting
        log::debug!("Sending peer announcement to {}", target_peer);
        Ok(())
    }

    async fn maintenance_cycle(&mut self) -> Result<()> {
        let now = chrono::Utc::now();
        
        // Remove stale connections
        self.connection_pool.connections.retain(|_, connection| {
            let age = now.signed_duration_since(connection.last_activity);
            age.num_minutes() < 30 // Keep connections active for 30 minutes
        });

        // Update peer reputation scores
        self.update_peer_reputations().await?;

        // Attempt to maintain minimum number of connections
        if self.connection_pool.connections.len() < 3 {
            log::info!("Low peer count, attempting to find more peers");
            // In a real implementation, this would trigger peer discovery
        }

        Ok(())
    }

    async fn update_peer_reputations(&mut self) -> Result<()> {
        // Update reputation based on:
        // - Message reliability
        // - Response times
        // - Service quality
        
        for (pubkey, peer) in &mut self.peers {
            // Simple reputation decay
            peer.reputation_score *= 0.99;
            
            // Bonus for recent activity
            let hours_since_activity = chrono::Utc::now()
                .signed_duration_since(peer.last_seen)
                .num_hours();
            
            if hours_since_activity < 24 {
                peer.reputation_score += 0.01;
            }
            
            peer.reputation_score = peer.reputation_score.clamp(0.0, 1.0);
        }

        Ok(())
    }

    pub fn get_peers_by_service(&self, service: ServiceType) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|peer| peer.services.contains(&service))
            .collect()
    }

    pub fn get_best_oracles(&self, count: usize) -> Vec<&PeerInfo> {
        let mut oracles: Vec<_> = self.get_peers_by_service(ServiceType::Oracle);
        oracles.sort_by(|a, b| b.reputation_score.partial_cmp(&a.reputation_score).unwrap());
        oracles.into_iter().take(count).collect()
    }

    pub fn get_available_liquidators(&self) -> Vec<&PeerInfo> {
        self.get_peers_by_service(ServiceType::Liquidator)
    }

    pub fn register_message_handler<F>(&mut self, message_type: MessageType, handler: F)
    where
        F: Fn(&NetworkMessage) -> Result<()> + 'static,
    {
        self.message_handlers.insert(message_type, Box::new(handler));
    }

    async fn handle_message(&self, message: NetworkMessage) -> Result<()> {
        // Verify message signature if present
        if let Some(_signature) = &message.signature {
            // In a real implementation, verify the signature
        }

        // Route to appropriate handler
        if let Some(handler) = self.message_handlers.get(&message.message_type) {
            handler(&message)?;
        }

        // Update peer info
        if let Some(peer) = self.peers.get(&message.sender) {
            // Update last seen time
        }

        Ok(())
    }
}

// Network statistics and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub oracle_nodes: usize,
    pub liquidator_nodes: usize,
    pub vault_nodes: usize,
    pub network_health_score: f64,
}

impl BitStableNetwork {
    pub fn get_network_stats(&self) -> NetworkStats {
        let oracle_count = self.get_peers_by_service(ServiceType::Oracle).len();
        let liquidator_count = self.get_peers_by_service(ServiceType::Liquidator).len();
        let vault_count = self.get_peers_by_service(ServiceType::VaultProvider).len();
        
        let health_score = self.calculate_network_health();

        NetworkStats {
            connected_peers: self.connection_pool.connections.len(),
            total_messages_sent: 0, // Would track in real implementation
            total_messages_received: 0,
            oracle_nodes: oracle_count,
            liquidator_nodes: liquidator_count,
            vault_nodes: vault_count,
            network_health_score: health_score,
        }
    }

    fn calculate_network_health(&self) -> f64 {
        let min_oracles = 5;
        let min_liquidators = 3;
        let min_peers = 10;

        let oracle_health = (self.get_peers_by_service(ServiceType::Oracle).len() as f64 / min_oracles as f64).min(1.0);
        let liquidator_health = (self.get_peers_by_service(ServiceType::Liquidator).len() as f64 / min_liquidators as f64).min(1.0);
        let peer_health = (self.connection_pool.connections.len() as f64 / min_peers as f64).min(1.0);

        (oracle_health + liquidator_health + peer_health) / 3.0
    }
}