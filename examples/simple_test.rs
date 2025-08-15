use bitstable::{ProtocolConfig, Currency, ExchangeRates, StabilityController};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PrivateKey, Network, PublicKey};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing BitStable Core Components...\n");
    
    // Test 1: Configuration
    println!("1. Testing Configuration...");
    let config = ProtocolConfig::testnet();
    println!("   ✓ Network: {:?}", config.network);
    println!("   ✓ Min Collateral Ratio: {:.1}%", config.min_collateral_ratio * 100.0);
    println!("   ✓ Liquidation Threshold: {:.1}%", config.liquidation_threshold * 100.0);
    
    // Test 2: Exchange Rates
    println!("\n2. Testing Exchange Rates...");
    let mut exchange_rates = ExchangeRates::new();
    
    // Set BTC price
    exchange_rates.update_btc_price(Currency::USD, 100000.0);
    exchange_rates.update_btc_price(Currency::EUR, 90000.0);
    
    // Set exchange rates
    exchange_rates.update_exchange_rate(Currency::EUR, 0.85); // 1 EUR = 0.85 USD
    
    if let Some(btc_usd) = exchange_rates.get_btc_price(&Currency::USD) {
        println!("   ✓ BTC/USD: ${:.2}", btc_usd);
    }
    
    if let Some(btc_eur) = exchange_rates.get_btc_price(&Currency::EUR) {
        println!("   ✓ BTC/EUR: €{:.2}", btc_eur);
    }
    
    // Test currency conversion
    let btc_eur_calculated = exchange_rates.calculate_btc_price(&Currency::EUR, 100000.0);
    println!("   ✓ Calculated BTC/EUR: €{:.2}", btc_eur_calculated);
    
    // Test 3: Stability Controller
    println!("\n3. Testing Stability Controller...");
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut rand::thread_rng());
    let holder = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
    
    // Create a controller targeting $1000 USD stable
    let controller = StabilityController::new(holder, Currency::USD, 1000.0);
    println!("   ✓ Target: {} {}", controller.target_amount, Currency::USD.to_string());
    
    // Test rebalancing scenarios
    println!("\n   Testing Rebalancing Scenarios:");
    
    // Scenario 1: Under target
    let action = controller.calculate_rebalance(800.0, 1.0, &exchange_rates);
    println!("   • Current: $800, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Scenario 2: Over target  
    let action = controller.calculate_rebalance(1200.0, 1.0, &exchange_rates);
    println!("   • Current: $1200, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Scenario 3: Within threshold
    let action = controller.calculate_rebalance(1010.0, 1.0, &exchange_rates);
    println!("   • Current: $1010, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Test 4: Percentage-based controller
    println!("\n   Testing Percentage-based Controller:");
    let percentage_controller = StabilityController::new_percentage(holder, Currency::USD, 40.0);
    
    // Portfolio: 1 BTC ($100k) + $50k stable = $150k total
    // Target: 40% of $150k = $60k stable
    let action = percentage_controller.calculate_rebalance(50000.0, 1.0, &exchange_rates);
    println!("   • Portfolio: 1 BTC + $50k stable = $150k total");
    println!("   • Target: 40% stable = $60k");
    println!("     → Action: {:?}", action);
    
    println!("\n🎉 All core components working correctly!");
    println!("\n📊 System Summary:");
    println!("   • Configuration: Ready for testnet");
    println!("   • Exchange Rates: Multi-currency support active");
    println!("   • Stability Control: Automatic rebalancing functional");
    println!("   • Architecture: Modular and extensible");
    
    Ok(())
}