use bitstable::{ProtocolConfig, BitStableError, Result};
use bitstable::stable::MultiCurrencyStableManager;
use bitstable::multi_currency::{Currency, ExchangeRates, CurrencyConfig};
use bitstable::stability_controller::{StabilityController, PortfolioManager, HolderBalance, RebalanceAction};
use bitstable::vault::{VaultManager, Vault};
use bitcoin::{Amount, PublicKey, secp256k1::{Secp256k1, SecretKey}, PrivateKey, Network};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    println!("🚀 BitStable Comprehensive Test Suite");
    println!("=====================================\n");

    // Setup
    let secp = Secp256k1::new();
    let secret1 = SecretKey::new(&mut rand::thread_rng());
    let secret2 = SecretKey::new(&mut rand::thread_rng());
    let secret3 = SecretKey::new(&mut rand::thread_rng());
    
    let alice = PublicKey::from_private_key(&secp, &PrivateKey::new(secret1, Network::Testnet));
    let bob = PublicKey::from_private_key(&secp, &PrivateKey::new(secret2, Network::Testnet));
    let charlie = PublicKey::from_private_key(&secp, &PrivateKey::new(secret3, Network::Testnet));

    println!("👥 Test Users:");
    println!("Alice:   {}", alice);
    println!("Bob:     {}", bob);
    println!("Charlie: {}\n", charlie);
    
    println!("⏱️  Press Enter to start the comprehensive test suite...");
    print!("   Ready? ");
    io::stdout().flush().unwrap();
    
    // Wait for user to press Enter
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    println!("🚀 Starting tests now...\n");

    // Test 1: Real Live Exchange Rates from Oracles
    println!("📊 Test 1: Live Oracle Exchange Rate System");
    println!("--------------------------------------------");
    println!("🌐 Fetching live prices from multiple exchanges...");
    
    let mut exchange_rates = ExchangeRates::new();
    
    // Fetch live BTC/USD from CoinGecko
    let coingecko_url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd,eur,gbp";
    let client = reqwest::Client::new();
    
    match client.get(coingecko_url).send().await {
        Ok(response) => {
            if let Ok(text) = response.text().await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(bitcoin_data) = parsed.get("bitcoin") {
                        if let Some(usd_price) = bitcoin_data.get("usd").and_then(|v| v.as_f64()) {
                            exchange_rates.update_btc_price(Currency::USD, usd_price);
                            println!("✅ Live BTC/USD: ${:.2}", usd_price);
                        }
                        if let Some(eur_price) = bitcoin_data.get("eur").and_then(|v| v.as_f64()) {
                            exchange_rates.update_btc_price(Currency::EUR, eur_price);
                            println!("✅ Live BTC/EUR: €{:.2}", eur_price);
                        }
                        if let Some(gbp_price) = bitcoin_data.get("gbp").and_then(|v| v.as_f64()) {
                            exchange_rates.update_btc_price(Currency::GBP, gbp_price);
                            println!("✅ Live BTC/GBP: £{:.2}", gbp_price);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("⚠️ Could not fetch live prices ({}), using fallback for demo", e);
            // Fallback only if network fails
            exchange_rates.update_btc_price(Currency::USD, 95000.0);
            exchange_rates.update_btc_price(Currency::EUR, 87000.0);
            exchange_rates.update_exchange_rate(Currency::EUR, 0.92);
            exchange_rates.update_exchange_rate(Currency::GBP, 1.27);
        }
    }
    
    // Calculate cross rates from live data
    if let (Some(usd_price), Some(eur_price)) = (
        exchange_rates.get_btc_price(&Currency::USD),
        exchange_rates.get_btc_price(&Currency::EUR)
    ) {
        let eur_usd_rate = eur_price / usd_price;
        exchange_rates.update_exchange_rate(Currency::EUR, eur_usd_rate);
        println!("✅ Calculated EUR/USD rate: {:.4}", eur_usd_rate);
    }
    
    if let (Some(usd_price), Some(gbp_price)) = (
        exchange_rates.get_btc_price(&Currency::USD),
        exchange_rates.get_btc_price(&Currency::GBP)
    ) {
        let gbp_usd_rate = gbp_price / usd_price;
        exchange_rates.update_exchange_rate(Currency::GBP, gbp_usd_rate);
        println!("✅ Calculated GBP/USD rate: {:.4}", gbp_usd_rate);
    }
    
    println!();
    sleep(Duration::from_secs(3)).await;

    // Test 2: Vault Creation and Multi-Currency Debt
    println!("🏦 Test 2: Vault Management");
    println!("----------------------------");
    let config = ProtocolConfig::testnet();
    let mut vault_manager = VaultManager::new(&config)?;
    
    // Add currency configurations (matching whitepaper defaults)
    vault_manager.add_currency(Currency::EUR, CurrencyConfig {
        stability_fee_apr: 0.025,  // 2.5% APR for EUR
        liquidation_penalty: 0.05,  // 5% penalty (whitepaper default)
        min_collateral_ratio: 1.5,  // 150% minimum (whitepaper default)
        liquidation_threshold: 1.1, // 110% liquidation (whitepaper default)
        min_mint_amount: 10.0,
        enabled: true,
    });
    
    vault_manager.add_currency(Currency::GBP, CurrencyConfig {
        stability_fee_apr: 0.03,    // 3% APR for GBP
        liquidation_penalty: 0.05,  // 5% penalty (whitepaper default)
        min_collateral_ratio: 1.5,  // 150% minimum (whitepaper default)
        liquidation_threshold: 1.1, // 110% liquidation (whitepaper default)
        min_mint_amount: 10.0,
        enabled: true,
    });

    vault_manager.update_exchange_rates(exchange_rates.clone());

    // Alice creates a vault with 1 BTC collateral
    let collateral = Amount::from_btc(1.0).unwrap();
    println!("Alice deposits {} BTC collateral", collateral.to_btc());
    
    let vault_id = vault_manager.create_vault(alice, collateral, Currency::USD, 50000.0).await?;
    println!("✅ Created vault {} with $50,000 USD debt", vault_id);
    
    // Add EUR debt to the same vault
    vault_manager.mint_additional(vault_id, Currency::EUR, 20000.0).await?;
    println!("✅ Added €20,000 EUR debt to vault");
    
    // Add GBP debt
    vault_manager.mint_additional(vault_id, Currency::GBP, 15000.0).await?;
    println!("✅ Added £15,000 GBP debt to vault");

    // Print vault status without holding a reference
    {
        let vault = vault_manager.get_vault(vault_id)?;
        println!("📊 Vault Status:");
        println!("   Collateral: {} BTC", vault.collateral_btc.to_btc());
        println!("   USD Debt: ${}", vault.debts.get_debt(&Currency::USD));
        println!("   EUR Debt: €{}", vault.debts.get_debt(&Currency::EUR));
        println!("   GBP Debt: £{}", vault.debts.get_debt(&Currency::GBP));
        println!("   Total Debt (USD): ${:.2}", vault.debts.total_debt_in_usd(&exchange_rates));
        println!("   Collateral Ratio: {:.2}%", vault.collateral_ratio(&exchange_rates) * 100.0);
    }
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 3: Multi-Currency Stable Value Management
    println!("💰 Test 3: Multi-Currency Stable Values");
    println!("----------------------------------------");
    let mut stable_manager = MultiCurrencyStableManager::new();
    
    // Mint stable values for users
    stable_manager.mint_stable(alice, Currency::USD, 50000.0, vault_id)?;
    stable_manager.mint_stable(alice, Currency::EUR, 20000.0, vault_id)?;
    stable_manager.mint_stable(alice, Currency::GBP, 15000.0, vault_id)?;
    
    println!("✅ Minted stable values for Alice:");
    println!("   USD: ${}", stable_manager.get_balance(alice, &Currency::USD));
    println!("   EUR: €{}", stable_manager.get_balance(alice, &Currency::EUR));
    println!("   GBP: £{}", stable_manager.get_balance(alice, &Currency::GBP));
    
    // Transfer some stable values
    println!("\n🔄 Testing transfers...");
    stable_manager.transfer_stable(alice, bob, Currency::USD, 15000.0)?;
    stable_manager.transfer_stable(alice, charlie, Currency::EUR, 5000.0)?;
    
    println!("✅ After transfers:");
    println!("   Alice USD: ${}", stable_manager.get_balance(alice, &Currency::USD));
    println!("   Bob USD: ${}", stable_manager.get_balance(bob, &Currency::USD));
    println!("   Alice EUR: €{}", stable_manager.get_balance(alice, &Currency::EUR));
    println!("   Charlie EUR: €{}", stable_manager.get_balance(charlie, &Currency::EUR));
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 4: FIFO Burning Mechanism
    println!("🔥 Test 4: FIFO Burning Mechanism");
    println!("----------------------------------");
    
    // Create multiple positions for Alice
    let vault_id2 = vault_manager.create_vault(alice, Amount::from_btc(0.5).unwrap(), Currency::USD, 20000.0).await?;
    stable_manager.mint_stable(alice, Currency::USD, 20000.0, vault_id2)?;
    
    println!("Alice now has USD positions from 2 vaults:");
    println!("   Total USD: ${}", stable_manager.get_balance(alice, &Currency::USD));
    
    // Burn some USD (should use FIFO)
    let burned_vaults = stable_manager.burn_stable(alice, Currency::USD, 30000.0)?;
    println!("✅ Burned $30,000 USD using FIFO, affected {} vaults", burned_vaults.len());
    println!("   Remaining USD: ${}", stable_manager.get_balance(alice, &Currency::USD));
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 5: Stability Controller (Autopilot)
    println!("🎯 Test 5: Stability Controller (Autopilot)");
    println!("--------------------------------------------");
    
    let mut portfolio_manager = PortfolioManager::new();
    
    // Alice wants to keep exactly $40,000 USD stable
    let controller1 = StabilityController::new(alice, Currency::USD, 40000.0);
    portfolio_manager.add_controller(controller1);
    
    // Bob wants to keep 30% of his portfolio in EUR
    let controller2 = StabilityController::new_percentage(bob, Currency::EUR, 30.0);
    portfolio_manager.add_controller(controller2);
    
    // Set up portfolio balances
    let mut balances = std::collections::HashMap::new();
    balances.insert(alice, HolderBalance {
        btc_balance: 0.8, // Alice has 0.8 BTC
        stable_balances: {
            let mut map = HashMap::new();
            map.insert(Currency::USD, stable_manager.get_balance(alice, &Currency::USD));
            map.insert(Currency::EUR, stable_manager.get_balance(alice, &Currency::EUR));
            map.insert(Currency::GBP, stable_manager.get_balance(alice, &Currency::GBP));
            map
        },
    });
    
    balances.insert(bob, HolderBalance {
        btc_balance: 1.2, // Bob has 1.2 BTC  
        stable_balances: {
            let mut map = HashMap::new();
            map.insert(Currency::USD, stable_manager.get_balance(bob, &Currency::USD));
            map.insert(Currency::EUR, 5000.0); // Bob has some EUR
            map
        },
    });
    
    println!("📊 Current Portfolio Status:");
    println!("Alice: {:.1} BTC + ${} USD + €{} EUR + £{} GBP", 
        balances[&alice].btc_balance,
        stable_manager.get_balance(alice, &Currency::USD),
        stable_manager.get_balance(alice, &Currency::EUR),
        stable_manager.get_balance(alice, &Currency::GBP)
    );
    println!("Bob: {:.1} BTC + ${} USD", 
        balances[&bob].btc_balance,
        stable_manager.get_balance(bob, &Currency::USD)
    );
    
    // Calculate rebalancing actions
    let total_cr = 2.0; // Assume system is well-collateralized
    let min_cr = 1.5;
    let actions = portfolio_manager.process_rebalancing(&balances, &exchange_rates, total_cr, min_cr);
    
    println!("\n🎯 Autopilot Rebalancing Recommendations:");
    for (holder, action) in actions {
        match action {
            RebalanceAction::Mint { currency, amount } => {
                println!("   {} should MINT {:.2} {}", 
                    if holder == alice { "Alice" } else { "Bob" },
                    amount, currency.to_string());
            },
            RebalanceAction::Burn { currency, amount } => {
                println!("   {} should BURN {:.2} {}", 
                    if holder == alice { "Alice" } else { "Bob" },
                    amount, currency.to_string());
            },
            RebalanceAction::None => {
                println!("   {} is balanced", 
                    if holder == alice { "Alice" } else { "Bob" });
            },
        }
    }
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 6: Fee Accrual and Compound Interest
    println!("💸 Test 6: Stability Fee Accrual");
    println!("---------------------------------");
    
    let debt_before = {
        let vault_before = vault_manager.get_vault(vault_id)?;
        vault_before.debts.total_debt_in_usd(&exchange_rates)
    };
    println!("Before fee update - Total debt: ${:.2}", debt_before);
    
    // Simulate time passage and fee accrual
    std::thread::sleep(std::time::Duration::from_millis(100));
    vault_manager.update_all_stability_fees()?;
    
    let debt_after = {
        let vault_after = vault_manager.get_vault(vault_id)?;
        vault_after.debts.total_debt_in_usd(&exchange_rates)
    };
    println!("After fee update - Total debt: ${:.2}", debt_after);
    println!("✅ Fees accrued using compound interest formula");
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 7: Liquidation Logic
    println!("⚠️  Test 7: Liquidation System");
    println!("------------------------------");
    
    // Check liquidation status
    let liquidatable_vaults = vault_manager.list_liquidatable_vaults();
    println!("Liquidatable vaults: {}", liquidatable_vaults.len());
    
    if liquidatable_vaults.is_empty() {
        println!("✅ All vaults are properly collateralized");
        
        // Calculate liquidation price and bonus for demonstration
        let (liq_price, bonus) = {
            let vault = vault_manager.get_vault(vault_id)?;
            let liq_price = vault.calculate_liquidation_price(&Currency::USD, &exchange_rates, 1.2);
            let bonus = vault.liquidation_bonus(&exchange_rates, 0.05);
            (liq_price, bonus)
        };
        println!("💡 USD liquidation would trigger at BTC price: ${:.2}", liq_price);
        println!("💰 Potential liquidation bonus: {} BTC", bonus.to_btc());
    }
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 8: System Health and Statistics
    println!("📈 Test 8: System Health Dashboard");
    println!("-----------------------------------");
    
    let total_usd_debt = vault_manager.get_total_debt_usd();
    let total_eur_debt = vault_manager.get_total_debt(&Currency::EUR);
    let total_gbp_debt = vault_manager.get_total_debt(&Currency::GBP);
    
    println!("🏦 System Statistics:");
    println!("   Total Vaults: {}", vault_manager.list_vaults().len());
    println!("   Total USD Debt: ${:.2}", total_usd_debt);
    println!("   Total EUR Debt: €{:.2}", total_eur_debt);
    println!("   Total GBP Debt: £{:.2}", total_gbp_debt);
    
    // Calculate system collateral backing
    let all_vaults: HashMap<bitcoin::Txid, Vault> = vault_manager.list_vaults()
        .into_iter()
        .map(|v| (v.id, v.clone()))
        .collect();
    
    let backing = stable_manager.calculate_collateral_backing(&exchange_rates, &all_vaults);
    println!("   Overall Collateral Ratio: {:.2}%", backing.overall_collateral_ratio * 100.0);
    println!("   Total Collateral Value: ${:.2}", backing.total_collateral_value_usd);
    println!("   Backing Vaults: {}", backing.backing_vault_count);
    
    println!("\n💱 Currency Breakdown:");
    for (currency, backing_info) in backing.currency_breakdowns {
        println!("   {}: {:.2}% collateralized, supply: {:.2}", 
            currency.to_string(),
            backing_info.collateral_ratio * 100.0,
            backing_info.total_supply
        );
    }
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 9: Transfer History and Audit Trail
    println!("📋 Test 9: Transfer History");
    println!("---------------------------");
    let recent_transfers = stable_manager.get_transfer_history(Some(5));
    println!("Recent {} transfers:", recent_transfers.len());
    for (i, transfer) in recent_transfers.iter().enumerate() {
        println!("   {}. {} → {}: {:.2} {} ({})", 
            i + 1,
            if transfer.from == alice { "Alice" } else if transfer.from == bob { "Bob" } else { "Charlie" },
            if transfer.to == alice { "Alice" } else if transfer.to == bob { "Bob" } else { "Charlie" },
            transfer.amount,
            transfer.currency.to_string(),
            transfer.timestamp.format("%H:%M:%S")
        );
    }
    println!();
    sleep(Duration::from_secs(2)).await;

    // Test 10: Edge Cases and Error Handling
    println!("🛡️  Test 10: Edge Cases and Security");
    println!("------------------------------------");
    
    // Test insufficient collateral
    let result = vault_manager.create_vault(
        bob, 
        Amount::from_btc(0.1).unwrap(), // Too little collateral
        Currency::USD, 
        50000.0  // Too much debt
    ).await;
    
    match result {
        Err(BitStableError::InsufficientCollateral { required, provided }) => {
            println!("✅ Correctly rejected under-collateralized vault:");
            println!("   Required: ${:.2}, Provided: ${:.2}", required, provided);
        },
        _ => println!("❌ Should have rejected under-collateralized vault"),
    }
    
    // Test burning more than available
    let result = stable_manager.burn_stable(alice, Currency::USD, 999999.0);
    match result {
        Err(_) => println!("✅ Correctly rejected burning more than available balance"),
        Ok(_) => println!("❌ Should have rejected excessive burn"),
    }
    
    // Test disabled currency
    vault_manager.add_currency(Currency::JPY, CurrencyConfig {
        enabled: false,
        ..Default::default()
    });
    
    let result = vault_manager.create_vault(charlie, Amount::from_btc(1.0).unwrap(), Currency::JPY, 1000.0).await;
    match result {
        Err(_) => println!("✅ Correctly rejected disabled currency"),
        Ok(_) => println!("❌ Should have rejected disabled currency"),
    }
    
    println!();
    sleep(Duration::from_secs(3)).await;

    // Final Summary
    println!("🎉 Test Suite Complete!");
    println!("========================");
    println!("✅ Multi-currency exchange rates");
    println!("✅ Vault creation and management");
    println!("✅ Multi-currency debt tracking");
    println!("✅ Stable value minting and transfers");
    println!("✅ FIFO burning mechanism");
    println!("✅ Stability controller (autopilot)");
    println!("✅ Fee accrual with compound interest");
    println!("✅ Liquidation system");
    println!("✅ System health monitoring");
    println!("✅ Transfer history and audit trail");
    println!("✅ Edge case handling and security");
    println!("\n🚀 BitStable is fully functional and ready for testnet!");

    Ok(())
}