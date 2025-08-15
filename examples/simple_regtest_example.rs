// Simple example showing automated Bitcoin regtest operations
use bitstable::BitcoinClient;
use bitcoin::Amount;

#[tokio::main]
async fn main() -> bitstable::Result<()> {
    println!("🤖 Simple Bitcoin Regtest Automation");
    println!("====================================");
    
    // Connect to regtest
    println!("🔗 Connecting to Bitcoin regtest...");
    let client = BitcoinClient::regtest("http://127.0.0.1:18443", "bitstable", "password")?;
    println!("✅ Connected!");
    
    // Generate an address
    let (address, _private_key) = client.generate_address()?;
    println!("📍 Generated address: {}", address);
    
    // Automatically generate funds by mining
    println!("⛏️  Mining blocks to generate funds...");
    let generated = client.generate_regtest_funds(&address, 1.0).await?;
    println!("💰 Generated {} BTC!", generated.to_btc());
    
    // Check balance
    let utxos = client.get_utxos(&address)?;
    let balance: u64 = utxos.iter().map(|u| u.amount.to_sat()).sum();
    println!("💰 Final balance: {} BTC in {} UTXOs", 
        Amount::from_sat(balance).to_btc(), utxos.len());
    
    // Show network stats
    let stats = client.get_blockchain_info()?;
    println!("📊 Network: {} blocks, difficulty: {:.2e}", 
        stats.block_height, stats.difficulty);
    
    println!("🎉 Regtest automation complete!");
    Ok(())
}