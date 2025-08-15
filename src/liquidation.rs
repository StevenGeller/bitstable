use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig, Vault, ExchangeRates, Currency, CurrencyConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationOpportunity {
    pub vault_id: Txid,
    pub owner: PublicKey,
    pub collateral: Amount,
    pub debt_usd: f64,
    pub collateral_ratio: f64,
    pub potential_bonus: Amount,
    pub discovered_at: DateTime<Utc>,
}

impl PartialEq for LiquidationOpportunity {
    fn eq(&self, other: &Self) -> bool {
        self.potential_bonus == other.potential_bonus
    }
}

impl Eq for LiquidationOpportunity {}

impl PartialOrd for LiquidationOpportunity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LiquidationOpportunity {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher bonus = higher priority
        self.potential_bonus.cmp(&other.potential_bonus)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationRecord {
    pub vault_id: Txid,
    pub liquidator: PublicKey,
    pub liquidated_at: DateTime<Utc>,
    pub collateral_seized: Amount,
    pub debt_covered: f64,
    pub bonus_paid: Amount,
    pub final_collateral_ratio: f64,
}

#[derive(Debug)]
pub struct LiquidationEngine {
    config: ProtocolConfig,
    liquidation_queue: BinaryHeap<LiquidationOpportunity>,
    liquidation_history: Vec<LiquidationRecord>,
    active_liquidators: HashMap<PublicKey, LiquidatorInfo>,
}

#[derive(Debug, Clone)]
pub struct LiquidatorInfo {
    pub pubkey: PublicKey,
    pub total_liquidations: u64,
    pub total_bonus_earned: Amount,
    pub last_liquidation: Option<DateTime<Utc>>,
}

impl LiquidationEngine {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            liquidation_queue: BinaryHeap::new(),
            liquidation_history: Vec::new(),
            active_liquidators: HashMap::new(),
        })
    }

    pub fn scan_for_liquidations(&mut self, vaults: &[&Vault], exchange_rates: &ExchangeRates) {
        self.liquidation_queue.clear();
        
        for vault in vaults {
            // Create a basic currency config map for liquidation check
            let mut currencies = HashMap::new();
            currencies.insert(Currency::USD, CurrencyConfig {
                stability_fee_apr: self.config.stability_fee_apr,
                liquidation_penalty: self.config.liquidation_penalty,
                min_collateral_ratio: self.config.min_collateral_ratio,
                liquidation_threshold: self.config.liquidation_threshold,
                min_mint_amount: 1.0,
                enabled: true,
            });
            
            if vault.is_liquidatable(exchange_rates, &currencies) {
                let opportunity = self.create_liquidation_opportunity(vault, exchange_rates);
                self.liquidation_queue.push(opportunity);
            }
        }
        
        log::info!("Found {} liquidation opportunities", self.liquidation_queue.len());
    }

    fn create_liquidation_opportunity(&self, vault: &Vault, exchange_rates: &ExchangeRates) -> LiquidationOpportunity {
        let collateral_ratio = vault.collateral_ratio(exchange_rates);
        let bonus = vault.liquidation_bonus(exchange_rates, self.config.liquidation_penalty);
        
        LiquidationOpportunity {
            vault_id: vault.id,
            owner: vault.owner,
            collateral: vault.collateral_btc,
            debt_usd: vault.debts.total_debt_in_usd(exchange_rates),
            collateral_ratio,
            potential_bonus: bonus,
            discovered_at: Utc::now(),
        }
    }

    pub async fn liquidate(
        &mut self,
        vault_id: Txid,
        liquidator: PublicKey,
        btc_price: f64,
    ) -> Result<LiquidationRecord> {
        // Find the liquidation opportunity
        let opportunity = self.liquidation_queue
            .iter()
            .find(|opp| opp.vault_id == vault_id)
            .ok_or(BitStableError::LiquidationThresholdNotReached)?;

        // Verify liquidation is still valid
        if opportunity.collateral_ratio >= self.config.liquidation_threshold {
            return Err(BitStableError::LiquidationNotPossible {
                ratio: opportunity.collateral_ratio
            });
        }

        // Calculate liquidation amounts
        let debt_in_btc = opportunity.debt_usd / btc_price;
        let collateral_needed = Amount::from_btc(debt_in_btc)?;
        let bonus = Amount::from_btc(debt_in_btc * self.config.liquidation_penalty)?;
        let total_seized = collateral_needed + bonus;

        // Ensure we don't seize more than available collateral
        let actual_seized = std::cmp::min(total_seized, opportunity.collateral);
        let actual_bonus = actual_seized - collateral_needed;

        // Record the liquidation
        let record = LiquidationRecord {
            vault_id,
            liquidator,
            liquidated_at: Utc::now(),
            collateral_seized: actual_seized,
            debt_covered: opportunity.debt_usd,
            bonus_paid: actual_bonus,
            final_collateral_ratio: opportunity.collateral_ratio,
        };

        // Update liquidator stats
        self.update_liquidator_stats(liquidator, actual_bonus);

        // Store liquidation record
        self.liquidation_history.push(record.clone());

        // Remove from queue
        self.liquidation_queue.retain(|opp| opp.vault_id != vault_id);

        log::info!(
            "Liquidated vault {} by {}. Seized: {} BTC, Bonus: {} BTC",
            vault_id,
            liquidator,
            actual_seized.to_btc(),
            actual_bonus.to_btc()
        );

        Ok(record)
    }

    fn update_liquidator_stats(&mut self, liquidator: PublicKey, bonus: Amount) {
        let stats = self.active_liquidators
            .entry(liquidator)
            .or_insert_with(|| LiquidatorInfo {
                pubkey: liquidator,
                total_liquidations: 0,
                total_bonus_earned: Amount::ZERO,
                last_liquidation: None,
            });

        stats.total_liquidations += 1;
        stats.total_bonus_earned += bonus;
        stats.last_liquidation = Some(Utc::now());
    }

    pub fn get_liquidation_opportunities(&self) -> Vec<&LiquidationOpportunity> {
        self.liquidation_queue.iter().collect()
    }

    pub fn get_best_liquidation_opportunity(&self) -> Option<&LiquidationOpportunity> {
        self.liquidation_queue.peek()
    }

    pub fn estimate_liquidation_profit(
        &self,
        vault_id: Txid,
        gas_cost_btc: Amount,
    ) -> Option<Amount> {
        let opportunity = self.liquidation_queue
            .iter()
            .find(|opp| opp.vault_id == vault_id)?;

        if opportunity.potential_bonus > gas_cost_btc {
            Some(opportunity.potential_bonus - gas_cost_btc)
        } else {
            None
        }
    }

    pub fn get_liquidator_stats(&self, liquidator: PublicKey) -> Option<&LiquidatorInfo> {
        self.active_liquidators.get(&liquidator)
    }

    pub fn get_liquidation_history(&self, limit: Option<usize>) -> Vec<&LiquidationRecord> {
        let limit = limit.unwrap_or(self.liquidation_history.len());
        let start = if self.liquidation_history.len() > limit {
            self.liquidation_history.len() - limit
        } else {
            0
        };
        self.liquidation_history[start..].iter().collect()
    }

    pub fn calculate_liquidation_health_score(&self, btc_price: f64) -> f64 {
        if self.liquidation_queue.is_empty() {
            return 1.0; // Perfect health, no liquidations needed
        }

        let total_at_risk_value: f64 = self.liquidation_queue
            .iter()
            .map(|opp| opp.collateral.to_btc() * btc_price)
            .sum();

        // Lower score means more liquidation risk
        let base_score = 1.0 - (self.liquidation_queue.len() as f64 / 100.0).min(1.0);
        let value_adjustment = (total_at_risk_value / 1_000_000.0).min(0.5); // Max 50% reduction
        
        (base_score - value_adjustment).max(0.0)
    }

    pub fn get_liquidation_statistics(&self) -> LiquidationStatistics {
        let total_liquidations = self.liquidation_history.len();
        let total_value_liquidated: f64 = self.liquidation_history
            .iter()
            .map(|record| record.debt_covered)
            .sum();
        let total_bonuses_paid = self.liquidation_history
            .iter()
            .map(|record| record.bonus_paid)
            .sum::<Amount>();

        let avg_liquidation_ratio = if total_liquidations > 0 {
            self.liquidation_history
                .iter()
                .map(|record| record.final_collateral_ratio)
                .sum::<f64>() / total_liquidations as f64
        } else {
            0.0
        };

        LiquidationStatistics {
            total_liquidations,
            total_value_liquidated,
            total_bonuses_paid,
            average_liquidation_ratio: avg_liquidation_ratio,
            active_liquidators: self.active_liquidators.len(),
            pending_liquidations: self.liquidation_queue.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationStatistics {
    pub total_liquidations: usize,
    pub total_value_liquidated: f64,
    pub total_bonuses_paid: Amount,
    pub average_liquidation_ratio: f64,
    pub active_liquidators: usize,
    pub pending_liquidations: usize,
}

pub struct LiquidatorBot {
    pub liquidator_key: PublicKey,
    pub min_profit_threshold: Amount,
    pub max_gas_price: Amount,
    pub target_collateral_ratio: f64,
}

impl LiquidatorBot {
    pub fn new(
        liquidator_key: PublicKey,
        min_profit_threshold: Amount,
        max_gas_price: Amount,
    ) -> Self {
        Self {
            liquidator_key,
            min_profit_threshold,
            max_gas_price,
            target_collateral_ratio: 1.05, // Target vaults below 105% ratio
        }
    }

    pub fn should_liquidate(&self, opportunity: &LiquidationOpportunity) -> bool {
        opportunity.collateral_ratio < self.target_collateral_ratio
            && opportunity.potential_bonus > self.min_profit_threshold + self.max_gas_price
    }

    pub fn select_best_opportunities(
        &self,
        opportunities: &[&LiquidationOpportunity],
        max_count: usize,
    ) -> Vec<Txid> {
        let mut profitable: Vec<_> = opportunities
            .iter()
            .filter(|opp| self.should_liquidate(opp))
            .collect();

        // Sort by potential profit (bonus - gas cost)
        profitable.sort_by(|a, b| {
            let profit_a = if a.potential_bonus > self.max_gas_price {
                a.potential_bonus - self.max_gas_price
            } else {
                Amount::ZERO
            };
            let profit_b = if b.potential_bonus > self.max_gas_price {
                b.potential_bonus - self.max_gas_price
            } else {
                Amount::ZERO
            };
            profit_b.cmp(&profit_a)
        });

        profitable
            .into_iter()
            .take(max_count)
            .map(|opp| opp.vault_id)
            .collect()
    }
}