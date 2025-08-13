use clap::Parser;
use bitcoin::{PublicKey, Amount};
use bitstable::{BitStableProtocol, ProtocolConfig, Result, liquidation::LiquidatorBot};
use std::str::FromStr;
use tokio::time::{sleep, Duration};

#[derive(Parser)]
#[command(name = "liquidator-bot")]
#[command(about = "BitStable Liquidator Bot - Automatically liquidates unhealthy vaults")]
#[command(version = "0.1.0")]
struct Cli {
    #[arg(long, default_value = "testnet")]
    network: String,

    #[arg(long)]
    config: Option<String>,

    #[arg(long)]
    liquidator_key: String,

    #[arg(long, default_value = "0.001")]
    min_profit_btc: f64,

    #[arg(long, default_value = "0.0001")]
    max_gas_btc: f64,

    #[arg(long, default_value = "30")]
    scan_interval: u64,

    #[arg(long, default_value = "3")]
    max_liquidations_per_round: usize,

    #[arg(long)]
    dry_run: bool,

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

    // Parse liquidator key
    let liquidator_pubkey = PublicKey::from_str(&cli.liquidator_key)
        .map_err(|e| bitstable::BitStableError::PublicKeyParseError(e.to_string()))?;
    let min_profit = Amount::from_btc(cli.min_profit_btc)?;
    let max_gas = Amount::from_btc(cli.max_gas_btc)?;

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

    println!("⚡ BitStable Liquidator Bot Starting");
    println!("===================================");
    println!("Network: {:?}", config.network);
    println!("Liquidator: {}", liquidator_pubkey);
    println!("Min Profit: {} BTC", min_profit.to_btc());
    println!("Max Gas Cost: {} BTC", max_gas.to_btc());
    println!("Scan Interval: {}s", cli.scan_interval);
    
    if cli.dry_run {
        println!("🔬 DRY RUN MODE - No actual liquidations will be executed");
    }

    // Initialize protocol and liquidator bot
    let mut protocol = BitStableProtocol::new(config)?;
    let liquidator_bot = LiquidatorBot::new(liquidator_pubkey, min_profit, max_gas);

    println!("\n🤖 Liquidator bot is running...");
    println!("Press Ctrl+C to stop\n");

    // Main liquidation loop
    let mut scan_counter = 0u64;
    let mut total_liquidations = 0u64;
    let mut total_profit = Amount::ZERO;
    
    loop {
        tokio::select! {
            // Periodic liquidation scans
            _ = sleep(Duration::from_secs(cli.scan_interval)) => {
                scan_counter += 1;
                
                match run_liquidation_scan(&mut protocol, &liquidator_bot, cli.max_liquidations_per_round, cli.dry_run).await {
                    Ok(results) => {
                        if results.liquidations > 0 {
                            total_liquidations += results.liquidations as u64;
                            total_profit += results.profit;
                            
                            println!("⚡ Scan #{}: {} liquidations executed, profit: {} BTC", 
                                scan_counter, results.liquidations, results.profit.to_btc());
                        } else {
                            println!("✅ Scan #{}: No liquidation opportunities", scan_counter);
                        }
                    }
                    Err(e) => {
                        log::error!("Liquidation scan failed: {}", e);
                        println!("❌ Scan #{} failed: {}", scan_counter, e);
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

    println!("📊 Liquidator Statistics:");
    println!("   Total Scans: {}", scan_counter);
    println!("   Total Liquidations: {}", total_liquidations);
    println!("   Total Profit: {} BTC", total_profit.to_btc());
    
    if total_liquidations > 0 {
        println!("   Average Profit per Liquidation: {} BTC", 
            (total_profit.to_btc() / total_liquidations as f64));
    }
    
    println!("⚡ Liquidator bot stopped");
    Ok(())
}

struct LiquidationResults {
    liquidations: usize,
    profit: Amount,
}

async fn run_liquidation_scan(
    protocol: &mut BitStableProtocol,
    bot: &LiquidatorBot,
    max_liquidations: usize,
    dry_run: bool,
) -> Result<LiquidationResults> {
    // Get current price
    let price = protocol.oracle_network.get_consensus_price().await?;
    
    // Scan for liquidation opportunities
    let vaults = protocol.vault_manager.list_vaults();
    protocol.liquidation_engine.scan_for_liquidations(&vaults, price);
    
    let opportunities = protocol.liquidation_engine.get_liquidation_opportunities();
    
    if opportunities.is_empty() {
        return Ok(LiquidationResults {
            liquidations: 0,
            profit: Amount::ZERO,
        });
    }

    // Collect liquidation data before borrowing protocol mutably
    let liquidation_data: Vec<_> = opportunities.iter()
        .filter_map(|opp| {
            if bot.should_liquidate(opp) {
                let expected_profit = if opp.potential_bonus > bot.max_gas_price {
                    opp.potential_bonus - bot.max_gas_price
                } else {
                    Amount::ZERO
                };
                Some((opp.vault_id, opp.collateral_ratio, expected_profit))
            } else {
                None
            }
        })
        .take(max_liquidations)
        .collect();
    
    if liquidation_data.is_empty() {
        log::debug!("No profitable liquidation opportunities found");
        return Ok(LiquidationResults {
            liquidations: 0,
            profit: Amount::ZERO,
        });
    }

    println!("🎯 Found {} profitable liquidation opportunities:", liquidation_data.len());
    
    let mut total_profit = Amount::ZERO;
    let mut successful_liquidations = 0;

    for (vault_id, collateral_ratio, expected_profit) in liquidation_data {
        println!("   Vault {}: Ratio {:.2}%, Expected profit: {} BTC", 
            vault_id.to_string()[..8].to_string(),
            collateral_ratio * 100.0,
            expected_profit.to_btc()
        );
        
        if !dry_run {
            match protocol.liquidate_vault(vault_id, bot.liquidator_key).await {
                Ok(()) => {
                    successful_liquidations += 1;
                    total_profit += expected_profit;
                    log::info!("Successfully liquidated vault {}", vault_id);
                }
                Err(e) => {
                    log::error!("Failed to liquidate vault {}: {}", vault_id, e);
                    println!("   ❌ Failed to liquidate vault {}: {}", vault_id, e);
                }
            }
        } else {
            // Dry run - just simulate
            successful_liquidations += 1;
            total_profit += expected_profit;
            println!("   ✅ [DRY RUN] Would liquidate vault {}", vault_id);
        }
    }

    Ok(LiquidationResults {
        liquidations: successful_liquidations,
        profit: total_profit,
    })
}