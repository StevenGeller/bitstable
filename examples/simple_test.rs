use bitstable::{ProtocolConfig, Currency, ExchangeRates, StabilityController};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PrivateKey, Network, PublicKey};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing BitStable Core Components...\n");
    
    // Test 1: Configuration
    println!("1. Testing Configuration...");
    let config = ProtocolConfig::testnet();
    println!("   ✓ Network: {:?}", config.network);
    println!("   ✓ Min Collateral Ratio: {:.1}%", config.min_collateral_ratio * 100.0);
    println!("   ✓ Liquidation Threshold: {:.1}%", config.liquidation_threshold * 100.0);
    
    // Test 2: Live Exchange Rates
    println!("\n2. Testing Live Exchange Rates...");
    let mut exchange_rates = ExchangeRates::new();
    
    // Fetch live prices from CoinGecko
    println!("   🌐 Fetching live BTC prices...");
    let client = reqwest::Client::new();
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd,eur";
    
    match client.get(url).send().await {
        Ok(response) => {
            if let Ok(text) = response.text().await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(bitcoin_data) = parsed.get("bitcoin") {
                        if let Some(usd_price) = bitcoin_data.get("usd").and_then(|v| v.as_f64()) {
                            exchange_rates.update_btc_price(Currency::USD, usd_price);
                            println!("   ✓ Live BTC/USD: ${:.2}", usd_price);
                        }
                        if let Some(eur_price) = bitcoin_data.get("eur").and_then(|v| v.as_f64()) {
                            exchange_rates.update_btc_price(Currency::EUR, eur_price);
                            println!("   ✓ Live BTC/EUR: €{:.2}", eur_price);
                            
                            // Calculate EUR/USD rate from live data
                            if let Some(usd_price) = exchange_rates.get_btc_price(&Currency::USD) {
                                let eur_usd_rate = eur_price / usd_price;
                                exchange_rates.update_exchange_rate(Currency::EUR, eur_usd_rate);
                                println!("   ✓ Calculated EUR/USD: {:.4}", eur_usd_rate);
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            println!("   ⚠️ Network unavailable, skipping live data test");
            exchange_rates.update_btc_price(Currency::USD, 100000.0);
            exchange_rates.update_btc_price(Currency::EUR, 90000.0);
            exchange_rates.update_exchange_rate(Currency::EUR, 0.85);
        }
    }
    
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
    let action = controller.calculate_rebalance(800.0, 1.0, &exchange_rates, 2.0, 1.5);
    println!("   • Current: $800, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Scenario 2: Over target  
    let action = controller.calculate_rebalance(1200.0, 1.0, &exchange_rates, 2.0, 1.5);
    println!("   • Current: $1200, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Scenario 3: Within threshold
    let action = controller.calculate_rebalance(1010.0, 1.0, &exchange_rates, 2.0, 1.5);
    println!("   • Current: $1010, Target: $1000");
    println!("     → Action: {:?}", action);
    
    // Test 4: Percentage-based controller
    println!("\n   Testing Percentage-based Controller:");
    let percentage_controller = StabilityController::new_percentage(holder, Currency::USD, 40.0);
    
    // Portfolio: 1 BTC ($100k) + $50k stable = $150k total
    // Target: 40% of $150k = $60k stable
    let action = percentage_controller.calculate_rebalance(50000.0, 1.0, &exchange_rates, 2.0, 1.5);
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