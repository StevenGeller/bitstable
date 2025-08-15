// Simple example showing how to use automated Bitcoin testnet faucets
use bitstable::BitcoinClient;

#[tokio::main]
async fn main() -> bitstable::Result<()> {
    // Connect to Bitcoin testnet node
    let client = BitcoinClient::testnet_with_cookie("http://127.0.0.1:18332")?;
    
    // Generate an address
    let (address, _private_key) = client.generate_testnet_address()?;
    println!("Address: {}", address);
    
    // Automatically request testnet funds
    println!("Requesting testnet funds...");
    match client.request_testnet_funds(&address).await {
        Ok(txid) => {
            println!("Success! Transaction: {}", txid);
            println!("View at: https://mempool.space/testnet/tx/{}", txid);
        }
        Err(e) => {
            println!("Automated request failed: {}", e);
            println!("Please visit https://coinfaucet.eu/en/btc-testnet/ manually");
        }
    }
    
    Ok(())
}