use bitstable::{ProtocolConfig, Currency, ExchangeRates, StabilityController};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PrivateKey, Network, PublicKey};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing BitStable basic functionality...");
    
    // Test configuration
    let config = ProtocolConfig::testnet();
    println!("✓ Configuration created: {:?}", config.network);
    
    // Test exchange rates
    let mut exchange_rates = ExchangeRates::new();
    exchange_rates.update_btc_price(Currency::USD, 100000.0);
    exchange_rates.update_exchange_rate(Currency::EUR, 0.85);
    
    println!("✓ Exchange rates set: BTC/USD = ${}", 
             exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0));
    
    // Test stability controller
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut rand::thread_rng());
    let holder = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
    
    let controller = StabilityController::new(holder, Currency::USD, 1000.0);
    println!("✓ Stability controller created for {} USD", controller.target_amount);
    
    // Test rebalance calculation
    let action = controller.calculate_rebalance(800.0, 1.0, &exchange_rates);
    println!("✓ Rebalance calculation complete: {:?}", action);
    
    println!("\n🎉 Basic functionality test completed successfully!");
    Ok(())
}