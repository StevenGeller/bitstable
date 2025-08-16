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
    pub liquidation_type: LiquidationType,
    pub liquidation_percentage: f64,
}

/// Progressive liquidation types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LiquidationType {
    Partial { percentage: f64 },  // 25%, 50%, 75%
    Full,                         // 100%
    None,                         // No liquidation needed
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
            let collateral_ratio = vault.collateral_ratio(exchange_rates);
            let (liquidation_type, _) = self.determine_liquidation_type(collateral_ratio);
            
            // Include vaults that need any type of liquidation (including partial)
            if !matches!(liquidation_type, LiquidationType::None) {
                let opportunity = self.create_liquidation_opportunity(vault, exchange_rates);
                self.liquidation_queue.push(opportunity);
            }
        }
        
        log::info!("Found {} liquidation opportunities (including progressive)", self.liquidation_queue.len());
    }

    fn create_liquidation_opportunity(&self, vault: &Vault, exchange_rates: &ExchangeRates) -> LiquidationOpportunity {
        let collateral_ratio = vault.collateral_ratio(exchange_rates);
        let (liquidation_type, liquidation_percentage) = self.determine_liquidation_type(collateral_ratio);
        let bonus = vault.liquidation_bonus(exchange_rates, self.config.liquidation_penalty * liquidation_percentage);
        
        LiquidationOpportunity {
            vault_id: vault.id,
            owner: vault.owner,
            collateral: vault.collateral_btc,
            debt_usd: vault.debts.total_debt_in_usd(exchange_rates),
            collateral_ratio,
            potential_bonus: bonus,
            discovered_at: Utc::now(),
            liquidation_type,
            liquidation_percentage,
        }
    }
    
    /// Determine progressive liquidation type based on collateral ratio
    fn determine_liquidation_type(&self, collateral_ratio: f64) -> (LiquidationType, f64) {
        if collateral_ratio >= self.config.progressive_liquidation_threshold {
            (LiquidationType::None, 0.0)
        } else if collateral_ratio < self.config.liquidation_threshold {
            // Full liquidation below 125%
            (LiquidationType::Full, 1.0)
        } else if collateral_ratio < self.config.partial_liquidation_75 {
            // 75% liquidation between 125% and 125% (this tier)
            (LiquidationType::Partial { percentage: 0.75 }, 0.75)
        } else if collateral_ratio < self.config.partial_liquidation_50 {
            // 50% liquidation between 127.5% and 125%
            (LiquidationType::Partial { percentage: 0.50 }, 0.50)
        } else if collateral_ratio < self.config.partial_liquidation_25 {
            // 25% liquidation between 130% and 127.5%
            (LiquidationType::Partial { percentage: 0.25 }, 0.25)
        } else {
            (LiquidationType::None, 0.0)
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
            .ok_or(BitStableError::LiquidationThresholdNotReached)?
            .clone();

        // Verify liquidation is still valid based on progressive thresholds
        let (liquidation_type, liquidation_percentage) = self.determine_liquidation_type(opportunity.collateral_ratio);
        if matches!(liquidation_type, LiquidationType::None) {
            return Err(BitStableError::LiquidationNotPossible {
                ratio: opportunity.collateral_ratio
            });
        }

        // Calculate progressive liquidation amounts
        let debt_in_btc = opportunity.debt_usd / btc_price;
        let debt_to_cover = debt_in_btc * liquidation_percentage;
        let collateral_needed = Amount::from_btc(debt_to_cover)?;
        let bonus = Amount::from_btc(debt_to_cover * self.config.liquidation_penalty)?;
        let total_seized = collateral_needed + bonus;

        // Ensure we don't seize more than available collateral
        let max_seizeable = Amount::from_btc(opportunity.collateral.to_btc() * liquidation_percentage)?;
        let actual_seized = std::cmp::min(total_seized, max_seizeable);
        let actual_bonus = if actual_seized > collateral_needed {
            actual_seized - collateral_needed
        } else {
            Amount::ZERO
        };

        // Record the liquidation
        let record = LiquidationRecord {
            vault_id,
            liquidator,
            liquidated_at: Utc::now(),
            collateral_seized: actual_seized,
            debt_covered: debt_to_cover * btc_price,  // In USD
            bonus_paid: actual_bonus,
            final_collateral_ratio: opportunity.collateral_ratio,
        };

        // Update liquidator stats
        self.update_liquidator_stats(liquidator, actual_bonus);

        // Store liquidation record
        self.liquidation_history.push(record.clone());

        // Remove from queue if fully liquidated, otherwise update
        if matches!(liquidation_type, LiquidationType::Full) {
            self.liquidation_queue.retain(|opp| opp.vault_id != vault_id);
        } else {
            // For partial liquidations, the vault remains but with updated metrics
            // This would typically be handled by the vault manager updating the queue
        }

        log::info!(
            "Progressive liquidation of vault {} by {} ({:.1}%): Seized {} BTC, Bonus: {} BTC",
            vault_id,
            liquidator,
            liquidation_percentage * 100.0,
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