use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig};
use crate::multi_currency::{Currency, ExchangeRates};

/// Stability pool where users pre-commit stablecoins for liquidations and earn rewards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityPool {
    pub deposits: HashMap<PublicKey, StabilityDeposit>,
    pub total_deposited: HashMap<Currency, f64>,
    pub total_rewards_earned: HashMap<Currency, Amount>,
    pub liquidation_history: Vec<StabilityLiquidation>,
    pub pool_config: StabilityPoolConfig,
    pub reward_snapshots: Vec<RewardSnapshot>,
    pub last_reward_distribution: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityDeposit {
    pub depositor: PublicKey,
    pub deposits_by_currency: HashMap<Currency, f64>,
    pub rewards_earned: HashMap<Currency, Amount>,
    pub deposit_timestamp: DateTime<Utc>,
    pub last_claim: DateTime<Utc>,
    pub total_liquidation_gains: Amount,
    pub liquidation_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityLiquidation {
    pub vault_id: Txid,
    pub liquidated_debt: HashMap<Currency, f64>,
    pub collateral_distributed: Amount,
    pub participants: Vec<LiquidationParticipant>,
    pub timestamp: DateTime<Utc>,
    pub liquidation_bonus: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationParticipant {
    pub depositor: PublicKey,
    pub debt_absorbed: HashMap<Currency, f64>,
    pub collateral_received: Amount,
    pub share_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityPoolConfig {
    pub min_deposit_amount: f64,
    pub withdrawal_delay_hours: u64,
    pub reward_distribution_frequency_hours: u64,
    pub liquidation_priority: bool,         // Pool gets first chance at liquidations
    pub maximum_pool_utilization: f64,      // Max % of total debt pool can absorb
    pub early_withdrawal_penalty: f64,      // Penalty for early withdrawal
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_pool_size: HashMap<Currency, f64>,
    pub rewards_distributed: HashMap<Currency, Amount>,
    pub participating_depositors: usize,
}

impl StabilityPool {
    pub fn new(_config: &ProtocolConfig) -> Self {
        let pool_config = StabilityPoolConfig {
            min_deposit_amount: 100.0,              // $100 minimum
            withdrawal_delay_hours: 24,             // 24 hour withdrawal delay
            reward_distribution_frequency_hours: 168, // Weekly reward distribution
            liquidation_priority: true,
            maximum_pool_utilization: 0.5,         // Pool can absorb 50% of liquidations
            early_withdrawal_penalty: 0.01,        // 1% early withdrawal penalty
        };

        Self {
            deposits: HashMap::new(),
            total_deposited: HashMap::new(),
            total_rewards_earned: HashMap::new(),
            liquidation_history: Vec::new(),
            pool_config,
            reward_snapshots: Vec::new(),
            last_reward_distribution: Utc::now(),
        }
    }

    /// Deposit stablecoins into the stability pool
    pub fn deposit(
        &mut self,
        depositor: PublicKey,
        currency: Currency,
        amount: f64,
    ) -> Result<()> {
        if amount < self.pool_config.min_deposit_amount {
            return Err(BitStableError::InvalidConfig(format!(
                "Deposit amount {} below minimum {}",
                amount,
                self.pool_config.min_deposit_amount
            )));
        }

        // Create or update deposit
        let deposit = self.deposits.entry(depositor).or_insert_with(|| StabilityDeposit {
            depositor,
            deposits_by_currency: HashMap::new(),
            rewards_earned: HashMap::new(),
            deposit_timestamp: Utc::now(),
            last_claim: Utc::now(),
            total_liquidation_gains: Amount::ZERO,
            liquidation_count: 0,
        });

        // Add to depositor's balance
        *deposit.deposits_by_currency.entry(currency.clone()).or_insert(0.0) += amount;

        // Add to total pool
        *self.total_deposited.entry(currency.clone()).or_insert(0.0) += amount;

        log::info!(
            "Stability pool deposit: {} deposited {} {}",
            depositor,
            amount,
            currency.to_string()
        );

        Ok(())
    }

    /// Withdraw stablecoins from the stability pool
    pub fn withdraw(
        &mut self,
        depositor: PublicKey,
        currency: Currency,
        amount: f64,
    ) -> Result<WithdrawalResult> {
        let deposit = self.deposits.get_mut(&depositor)
            .ok_or_else(|| BitStableError::InvalidConfig("No deposit found".to_string()))?;

        let available = deposit.deposits_by_currency.get(&currency).copied().unwrap_or(0.0);
        if amount > available {
            return Err(BitStableError::InvalidConfig(format!(
                "Insufficient balance: {} requested, {} available",
                amount,
                available
            )));
        }

        // Check withdrawal delay
        let hours_since_deposit = Utc::now()
            .signed_duration_since(deposit.deposit_timestamp)
            .num_hours() as u64;

        let penalty = if hours_since_deposit < self.pool_config.withdrawal_delay_hours {
            amount * self.pool_config.early_withdrawal_penalty
        } else {
            0.0
        };

        let net_withdrawal = amount - penalty;

        // Update balances
        *deposit.deposits_by_currency.get_mut(&currency).unwrap() -= amount;
        *self.total_deposited.get_mut(&currency).unwrap() -= amount;

        // Remove empty deposits
        if deposit.deposits_by_currency.get(&currency).copied().unwrap_or(0.0) == 0.0 {
            deposit.deposits_by_currency.remove(&currency);
        }

        let result = WithdrawalResult {
            depositor,
            currency: currency.clone(),
            requested_amount: amount,
            penalty_amount: penalty,
            net_amount: net_withdrawal,
            early_withdrawal: hours_since_deposit < self.pool_config.withdrawal_delay_hours,
        };

        log::info!(
            "Stability pool withdrawal: {} withdrew {} {} (penalty: {})",
            depositor,
            net_withdrawal,
            currency.to_string(),
            penalty
        );

        Ok(result)
    }

    /// Process a liquidation using the stability pool
    pub fn process_liquidation(
        &mut self,
        vault_id: Txid,
        liquidated_debt: HashMap<Currency, f64>,
        collateral_amount: Amount,
        _exchange_rates: &ExchangeRates,
    ) -> Result<StabilityLiquidation> {
        let mut participants = Vec::new();
        let mut remaining_debt = liquidated_debt.clone();
        let total_collateral = collateral_amount;

        // Calculate total pool capacity for each currency
        let mut total_pool_shares = HashMap::new();
        for (currency, debt_amount) in &liquidated_debt {
            let pool_size = self.total_deposited.get(currency).copied().unwrap_or(0.0);
            let max_absorption = pool_size * self.pool_config.maximum_pool_utilization;
            let debt_to_absorb = debt_amount.min(max_absorption);
            
            if debt_to_absorb > 0.0 {
                total_pool_shares.insert(currency.clone(), pool_size);
            }
        }

        // Distribute liquidation among depositors proportionally
        for (depositor, deposit) in &mut self.deposits {
            let mut participant = LiquidationParticipant {
                depositor: *depositor,
                debt_absorbed: HashMap::new(),
                collateral_received: Amount::ZERO,
                share_percentage: 0.0,
            };

            let mut total_share = 0.0;

            for (currency, debt_amount) in &remaining_debt {
                if let Some(depositor_balance) = deposit.deposits_by_currency.get(currency) {
                    if let Some(total_pool) = total_pool_shares.get(currency) {
                        if *total_pool > 0.0 {
                            let share = depositor_balance / total_pool;
                            let debt_absorbed = debt_amount * share;
                            
                            if debt_absorbed > 0.0 {
                                participant.debt_absorbed.insert(currency.clone(), debt_absorbed);
                                total_share += share;
                                
                                // Reduce depositor's balance
                                *deposit.deposits_by_currency.get_mut(currency).unwrap() -= debt_absorbed;
                                *self.total_deposited.get_mut(currency).unwrap() -= debt_absorbed;
                            }
                        }
                    }
                }
            }

            if total_share > 0.0 {
                // Calculate collateral reward
                let collateral_share = Amount::from_btc(total_collateral.to_btc() * total_share)
                    .map_err(|e| BitStableError::InvalidConfig(format!("Invalid collateral amount: {}", e)))?;
                
                participant.collateral_received = collateral_share;
                participant.share_percentage = total_share;

                // Update deposit rewards
                deposit.total_liquidation_gains += collateral_share;
                deposit.liquidation_count += 1;
                *deposit.rewards_earned.entry(Currency::USD).or_insert(Amount::ZERO) += collateral_share;

                participants.push(participant);
            }
        }

        // Update remaining debt after pool absorption
        for participant in &participants {
            for (currency, absorbed) in &participant.debt_absorbed {
                if let Some(remaining) = remaining_debt.get_mut(currency) {
                    *remaining -= absorbed;
                }
            }
        }

        let liquidation = StabilityLiquidation {
            vault_id,
            liquidated_debt: liquidated_debt.clone(),
            collateral_distributed: total_collateral,
            participants,
            timestamp: Utc::now(),
            liquidation_bonus: Amount::ZERO, // Could be calculated based on config
        };

        self.liquidation_history.push(liquidation.clone());

        log::info!(
            "Stability pool processed liquidation for vault {}: {} participants",
            vault_id,
            liquidation.participants.len()
        );

        Ok(liquidation)
    }

    /// Claim accumulated rewards
    pub fn claim_rewards(
        &mut self,
        depositor: PublicKey,
        currency: Currency,
    ) -> Result<Amount> {
        let deposit = self.deposits.get_mut(&depositor)
            .ok_or_else(|| BitStableError::InvalidConfig("No deposit found".to_string()))?;

        let rewards = deposit.rewards_earned.get(&currency).copied().unwrap_or(Amount::ZERO);
        
        if rewards == Amount::ZERO {
            return Err(BitStableError::InvalidConfig("No rewards to claim".to_string()));
        }

        // Reset claimed rewards
        deposit.rewards_earned.insert(currency.clone(), Amount::ZERO);
        deposit.last_claim = Utc::now();

        log::info!(
            "Rewards claimed: {} claimed {} BTC rewards",
            depositor,
            rewards.to_btc()
        );

        Ok(rewards)
    }

    /// Calculate pending rewards for a depositor
    pub fn calculate_pending_rewards(
        &self,
        depositor: PublicKey,
        currency: Currency,
    ) -> Amount {
        if let Some(deposit) = self.deposits.get(&depositor) {
            deposit.rewards_earned.get(&currency).copied().unwrap_or(Amount::ZERO)
        } else {
            Amount::ZERO
        }
    }

    /// Get pool statistics
    pub fn get_pool_stats(&self) -> StabilityPoolStats {
        let total_depositors = self.deposits.len();
        let active_depositors = self.deposits.values()
            .filter(|d| !d.deposits_by_currency.is_empty())
            .count();

        let total_liquidations = self.liquidation_history.len();
        let total_collateral_distributed: f64 = self.liquidation_history.iter()
            .map(|l| l.collateral_distributed.to_btc())
            .sum();

        let avg_deposit_size: f64 = if total_depositors > 0 {
            self.total_deposited.values().sum::<f64>() / total_depositors as f64
        } else {
            0.0
        };

        StabilityPoolStats {
            total_depositors,
            active_depositors,
            total_deposited: self.total_deposited.clone(),
            total_liquidations,
            total_collateral_distributed: Amount::from_btc(total_collateral_distributed).unwrap_or(Amount::ZERO),
            average_deposit_size: avg_deposit_size,
            pool_utilization: self.calculate_current_utilization(),
            last_liquidation: self.liquidation_history.last().map(|l| l.timestamp),
        }
    }

    /// Calculate current pool utilization
    fn calculate_current_utilization(&self) -> f64 {
        let total_deposited: f64 = self.total_deposited.values().sum();
        if total_deposited > 0.0 {
            // This would need integration with system debt to calculate actual utilization
            0.0 // Placeholder
        } else {
            0.0
        }
    }

    /// Get depositor information
    pub fn get_depositor_info(&self, depositor: PublicKey) -> Option<DepositorInfo> {
        self.deposits.get(&depositor).map(|deposit| {
            let total_deposited: f64 = deposit.deposits_by_currency.values().sum();
            let total_rewards: f64 = deposit.rewards_earned.values()
                .map(|r| r.to_btc())
                .sum();

            DepositorInfo {
                depositor,
                total_deposited,
                deposits_by_currency: deposit.deposits_by_currency.clone(),
                total_rewards_btc: total_rewards,
                liquidation_count: deposit.liquidation_count,
                total_liquidation_gains: deposit.total_liquidation_gains,
                deposit_date: deposit.deposit_timestamp,
                last_claim_date: deposit.last_claim,
            }
        })
    }

    /// Get recent liquidation history
    pub fn get_recent_liquidations(&self, limit: usize) -> Vec<&StabilityLiquidation> {
        self.liquidation_history
            .iter()
            .rev()
            .take(limit)
            .collect()
    }

    /// Estimate liquidation capacity for a given debt amount
    pub fn estimate_liquidation_capacity(
        &self,
        liquidated_debt: &HashMap<Currency, f64>,
    ) -> LiquidationCapacityEstimate {
        let mut total_capacity = 0.0;
        let mut capacity_by_currency = HashMap::new();

        for (currency, debt_amount) in liquidated_debt {
            let pool_size = self.total_deposited.get(currency).copied().unwrap_or(0.0);
            let max_absorption = pool_size * self.pool_config.maximum_pool_utilization;
            let can_absorb = debt_amount.min(max_absorption);
            
            capacity_by_currency.insert(currency.clone(), can_absorb);
            total_capacity += can_absorb;
        }

        let coverage_percentage = if liquidated_debt.values().sum::<f64>() > 0.0 {
            total_capacity / liquidated_debt.values().sum::<f64>()
        } else {
            0.0
        };

        LiquidationCapacityEstimate {
            total_debt_amount: liquidated_debt.values().sum(),
            pool_can_absorb: total_capacity,
            coverage_percentage,
            capacity_by_currency,
            insufficient_coverage: coverage_percentage < 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalResult {
    pub depositor: PublicKey,
    pub currency: Currency,
    pub requested_amount: f64,
    pub penalty_amount: f64,
    pub net_amount: f64,
    pub early_withdrawal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityPoolStats {
    pub total_depositors: usize,
    pub active_depositors: usize,
    pub total_deposited: HashMap<Currency, f64>,
    pub total_liquidations: usize,
    pub total_collateral_distributed: Amount,
    pub average_deposit_size: f64,
    pub pool_utilization: f64,
    pub last_liquidation: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositorInfo {
    pub depositor: PublicKey,
    pub total_deposited: f64,
    pub deposits_by_currency: HashMap<Currency, f64>,
    pub total_rewards_btc: f64,
    pub liquidation_count: u64,
    pub total_liquidation_gains: Amount,
    pub deposit_date: DateTime<Utc>,
    pub last_claim_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationCapacityEstimate {
    pub total_debt_amount: f64,
    pub pool_can_absorb: f64,
    pub coverage_percentage: f64,
    pub capacity_by_currency: HashMap<Currency, f64>,
    pub insufficient_coverage: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};
    use bitcoin::hashes::Hash;

    #[test]
    fn test_stability_pool_creation() {
        let config = ProtocolConfig::testnet();
        let pool = StabilityPool::new(&config);
        
        assert!(pool.deposits.is_empty());
        assert_eq!(pool.pool_config.min_deposit_amount, 100.0);
    }

    #[test]
    fn test_deposit_and_withdrawal() {
        let config = ProtocolConfig::testnet();
        let mut pool = StabilityPool::new(&config);
        
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let depositor = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));

        // Test deposit
        pool.deposit(depositor, Currency::USD, 1000.0).unwrap();
        assert_eq!(pool.total_deposited.get(&Currency::USD), Some(&1000.0));

        // Test withdrawal
        let result = pool.withdraw(depositor, Currency::USD, 500.0).unwrap();
        assert_eq!(result.net_amount, 495.0); // 1% early withdrawal penalty
        assert_eq!(pool.total_deposited.get(&Currency::USD), Some(&500.0));
    }

    #[test]
    fn test_liquidation_processing() {
        let config = ProtocolConfig::testnet();
        let mut pool = StabilityPool::new(&config);
        
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let depositor = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));

        // Setup pool with deposits
        pool.deposit(depositor, Currency::USD, 10000.0).unwrap();

        // Create liquidation
        let mut liquidated_debt = HashMap::new();
        liquidated_debt.insert(Currency::USD, 1000.0);
        
        let collateral = Amount::from_btc(0.1).unwrap();
        let exchange_rates = crate::multi_currency::ExchangeRates::new();
        
        let vault_id = Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::all_zeros());
        let liquidation = pool.process_liquidation(
            vault_id,
            liquidated_debt,
            collateral,
            &exchange_rates,
        ).unwrap();

        assert_eq!(liquidation.participants.len(), 1);
        assert!(liquidation.participants[0].collateral_received > Amount::ZERO);
    }
}