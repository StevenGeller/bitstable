use clap::Parser;
use bitstable::{ProtocolConfig, Result, oracle::OracleNetwork};
use tokio::time::{sleep, Duration};

#[derive(Parser)]
#[command(name = "oracle-node")]
#[command(about = "BitStable Oracle Node - Provides price feeds to the network")]
#[command(version = "0.1.0")]
struct Cli {
    #[arg(long, default_value = "testnet")]
    network: String,

    #[arg(long)]
    config: Option<String>,

    #[arg(long, default_value = "127.0.0.1:8336")]
    listen: String,

    #[arg(long)]
    oracle_key: Option<String>,

    #[arg(long, default_value = "30")]
    update_interval: u64,

    #[arg(short, long)]
    verbose: bool,
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

    println!("🔮 BitStable Oracle Node Starting");
    println!("=================================");
    println!("Network: {:?}", config.network);
    println!("Listening on: {}", cli.listen);
    println!("Update interval: {}s", cli.update_interval);

    // Initialize oracle network
    let mut oracle_network = OracleNetwork::new(&config)?;

    println!("📡 Configured {} oracle endpoints", config.oracle_endpoints.len());
    for endpoint in &config.oracle_endpoints {
        println!("   - {}: {}", endpoint.name, endpoint.url);
    }

    println!("\n🚀 Oracle node is running...");
    println!("Press Ctrl+C to stop\n");

    // Main oracle loop
    let mut update_counter = 0u64;
    
    loop {
        tokio::select! {
            // Periodic price updates
            _ = sleep(Duration::from_secs(cli.update_interval)) => {
                update_counter += 1;
                
                match oracle_network.get_consensus_price().await {
                    Ok(price) => {
                        log::info!("Price update #{}: ${:.2}", update_counter, price);
                        
                        if let Some(consensus) = oracle_network.get_latest_consensus() {
                            println!("💰 Update #{}: ${:.2} (from {}/{} oracles) at {}", 
                                update_counter,
                                price, 
                                consensus.participating_oracles,
                                consensus.total_oracles,
                                consensus.timestamp.format("%H:%M:%S")
                            );
                        }
                        
                        // In a real implementation, broadcast this price to the network
                        // network.broadcast_price_update(price, "local_oracle", signature).await?;
                    }
                    Err(e) => {
                        log::error!("Failed to get price consensus: {}", e);
                        println!("❌ Price update #{} failed: {}", update_counter, e);
                    }
                }
            }
            
            // Handle shutdown signal
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Received shutdown signal");
                break;
            }
        }
    }

    println!("📊 Oracle Statistics:");
    if let Some(latest) = oracle_network.get_latest_consensus() {
        println!("   Last Price: ${:.2}", latest.price_usd);
        println!("   Total Updates: {}", update_counter);
        println!("   Final Oracle Participation: {}/{}", 
            latest.participating_oracles, 
            latest.total_oracles
        );
    }
    
    println!("🔮 Oracle node stopped");
    Ok(())
}