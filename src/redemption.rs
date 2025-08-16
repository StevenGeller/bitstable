use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig, VaultManager};
use crate::multi_currency::{Currency, ExchangeRates};

/// Direct redemption engine for maintaining stablecoin peg
#[derive(Debug)]
pub struct RedemptionEngine {
    config: ProtocolConfig,
    daily_redemption_limits: HashMap<Currency, f64>,
    daily_redemption_used: HashMap<Currency, f64>,
    last_reset: DateTime<Utc>,
    redemption_history: Vec<RedemptionRecord>,
    base_redemption_fee: f64,          // Base 0.5% fee
    dynamic_fee_multiplier: f64,       // Multiplier based on demand
    #[allow(dead_code)]
    redemption_pool: HashMap<Currency, f64>, // Available for immediate redemption
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionRecord {
    pub redeemer: PublicKey,
    pub currency: Currency,
    pub stable_amount: f64,
    pub btc_received: Amount,
    pub fee_paid: f64,
    pub vault_id: Txid,                // Which vault provided collateral
    pub redemption_price: f64,         // BTC price at redemption
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionOpportunity {
    pub vault_id: Txid,
    pub owner: PublicKey,
    pub collateral_ratio: f64,
    pub available_collateral: Amount,
    pub total_debt: f64,
    pub redemption_priority: f64,      // Lower ratio = higher priority
}

impl RedemptionEngine {
    pub fn new(config: &ProtocolConfig) -> Self {
        let mut daily_limits = HashMap::new();
        daily_limits.insert(Currency::USD, 1_000_000.0);  // $1M daily limit
        daily_limits.insert(Currency::EUR, 900_000.0);    // €900K daily limit
        daily_limits.insert(Currency::GBP, 800_000.0);    // £800K daily limit
        
        Self {
            config: config.clone(),
            daily_redemption_limits: daily_limits,
            daily_redemption_used: HashMap::new(),
            last_reset: Utc::now(),
            redemption_history: Vec::new(),
            base_redemption_fee: 0.005,    // 0.5%
            dynamic_fee_multiplier: 1.0,
            redemption_pool: HashMap::new(),
        }
    }

    /// Redeem stablecoins for BTC collateral from least collateralized vaults
    pub async fn redeem_stablecoins(
        &mut self,
        redeemer: PublicKey,
        currency: Currency,
        stable_amount: f64,
        vault_manager: &mut VaultManager,
        exchange_rates: &ExchangeRates,
    ) -> Result<RedemptionRecord> {
        // Reset daily limits if needed
        self.reset_daily_limits_if_needed();
        
        // Check daily redemption limits
        self.check_daily_limits(&currency, stable_amount)?;
        
        // Calculate redemption fee
        let redemption_fee = self.calculate_dynamic_redemption_fee(&currency, stable_amount);
        let net_stable_amount = stable_amount * (1.0 - redemption_fee);
        
        // Find best vault for redemption (lowest collateral ratio)
        let redemption_target = self.find_best_redemption_target(
            vault_manager, 
            &currency, 
            net_stable_amount, 
            exchange_rates
        )?;
        
        // Calculate BTC to redeem
        let btc_price = exchange_rates.get_btc_price(&currency)
            .ok_or_else(|| BitStableError::PriceFeedError("Currency price not available".to_string()))?;
        
        let btc_amount = Amount::from_btc(net_stable_amount / btc_price)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid BTC amount: {}", e)))?;
        
        // Execute redemption on the target vault
        let redemption_record = self.execute_redemption(
            redeemer,
            currency.clone(),
            stable_amount,
            net_stable_amount,
            btc_amount,
            redemption_fee,
            redemption_target,
            btc_price,
            vault_manager,
        ).await?;
        
        // Update daily limits
        *self.daily_redemption_used.entry(currency.clone()).or_insert(0.0) += stable_amount;
        
        // Update dynamic fee based on redemption demand
        self.update_dynamic_fee(&currency, stable_amount);
        
        log::info!(
            "Redemption completed: {} {} for {} BTC (fee: {:.2}%)",
            stable_amount,
            currency.to_string(),
            btc_amount.to_btc(),
            redemption_fee * 100.0
        );
        
        Ok(redemption_record)
    }
    
    /// Find the best vault for redemption (lowest collateral ratio above minimum)
    fn find_best_redemption_target(
        &self,
        vault_manager: &VaultManager,
        currency: &Currency,
        stable_amount: f64,
        exchange_rates: &ExchangeRates,
    ) -> Result<RedemptionOpportunity> {
        let vaults = vault_manager.get_active_vaults();
        let mut opportunities = Vec::new();
        
        for vault in vaults {
            let collateral_ratio = vault.collateral_ratio(exchange_rates);
            
            // Only consider vaults above minimum collateral ratio with debt in this currency
            if collateral_ratio >= self.config.min_collateral_ratio && 
               vault.debts.get_debt(currency) > 0.0 {
                
                let available_debt = vault.debts.get_debt(currency);
                let redeemable_amount = available_debt.min(stable_amount);
                
                if redeemable_amount > 0.0 {
                    opportunities.push(RedemptionOpportunity {
                        vault_id: vault.id,
                        owner: vault.owner,
                        collateral_ratio,
                        available_collateral: vault.collateral_btc,
                        total_debt: available_debt,
                        redemption_priority: collateral_ratio, // Lower = better for redemption
                    });
                }
            }
        }
        
        if opportunities.is_empty() {
            return Err(BitStableError::InvalidConfig("No vaults available for redemption".to_string()));
        }
        
        // Sort by collateral ratio (ascending - target lowest ratio first)
        opportunities.sort_by(|a, b| a.redemption_priority.partial_cmp(&b.redemption_priority).unwrap());
        
        Ok(opportunities.into_iter().next().unwrap())
    }
    
    /// Execute the actual redemption on a vault
    async fn execute_redemption(
        &mut self,
        redeemer: PublicKey,
        currency: Currency,
        original_stable_amount: f64,
        net_stable_amount: f64,
        btc_amount: Amount,
        fee_rate: f64,
        target: RedemptionOpportunity,
        btc_price: f64,
        vault_manager: &mut VaultManager,
    ) -> Result<RedemptionRecord> {
        // Reduce vault debt and collateral
        vault_manager.process_redemption(
            target.vault_id,
            currency.clone(),
            net_stable_amount,
            redeemer,
        )?;
        
        let record = RedemptionRecord {
            redeemer,
            currency,
            stable_amount: original_stable_amount,
            btc_received: btc_amount,
            fee_paid: fee_rate,
            vault_id: target.vault_id,
            redemption_price: btc_price,
            timestamp: Utc::now(),
        };
        
        self.redemption_history.push(record.clone());
        
        // Keep only last 10,000 records
        if self.redemption_history.len() > 10_000 {
            self.redemption_history.remove(0);
        }
        
        Ok(record)
    }
    
    /// Calculate dynamic redemption fee based on demand
    fn calculate_dynamic_redemption_fee(&self, currency: &Currency, _amount: f64) -> f64 {
        let daily_limit = self.daily_redemption_limits.get(currency).copied().unwrap_or(1_000_000.0);
        let daily_used = self.daily_redemption_used.get(currency).copied().unwrap_or(0.0);
        let utilization = daily_used / daily_limit;
        
        // Exponential fee curve: base_fee * (1 + utilization^2 * multiplier)
        let dynamic_fee = self.base_redemption_fee * (1.0 + utilization.powi(2) * self.dynamic_fee_multiplier);
        
        // Cap at 2% maximum fee
        dynamic_fee.min(0.02)
    }
    
    /// Update dynamic fee multiplier based on recent redemption pressure
    fn update_dynamic_fee(&mut self, currency: &Currency, _amount: f64) {
        let recent_redemptions: f64 = self.redemption_history
            .iter()
            .rev()
            .take(100)  // Last 100 redemptions
            .filter(|r| r.currency == *currency)
            .map(|r| r.stable_amount)
            .sum();
        
        // Increase multiplier if high recent activity
        if recent_redemptions > 100_000.0 {  // $100k in recent redemptions
            self.dynamic_fee_multiplier = (self.dynamic_fee_multiplier * 1.1).min(3.0);
        } else {
            self.dynamic_fee_multiplier = (self.dynamic_fee_multiplier * 0.99).max(1.0);
        }
    }
    
    /// Check if redemption is within daily limits
    fn check_daily_limits(&self, currency: &Currency, amount: f64) -> Result<()> {
        let daily_limit = self.daily_redemption_limits.get(currency).copied().unwrap_or(1_000_000.0);
        let daily_used = self.daily_redemption_used.get(currency).copied().unwrap_or(0.0);
        
        if daily_used + amount > daily_limit {
            return Err(BitStableError::InvalidConfig(format!(
                "Redemption would exceed daily limit for {}: {} / {} used",
                currency.to_string(),
                daily_used + amount,
                daily_limit
            )));
        }
        
        Ok(())
    }
    
    /// Reset daily limits if a new day has started
    fn reset_daily_limits_if_needed(&mut self) {
        let now = Utc::now();
        if now.date_naive() != self.last_reset.date_naive() {
            self.daily_redemption_used.clear();
            self.last_reset = now;
            log::info!("Daily redemption limits reset");
        }
    }
    
    /// Get redemption statistics
    pub fn get_redemption_stats(&self) -> RedemptionStats {
        let total_redemptions = self.redemption_history.len();
        let total_volume: f64 = self.redemption_history.iter().map(|r| r.stable_amount).sum();
        let total_btc_redeemed: f64 = self.redemption_history.iter().map(|r| r.btc_received.to_btc()).sum();
        let total_fees: f64 = self.redemption_history.iter().map(|r| r.fee_paid * r.stable_amount).sum();
        
        let avg_fee = if total_volume > 0.0 {
            total_fees / total_volume
        } else {
            0.0
        };
        
        RedemptionStats {
            total_redemptions,
            total_volume_usd: total_volume,
            total_btc_redeemed: Amount::from_btc(total_btc_redeemed).unwrap_or(Amount::ZERO),
            average_fee_rate: avg_fee,
            current_dynamic_multiplier: self.dynamic_fee_multiplier,
            daily_limits: self.daily_redemption_limits.clone(),
            daily_used: self.daily_redemption_used.clone(),
        }
    }
    
    /// Get recent redemption history
    pub fn get_recent_redemptions(&self, limit: usize) -> Vec<&RedemptionRecord> {
        self.redemption_history
            .iter()
            .rev()
            .take(limit)
            .collect()
    }
    
    /// Estimate redemption output for a given amount
    pub fn estimate_redemption(
        &self,
        currency: &Currency,
        stable_amount: f64,
        exchange_rates: &ExchangeRates,
    ) -> Result<RedemptionEstimate> {
        let redemption_fee = self.calculate_dynamic_redemption_fee(currency, stable_amount);
        let net_stable_amount = stable_amount * (1.0 - redemption_fee);
        
        let btc_price = exchange_rates.get_btc_price(currency)
            .ok_or_else(|| BitStableError::PriceFeedError("Currency price not available".to_string()))?;
        
        let btc_amount = Amount::from_btc(net_stable_amount / btc_price)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid BTC amount: {}", e)))?;
        
        Ok(RedemptionEstimate {
            input_stable_amount: stable_amount,
            redemption_fee_rate: redemption_fee,
            fee_amount: stable_amount * redemption_fee,
            net_stable_amount,
            btc_output: btc_amount,
            btc_price_used: btc_price,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionStats {
    pub total_redemptions: usize,
    pub total_volume_usd: f64,
    pub total_btc_redeemed: Amount,
    pub average_fee_rate: f64,
    pub current_dynamic_multiplier: f64,
    pub daily_limits: HashMap<Currency, f64>,
    pub daily_used: HashMap<Currency, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionEstimate {
    pub input_stable_amount: f64,
    pub redemption_fee_rate: f64,
    pub fee_amount: f64,
    pub net_stable_amount: f64,
    pub btc_output: Amount,
    pub btc_price_used: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;

    #[test]
    fn test_dynamic_fee_calculation() {
        let config = ProtocolConfig::testnet();
        let mut redemption_engine = RedemptionEngine::new(&config);
        
        // Base fee should be 0.5%
        let fee = redemption_engine.calculate_dynamic_redemption_fee(&Currency::USD, 1000.0);
        assert!(fee >= 0.005 && fee <= 0.02);
        
        // High utilization should increase fee
        redemption_engine.daily_redemption_used.insert(Currency::USD, 800_000.0);
        let high_utilization_fee = redemption_engine.calculate_dynamic_redemption_fee(&Currency::USD, 100_000.0);
        assert!(high_utilization_fee > fee);
    }
    
    #[test]
    fn test_daily_limit_reset() {
        let config = ProtocolConfig::testnet();
        let mut redemption_engine = RedemptionEngine::new(&config);
        
        redemption_engine.daily_redemption_used.insert(Currency::USD, 500_000.0);
        assert_eq!(redemption_engine.daily_redemption_used.get(&Currency::USD), Some(&500_000.0));
        
        // Simulate day change
        redemption_engine.last_reset = Utc::now() - chrono::Duration::days(1);
        redemption_engine.reset_daily_limits_if_needed();
        
        assert_eq!(redemption_engine.daily_redemption_used.get(&Currency::USD), None);
    }
}