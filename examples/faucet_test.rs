use bitstable::BitcoinClient;

#[tokio::main]
async fn main() -> bitstable::Result<()> {
    env_logger::init();
    
    println!("🚰 Bitcoin Testnet Faucet Automation Test");
    println!("=========================================");
    
    // Connect to Bitcoin testnet
    println!("🌐 Connecting to Bitcoin Core testnet...");
    let bitcoin_client = match BitcoinClient::testnet_with_cookie("http://127.0.0.1:18332") {
        Ok(client) => {
            println!("✅ Connected to Bitcoin testnet node using cookie authentication");
            client
        }
        Err(_) => {
            println!("⚠️ Cookie authentication failed, trying username/password...");
            match BitcoinClient::testnet("http://127.0.0.1:18332", "bitcoin", "password") {
                Ok(client) => {
                    println!("✅ Connected to Bitcoin testnet node using RPC authentication");
                    client
                }
                Err(e) => {
                    println!("❌ Failed to connect to Bitcoin node: {}", e);
                    return Err(e);
                }
            }
        }
    };

    // Generate a test address
    let (test_address, _) = bitcoin_client.generate_testnet_address()?;
    println!("📍 Generated test address: {}", test_address);
    
    // Test automated faucet requests
    println!("\n🤖 Testing automated faucet requests...");
    match bitcoin_client.request_testnet_funds(&test_address).await {
        Ok(txid) => {
            println!("🎉 SUCCESS! Faucet automation worked!");
            println!("   Transaction ID: {}", txid);
            println!("   View on explorer: https://mempool.space/testnet/tx/{}", txid);
            
            // Wait and check for UTXOs
            println!("\n⏳ Waiting 30 seconds for transaction to propagate...");
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            
            println!("🔍 Checking for UTXOs...");
            match bitcoin_client.get_utxos(&test_address) {
                Ok(utxos) => {
                    if utxos.is_empty() {
                        println!("⏰ No UTXOs found yet (transaction may still be propagating)");
                    } else {
                        println!("✅ Found {} UTXOs:", utxos.len());
                        for utxo in utxos {
                            println!("   {}:{} = {} BTC ({} confirmations)", 
                                utxo.txid, utxo.vout, utxo.amount.to_btc(), utxo.confirmations);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Error checking UTXOs: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Automated faucet requests failed: {}", e);
            println!("\n💡 This is expected since most faucets require captcha verification");
            println!("   The demo will fall back to manual instructions for users");
        }
    }
    
    println!("\n✅ Faucet automation test complete!");
    Ok(())
}