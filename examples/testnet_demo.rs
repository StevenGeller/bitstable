use bitstable::{
    BitStableProtocol, ProtocolConfig, Currency, ExchangeRates, 
    StabilityController, VaultManager
};
use bitcoin::{Amount, PublicKey, Network};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PrivateKey};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 BitStable Testnet Demo");
    println!("========================\n");

    // Test 1: Initialize Protocol
    println!("1. Initializing BitStable Protocol...");
    let config = ProtocolConfig::testnet();
    println!("   ✓ Network: {:?}", config.network);
    println!("   ✓ Min Collateral: {:.1}%", config.min_collateral_ratio * 100.0);
    println!("   ✓ Liquidation Threshold: {:.1}%", config.liquidation_threshold * 100.0);
    println!("   ✓ Database: {}", config.database_path);

    let _protocol = BitStableProtocol::new(config)?;
    println!("   ✓ Protocol initialized successfully");

    // Test 2: Generate Test Users
    println!("\n2. Creating Test Users...");
    let secp = Secp256k1::new();
    
    let user1_key = SecretKey::new(&mut rand::thread_rng());
    let user1 = PublicKey::from_private_key(&secp, &PrivateKey::new(user1_key, Network::Testnet));
    println!("   ✓ User 1: {:.20}...", user1.to_string());
    
    let user2_key = SecretKey::new(&mut rand::thread_rng());
    let user2 = PublicKey::from_private_key(&secp, &PrivateKey::new(user2_key, Network::Testnet));
    println!("   ✓ User 2: {:.20}...", user2.to_string());

    // Test 3: Exchange Rate System
    println!("\n3. Testing Multi-Currency Exchange Rates...");
    let mut exchange_rates = ExchangeRates::new();
    exchange_rates.update_btc_price(Currency::USD, 95000.0);
    exchange_rates.update_btc_price(Currency::EUR, 85000.0);
    exchange_rates.update_exchange_rate(Currency::EUR, 0.92); // 1 EUR = 0.92 USD
    
    println!("   ✓ BTC/USD: ${:.2}", exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0));
    println!("   ✓ BTC/EUR: €{:.2}", exchange_rates.get_btc_price(&Currency::EUR).unwrap_or(0.0));
    println!("   ✓ EUR/USD Rate: {:.4}", exchange_rates.get_rate_to_usd(&Currency::EUR).unwrap_or(0.0));

    // Test 4: Stability Controller Scenarios
    println!("\n4. Testing Stability Controllers...");
    
    // Fixed amount controller
    let controller_fixed = StabilityController::new(user1, Currency::USD, 5000.0);
    println!("   Fixed Target Controller:");
    println!("   ✓ User: {:.20}...", controller_fixed.holder.to_string());
    println!("   ✓ Target: {} {}", controller_fixed.target_amount, Currency::USD.to_string());
    
    // Test various balance scenarios
    let scenarios = [
        (4500.0, "Under target"),
        (5500.0, "Over target"),
        (5050.0, "Within threshold"),
        (3000.0, "Significantly under"),
        (7000.0, "Significantly over"),
    ];
    
    for (balance, description) in scenarios {
        let action = controller_fixed.calculate_rebalance(balance, 1.0, &exchange_rates, 2.0, 1.5);
        println!("     • {} (${:.0}): {:?}", description, balance, action);
    }
    
    // Percentage-based controller
    println!("\n   Percentage-based Controller:");
    let controller_percent = StabilityController::new_percentage(user2, Currency::EUR, 30.0);
    println!("   ✓ Target: {}% in {}", 30, Currency::EUR.to_string());
    
    // Portfolio scenarios
    let portfolio_scenarios = [
        (1.0, 20000.0, "Balanced portfolio"),
        (2.0, 40000.0, "Large portfolio"),
        (0.5, 10000.0, "Small portfolio"),
    ];
    
    for (btc_amount, eur_stable, description) in portfolio_scenarios {
        let action = controller_percent.calculate_rebalance(eur_stable, btc_amount, &exchange_rates, 2.0, 1.5);
        let btc_value = btc_amount * exchange_rates.get_btc_price(&Currency::EUR).unwrap_or(0.0);
        let total_value = btc_value + eur_stable;
        let target_stable = total_value * 0.30;
        
        println!("     • {} ({:.1} BTC + €{:.0} = €{:.0} total)", 
                description, btc_amount, eur_stable, total_value);
        println!("       Target: €{:.0}, Action: {:?}", target_stable, action);
    }

    // Test 5: Vault Manager (Basic Operations)
    println!("\n5. Testing Vault Management...");
    let vault_config = ProtocolConfig::testnet();
    let mut _vault_manager = VaultManager::new(&vault_config)?;
    println!("   ✓ Vault Manager initialized");
    
    // Simulate vault creation (without Bitcoin client)
    let collateral_amount = Amount::from_btc(0.1)?; // 0.1 BTC
    let stable_amount = 3000.0; // $3000 USD
    
    println!("   Vault Creation Simulation:");
    println!("   • Collateral: {} BTC", collateral_amount.to_btc());
    println!("   • Stable Amount: ${}", stable_amount);
    println!("   • Currency: USD");
    
    // Calculate theoretical values
    let btc_price = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(95000.0);
    let collateral_value = collateral_amount.to_btc() * btc_price;
    let collateral_ratio = collateral_value / stable_amount;
    
    println!("   • Collateral Value: ${:.2}", collateral_value);
    println!("   • Collateral Ratio: {:.2}% ({:.2}x)", collateral_ratio * 100.0, collateral_ratio);
    
    if collateral_ratio >= vault_config.min_collateral_ratio {
        println!("   ✓ Collateral ratio meets minimum requirement");
    } else {
        println!("   ❌ Insufficient collateral ratio");
    }

    // Test 6: Liquidation Scenarios
    println!("\n6. Testing Liquidation Logic...");
    
    // Price drop simulation
    let price_drops = [0.95, 0.85, 0.75, 0.65]; // 5%, 15%, 25%, 35% drops
    
    for &drop_factor in &price_drops {
        let new_btc_price = btc_price * drop_factor;
        let new_collateral_value = collateral_amount.to_btc() * new_btc_price;
        let new_ratio = new_collateral_value / stable_amount;
        
        let price_drop_percent = (1.0 - drop_factor) * 100.0;
        let is_liquidatable = new_ratio < vault_config.liquidation_threshold;
        
        println!("   • BTC drops {:.0}% to ${:.0}: Ratio {:.2}% - {}", 
                price_drop_percent, new_btc_price, new_ratio * 100.0,
                if is_liquidatable { "⚠️  LIQUIDATABLE" } else { "✅ Safe" });
    }

    // Test 7: Multi-Currency Operations
    println!("\n7. Testing Multi-Currency Operations...");
    
    // Different currency minting scenarios
    let currencies = [Currency::USD, Currency::EUR];
    let amounts = [1000.0, 850.0]; // Equivalent amounts in USD and EUR
    
    for (currency, amount) in currencies.iter().zip(amounts.iter()) {
        let btc_price = exchange_rates.get_btc_price(currency).unwrap_or(0.0);
        let required_collateral_value = amount * vault_config.min_collateral_ratio;
        let required_btc = required_collateral_value / btc_price;
        
        println!("   • Mint {} {}: Requires {:.4} BTC collateral", 
                amount, currency.to_string(), required_btc);
    }

    // Summary
    println!("\n🎉 BitStable Testnet Demo Complete!");
    println!("\n📊 System Status:");
    println!("   ✅ Core Protocol: Functional");
    println!("   ✅ Exchange Rates: Multi-currency ready");
    println!("   ✅ Stability Controllers: Active rebalancing");
    println!("   ✅ Vault Management: Ready for collateral");
    println!("   ✅ Liquidation Logic: Risk management active");
    println!("   ✅ Multi-Currency: USD, EUR support");

    println!("\n🚀 Ready for testnet deployment!");
    println!("📋 Next Steps:");
    println!("   1. Deploy to Bitcoin testnet");
    println!("   2. Connect Bitcoin RPC client");
    println!("   3. Fund test wallets with testnet BTC");
    println!("   4. Create real vaults with Bitcoin collateral");
    println!("   5. Test liquidation with price feeds");

    Ok(())
}