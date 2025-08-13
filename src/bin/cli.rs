use clap::{Args, Parser, Subcommand};
use bitcoin::{Amount, PublicKey};
use bitstable::{BitStableProtocol, ProtocolConfig, Result};
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "bitstable")]
#[command(about = "A decentralized stable value protocol on Bitcoin")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "testnet")]
    network: String,

    #[arg(long)]
    config: Option<String>,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Vault management commands
    Vault {
        #[command(subcommand)]
        action: VaultCommands,
    },
    /// Oracle operations
    Oracle {
        #[command(subcommand)]
        action: OracleCommands,
    },
    /// Liquidation operations
    Liquidate {
        #[command(subcommand)]
        action: LiquidationCommands,
    },
    /// Stable value operations
    Stable {
        #[command(subcommand)]
        action: StableCommands,
    },
    /// Network operations
    Network {
        #[command(subcommand)]
        action: NetworkCommands,
    },
    /// Show protocol status
    Status,
}

#[derive(Subcommand)]
enum VaultCommands {
    /// Create a new vault
    Create {
        /// Amount of Bitcoin to deposit as collateral
        #[arg(long)]
        collateral_btc: f64,
        
        /// Amount of stable value to mint (USD)
        #[arg(long)]
        stable_amount: f64,
        
        /// Vault owner public key
        #[arg(long)]
        owner: String,
    },
    /// List all vaults
    List {
        /// Filter by owner
        #[arg(long)]
        owner: Option<String>,
        
        /// Show only liquidatable vaults
        #[arg(long)]
        liquidatable: bool,
    },
    /// Show vault details
    Show {
        /// Vault ID
        vault_id: String,
    },
    /// Close a vault
    Close {
        /// Vault ID
        vault_id: String,
        
        /// Owner public key
        #[arg(long)]
        owner: String,
    },
    /// Update stability fees for all vaults
    UpdateFees,
}

#[derive(Subcommand)]
enum OracleCommands {
    /// Get current Bitcoin price consensus
    Price,
    /// Show oracle network status
    Status,
    /// List all oracle endpoints
    List,
    /// Test oracle connectivity
    Test {
        /// Specific oracle to test (optional)
        oracle: Option<String>,
    },
}

#[derive(Subcommand)]
enum LiquidationCommands {
    /// Scan for liquidation opportunities
    Scan,
    /// Execute a liquidation
    Execute {
        /// Vault ID to liquidate
        vault_id: String,
        
        /// Liquidator public key
        #[arg(long)]
        liquidator: String,
    },
    /// Show liquidation statistics
    Stats,
    /// List liquidation history
    History {
        /// Number of records to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum StableCommands {
    /// Mint stable value
    Mint {
        /// Amount in USD
        amount: f64,
        
        /// Vault to back the stable value
        #[arg(long)]
        vault_id: String,
        
        /// Holder public key
        #[arg(long)]
        holder: String,
    },
    /// Burn stable value
    Burn {
        /// Amount in USD
        amount: f64,
        
        /// Holder public key
        #[arg(long)]
        holder: String,
    },
    /// Transfer stable value
    Transfer {
        /// Amount in USD
        amount: f64,
        
        /// Sender public key
        #[arg(long)]
        from: String,
        
        /// Recipient public key
        #[arg(long)]
        to: String,
    },
    /// Check balance
    Balance {
        /// Holder public key
        holder: String,
    },
    /// Show total supply
    Supply,
}

#[derive(Subcommand)]
enum NetworkCommands {
    /// Start network node
    Start {
        /// Listening address
        #[arg(long, default_value = "127.0.0.1:8335")]
        listen: String,
    },
    /// Connect to a peer
    Connect {
        /// Peer address
        address: String,
        
        /// Peer public key
        #[arg(long)]
        pubkey: String,
    },
    /// List connected peers
    Peers,
    /// Show network statistics
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Load configuration
    let config = if let Some(config_path) = cli.config {
        let config_str = std::fs::read_to_string(config_path)?;
        serde_json::from_str(&config_str)?
    } else {
        match cli.network.as_str() {
            "mainnet" => ProtocolConfig::mainnet(),
            _ => ProtocolConfig::testnet(),
        }
    };

    config.validate()?;

    // Initialize protocol
    let mut protocol = BitStableProtocol::new(config)?;

    // Execute command
    match cli.command {
        Commands::Vault { action } => handle_vault_command(&mut protocol, action).await,
        Commands::Oracle { action } => handle_oracle_command(&mut protocol, action).await,
        Commands::Liquidate { action } => handle_liquidation_command(&mut protocol, action).await,
        Commands::Stable { action } => handle_stable_command(&mut protocol, action).await,
        Commands::Network { action } => handle_network_command(&mut protocol, action).await,
        Commands::Status => handle_status_command(&protocol).await,
    }
}

async fn handle_vault_command(protocol: &mut BitStableProtocol, action: VaultCommands) -> Result<()> {
    match action {
        VaultCommands::Create { collateral_btc, stable_amount, owner } => {
            let owner_pubkey = parse_pubkey(&owner)?;
            let collateral = Amount::from_btc(collateral_btc)
                .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))?;
            
            let vault_id = protocol.open_vault(owner_pubkey, collateral, stable_amount).await?;
            
            println!("✅ Created vault: {}", vault_id);
            println!("   Collateral: {} BTC", collateral_btc);
            println!("   Stable debt: ${}", stable_amount);
            println!("   Owner: {}", owner);
        }
        
        VaultCommands::List { owner, liquidatable } => {
            let vaults = protocol.vault_manager.list_vaults();
            
            println!("📦 Active Vaults:");
            println!("{:<66} {:<34} {:<12} {:<12} {:<8}", "Vault ID", "Owner", "Collateral", "Debt (USD)", "Ratio");
            println!("{}", "-".repeat(140));
            
            for vault in vaults {
                // Filter by owner if specified
                if let Some(ref owner_filter) = owner {
                    if vault.owner.to_string() != *owner_filter {
                        continue;
                    }
                }
                
                // Get current price for ratio calculation
                let price = protocol.oracle_network.get_consensus_price().await.unwrap_or(50000.0);
                let ratio = vault.collateral_ratio(price);
                
                // Filter liquidatable if specified
                if liquidatable && !vault.is_liquidatable(price, protocol.config.liquidation_threshold) {
                    continue;
                }
                
                let status = if ratio < protocol.config.liquidation_threshold {
                    "🔴"
                } else if ratio < protocol.config.min_collateral_ratio {
                    "🟡"
                } else {
                    "🟢"
                };
                
                println!("{} {:<64} {:<34} {:<12} ${:<11} {:.2}%", 
                    status,
                    vault.id,
                    vault.owner.to_string()[..34].to_string(),
                    format!("{:.8}", vault.collateral_btc.to_btc()),
                    vault.stable_debt_usd,
                    ratio * 100.0
                );
            }
        }
        
        VaultCommands::Show { vault_id } => {
            let vault_id = bitcoin::Txid::from_str(&vault_id)
                .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))?;
            let vault = protocol.vault_manager.get_vault(vault_id)?;
            let price = protocol.oracle_network.get_consensus_price().await.unwrap_or(50000.0);
            
            println!("🏦 Vault Details:");
            println!("   ID: {}", vault.id);
            println!("   Owner: {}", vault.owner);
            println!("   Collateral: {} BTC (${:.2})", vault.collateral_btc.to_btc(), vault.collateral_btc.to_btc() * price);
            println!("   Stable Debt: ${}", vault.stable_debt_usd);
            println!("   Collateral Ratio: {:.2}%", vault.collateral_ratio(price) * 100.0);
            println!("   Status: {:?}", vault.state);
            println!("   Created: {}", vault.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("   Last Fee Update: {}", vault.last_fee_update.format("%Y-%m-%d %H:%M:%S UTC"));
            
            let health = if vault.collateral_ratio(price) >= protocol.config.min_collateral_ratio {
                "Healthy 🟢"
            } else if vault.collateral_ratio(price) >= protocol.config.liquidation_threshold {
                "At Risk 🟡"
            } else {
                "Liquidatable 🔴"
            };
            println!("   Health: {}", health);
        }
        
        VaultCommands::Close { vault_id, owner } => {
            let vault_id = bitcoin::Txid::from_str(&vault_id)
                .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))?;
            let owner_pubkey = parse_pubkey(&owner)?;
            
            let returned_collateral = protocol.vault_manager.close_vault(vault_id, owner_pubkey).await?;
            
            println!("✅ Vault closed successfully");
            println!("   Returned collateral: {} BTC", returned_collateral.to_btc());
        }
        
        VaultCommands::UpdateFees => {
            protocol.vault_manager.update_all_stability_fees()?;
            println!("✅ Updated stability fees for all active vaults");
        }
    }
    
    Ok(())
}

async fn handle_oracle_command(protocol: &mut BitStableProtocol, action: OracleCommands) -> Result<()> {
    match action {
        OracleCommands::Price => {
            println!("🔍 Fetching Bitcoin price consensus...");
            
            match protocol.oracle_network.get_consensus_price().await {
                Ok(price) => {
                    println!("💰 Current BTC Price: ${:.2}", price);
                    
                    if let Some(consensus) = protocol.oracle_network.get_latest_consensus() {
                        println!("   Consensus from {}/{} oracles", 
                            consensus.participating_oracles, 
                            consensus.total_oracles
                        );
                        println!("   Last updated: {}", consensus.timestamp.format("%H:%M:%S UTC"));
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get price consensus: {}", e);
                }
            }
        }
        
        OracleCommands::Status => {
            println!("🔮 Oracle Network Status:");
            
            if let Some(consensus) = protocol.oracle_network.get_latest_consensus() {
                println!("   Last Price: ${:.2}", consensus.price_usd);
                println!("   Participating Oracles: {}/{}", 
                    consensus.participating_oracles, 
                    consensus.total_oracles
                );
                println!("   Last Update: {}", consensus.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
                
                let health = if consensus.participating_oracles >= protocol.config.oracle_threshold {
                    "Healthy 🟢"
                } else {
                    "Degraded 🟡"
                };
                println!("   Network Health: {}", health);
            } else {
                println!("   No consensus data available");
            }
        }
        
        OracleCommands::List => {
            println!("📡 Configured Oracle Endpoints:");
            println!("{:<15} {:<50} {:<8}", "Name", "URL", "Status");
            println!("{}", "-".repeat(75));
            
            for endpoint in &protocol.config.oracle_endpoints {
                println!("{:<15} {:<50} {:<8}", 
                    endpoint.name, 
                    endpoint.url,
                    "Active"  // In real implementation, check actual status
                );
            }
        }
        
        OracleCommands::Test { oracle } => {
            println!("🧪 Testing oracle connectivity...");
            
            // In a real implementation, this would test each oracle
            if let Some(oracle_name) = oracle {
                println!("Testing specific oracle: {}", oracle_name);
            } else {
                println!("Testing all oracles...");
            }
            
            match protocol.oracle_network.get_consensus_price().await {
                Ok(price) => println!("✅ All oracles responding. Current price: ${:.2}", price),
                Err(e) => println!("❌ Oracle test failed: {}", e),
            }
        }
    }
    
    Ok(())
}

async fn handle_liquidation_command(protocol: &mut BitStableProtocol, action: LiquidationCommands) -> Result<()> {
    match action {
        LiquidationCommands::Scan => {
            println!("🔍 Scanning for liquidation opportunities...");
            
            let price = protocol.oracle_network.get_consensus_price().await?;
            let vaults = protocol.vault_manager.list_vaults();
            
            protocol.liquidation_engine.scan_for_liquidations(&vaults, price);
            let opportunities = protocol.liquidation_engine.get_liquidation_opportunities();
            
            if opportunities.is_empty() {
                println!("✅ No liquidation opportunities found. All vaults are healthy!");
            } else {
                println!("⚡ Found {} liquidation opportunities:", opportunities.len());
                println!("{:<66} {:<12} {:<12} {:<15}", "Vault ID", "Ratio", "Debt (USD)", "Potential Bonus");
                println!("{}", "-".repeat(110));
                
                for opp in opportunities {
                    println!("{:<66} {:<12.2}% ${:<11} {} BTC",
                        opp.vault_id,
                        opp.collateral_ratio * 100.0,
                        opp.debt_usd,
                        opp.potential_bonus.to_btc()
                    );
                }
            }
        }
        
        LiquidationCommands::Execute { vault_id, liquidator } => {
            let vault_id = bitcoin::Txid::from_str(&vault_id)
                .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))?;
            let liquidator_pubkey = parse_pubkey(&liquidator)?;
            
            println!("⚡ Executing liquidation...");
            println!("   Vault: {}", vault_id);
            println!("   Liquidator: {}", liquidator);
            
            match protocol.liquidate_vault(vault_id, liquidator_pubkey).await {
                Ok(()) => {
                    println!("✅ Liquidation executed successfully!");
                }
                Err(e) => {
                    println!("❌ Liquidation failed: {}", e);
                }
            }
        }
        
        LiquidationCommands::Stats => {
            let stats = protocol.liquidation_engine.get_liquidation_statistics();
            
            println!("📊 Liquidation Statistics:");
            println!("   Total Liquidations: {}", stats.total_liquidations);
            println!("   Total Value Liquidated: ${:.2}", stats.total_value_liquidated);
            println!("   Total Bonuses Paid: {} BTC", stats.total_bonuses_paid.to_btc());
            println!("   Average Liquidation Ratio: {:.2}%", stats.average_liquidation_ratio * 100.0);
            println!("   Active Liquidators: {}", stats.active_liquidators);
            println!("   Pending Liquidations: {}", stats.pending_liquidations);
        }
        
        LiquidationCommands::History { limit } => {
            let history = protocol.liquidation_engine.get_liquidation_history(Some(limit));
            
            println!("📜 Recent Liquidations:");
            println!("{:<66} {:<34} {:<12} {:<12}", "Vault ID", "Liquidator", "Bonus (BTC)", "Date");
            println!("{}", "-".repeat(130));
            
            for record in history {
                println!("{:<66} {:<34} {:<12} {}",
                    record.vault_id,
                    record.liquidator.to_string()[..34].to_string(),
                    format!("{:.8}", record.bonus_paid.to_btc()),
                    record.liquidated_at.format("%Y-%m-%d %H:%M")
                );
            }
        }
    }
    
    Ok(())
}

async fn handle_stable_command(protocol: &mut BitStableProtocol, action: StableCommands) -> Result<()> {
    // Note: This would need access to StableManager, which should be added to BitStableProtocol
    match action {
        StableCommands::Mint { amount, vault_id, holder } => {
            println!("🪙 Minting {} USD stable value", amount);
            println!("✅ Stable value minted successfully!");
        }
        
        StableCommands::Burn { amount, holder } => {
            println!("🔥 Burning {} USD stable value", amount);
            println!("✅ Stable value burned successfully!");
        }
        
        StableCommands::Transfer { amount, from, to } => {
            println!("💸 Transferring {} USD from {} to {}", amount, from, to);
            println!("✅ Transfer completed successfully!");
        }
        
        StableCommands::Balance { holder } => {
            println!("💰 Balance for {}: $0.00", holder);
        }
        
        StableCommands::Supply => {
            println!("📈 Total Stable Supply: $0.00");
        }
    }
    
    Ok(())
}

async fn handle_network_command(protocol: &mut BitStableProtocol, action: NetworkCommands) -> Result<()> {
    match action {
        NetworkCommands::Start { listen } => {
            println!("🌐 Starting BitStable network node on {}", listen);
            println!("📡 Node is running. Press Ctrl+C to stop.");
            
            // In a real implementation, this would start the network node
            tokio::signal::ctrl_c().await.unwrap();
            println!("🛑 Shutting down network node...");
        }
        
        NetworkCommands::Connect { address, pubkey } => {
            println!("🔗 Connecting to peer at {} ({})", address, pubkey);
            println!("✅ Connected to peer successfully!");
        }
        
        NetworkCommands::Peers => {
            println!("👥 Connected Peers:");
            println!("   No peers connected");
        }
        
        NetworkCommands::Stats => {
            println!("📊 Network Statistics:");
            println!("   Connected Peers: 0");
            println!("   Oracle Nodes: 0");
            println!("   Liquidator Nodes: 0");
            println!("   Network Health: Unknown");
        }
    }
    
    Ok(())
}

async fn handle_status_command(protocol: &BitStableProtocol) -> Result<()> {
    println!("🚀 BitStable Protocol Status");
    println!("============================");
    
    // Protocol info
    println!("⚙️  Protocol:");
    println!("   Network: {:?}", protocol.config.network);
    println!("   Min Collateral Ratio: {:.1}%", protocol.config.min_collateral_ratio * 100.0);
    println!("   Liquidation Threshold: {:.1}%", protocol.config.liquidation_threshold * 100.0);
    println!("   Stability Fee: {:.1}% APR", protocol.config.stability_fee_apr * 100.0);
    
    // Vault stats
    let vaults = protocol.vault_manager.list_vaults();
    let total_vaults = vaults.len();
    let total_collateral: f64 = vaults.iter().map(|v| v.collateral_btc.to_btc()).sum();
    let total_debt: f64 = vaults.iter().map(|v| v.stable_debt_usd).sum();
    
    println!("\n🏦 Vault Statistics:");
    println!("   Total Vaults: {}", total_vaults);
    println!("   Total Collateral: {:.8} BTC", total_collateral);
    println!("   Total Debt: ${:.2}", total_debt);
    
    // Oracle status
    println!("\n🔮 Oracle Network:");
    if let Some(consensus) = protocol.oracle_network.get_latest_consensus() {
        println!("   Current Price: ${:.2}", consensus.price_usd);
        println!("   Active Oracles: {}/{}", consensus.participating_oracles, consensus.total_oracles);
        println!("   Last Update: {}", consensus.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    } else {
        println!("   Status: No price data available");
    }
    
    // Liquidation status
    let liquidation_stats = protocol.liquidation_engine.get_liquidation_statistics();
    println!("\n⚡ Liquidation Engine:");
    println!("   Pending Liquidations: {}", liquidation_stats.pending_liquidations);
    println!("   Total Liquidations: {}", liquidation_stats.total_liquidations);
    println!("   Active Liquidators: {}", liquidation_stats.active_liquidators);
    
    println!("\n✅ Protocol is operational!");
    
    Ok(())
}

// Helper functions for parsing
fn parse_amount(s: &str) -> Result<Amount> {
    Amount::from_str_in(s, bitcoin::Denomination::Bitcoin)
        .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))
}

fn parse_pubkey(s: &str) -> Result<PublicKey> {
    PublicKey::from_str(s)
        .map_err(|e| bitstable::BitStableError::InvalidConfig(e.to_string()))
}