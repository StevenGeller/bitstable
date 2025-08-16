use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::cmp::Ordering;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig, Vault, ExchangeRates};

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
    // Cascade prevention mechanisms
    hourly_liquidation_volume: HashMap<Txid, f64>,  // vault_id -> volume liquidated this hour
    block_liquidation_volume: f64,                  // total volume liquidated this block
    last_block_reset: DateTime<Utc>,               // when block volume was last reset
    emergency_halt_until: Option<DateTime<Utc>>,   // emergency trading halt timestamp
    cascade_detection: CascadeDetectionSystem,
}

#[derive(Debug, Clone)]
pub struct LiquidatorInfo {
    pub pubkey: PublicKey,
    pub total_liquidations: u64,
    pub total_bonus_earned: Amount,
    pub last_liquidation: Option<DateTime<Utc>>,
}

/// Cascade detection and prevention system
#[derive(Debug, Clone)]
pub struct CascadeDetectionSystem {
    pub liquidation_rate_10min: f64,     // % of system collateral liquidated in 10 min
    pub liquidation_events_10min: VecDeque<(DateTime<Utc>, f64)>, // timestamp, volume
    pub system_collateral_total: f64,     // total system collateral for % calculation
    pub emergency_threshold: f64,         // 20% liquidation in 10 min triggers halt
    pub max_block_liquidation: f64,       // 10% max per block
    pub max_vault_liquidation_per_hour: f64, // 50% max vault liquidation per hour
}

impl LiquidationEngine {
    pub fn new(config: &ProtocolConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            liquidation_queue: BinaryHeap::new(),
            liquidation_history: Vec::new(),
            active_liquidators: HashMap::new(),
            hourly_liquidation_volume: HashMap::new(),
            block_liquidation_volume: 0.0,
            last_block_reset: Utc::now(),
            emergency_halt_until: None,
            cascade_detection: CascadeDetectionSystem {
                liquidation_rate_10min: 0.0,
                liquidation_events_10min: VecDeque::new(),
                system_collateral_total: 0.0,
                emergency_threshold: 0.20, // 20%
                max_block_liquidation: 0.10, // 10%
                max_vault_liquidation_per_hour: 0.50, // 50%
            },
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
        // Check emergency halt
        if let Some(halt_until) = self.emergency_halt_until {
            if Utc::now() < halt_until {
                return Err(BitStableError::InvalidConfig(
                    "Trading halted due to liquidation cascade".to_string()
                ));
            } else {
                self.emergency_halt_until = None; // Clear expired halt
            }
        }
        
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
        
        // Calculate liquidation volume in USD
        let liquidation_volume_usd = opportunity.debt_usd * liquidation_percentage;
        
        // Check rate limiting constraints
        self.check_rate_limits(vault_id, liquidation_volume_usd, liquidation_percentage)?;

        // Apply dynamic liquidation penalty (smoothing function)
        let dynamic_penalty = self.calculate_dynamic_penalty(liquidation_volume_usd);
        
        // Calculate progressive liquidation amounts
        let debt_in_btc = opportunity.debt_usd / btc_price;
        let debt_to_cover = debt_in_btc * liquidation_percentage;
        let collateral_needed = Amount::from_btc(debt_to_cover)?;
        let bonus = Amount::from_btc(debt_to_cover * dynamic_penalty)?;
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

        // Update cascade detection and tracking
        self.update_cascade_tracking(liquidation_volume_usd)?;
        
        // Update liquidator stats
        self.update_liquidator_stats(liquidator, actual_bonus);

        // Store liquidation record
        self.liquidation_history.push(record.clone());
        
        // Check for cascade trigger after this liquidation
        self.check_cascade_emergency_trigger()?;

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
// Additional implementation for LiquidationEngine cascade prevention
impl LiquidationEngine {
    /// Check rate limiting constraints before liquidation
    fn check_rate_limits(
        &mut self,
        vault_id: Txid,
        liquidation_volume_usd: f64,
        liquidation_percentage: f64,
    ) -> Result<()> {
        // Update block volume tracking
        self.update_block_tracking();
        
        // Check block-level liquidation limit (10% of system collateral)
        let proposed_block_volume = self.block_liquidation_volume + liquidation_volume_usd;
        let max_block_volume = self.cascade_detection.system_collateral_total * self.cascade_detection.max_block_liquidation;
        
        if proposed_block_volume > max_block_volume {
            return Err(BitStableError::InvalidConfig(
                format!("Block liquidation limit exceeded: {:.2}% of system collateral", 
                       proposed_block_volume / self.cascade_detection.system_collateral_total * 100.0)
            ));
        }
        
        // Check vault-level hourly limit (50% per hour)
        let hourly_volume = self.hourly_liquidation_volume.get(&vault_id).copied().unwrap_or(0.0);
        if liquidation_percentage > self.cascade_detection.max_vault_liquidation_per_hour - hourly_volume {
            return Err(BitStableError::InvalidConfig(
                format!("Vault hourly liquidation limit exceeded: {:.1}% already liquidated this hour",
                       hourly_volume * 100.0)
            ));
        }
        
        Ok(())
    }
    
    /// Calculate dynamic liquidation penalty with smoothing function
    fn calculate_dynamic_penalty(&self, liquidation_volume_usd: f64) -> f64 {
        let base_penalty = self.config.liquidation_penalty; // 5%
        let volume_threshold = 1_000_000.0; // $1M threshold
        let k = 0.02; // Scaling factor
        
        // γ(V) = γ_base + k × log(1 + V/V_threshold)
        let volume_factor = (1.0 + liquidation_volume_usd / volume_threshold).ln();
        let dynamic_penalty = base_penalty + k * volume_factor;
        
        // Cap penalty at 15% to prevent excessive liquidation costs
        dynamic_penalty.min(0.15)
    }
    
    /// Update block-level liquidation tracking
    fn update_block_tracking(&mut self) {
        let now = Utc::now();
        
        // Reset block volume every minute (simplified block time)
        if now.signed_duration_since(self.last_block_reset).num_minutes() >= 1 {
            self.block_liquidation_volume = 0.0;
            self.last_block_reset = now;
        }
    }
    
    /// Update cascade detection tracking
    fn update_cascade_tracking(&mut self, liquidation_volume_usd: f64) -> Result<()> {
        let now = Utc::now();
        
        // Add to block volume
        self.block_liquidation_volume += liquidation_volume_usd;
        
        // Add to 10-minute tracking window
        self.cascade_detection.liquidation_events_10min.push_back((now, liquidation_volume_usd));
        
        // Remove events older than 10 minutes
        let cutoff = now - chrono::Duration::minutes(10);
        while let Some((timestamp, _)) = self.cascade_detection.liquidation_events_10min.front() {
            if *timestamp < cutoff {
                self.cascade_detection.liquidation_events_10min.pop_front();
            } else {
                break;
            }
        }
        
        // Calculate 10-minute liquidation rate
        let total_10min_volume: f64 = self.cascade_detection.liquidation_events_10min
            .iter()
            .map(|(_, volume)| volume)
            .sum();
        
        self.cascade_detection.liquidation_rate_10min = 
            total_10min_volume / self.cascade_detection.system_collateral_total;
        
        log::info!("10-minute liquidation rate: {:.2}%", 
                  self.cascade_detection.liquidation_rate_10min * 100.0);
        
        Ok(())
    }
    
    /// Check if emergency halt should be triggered
    fn check_cascade_emergency_trigger(&mut self) -> Result<()> {
        if self.cascade_detection.liquidation_rate_10min > self.cascade_detection.emergency_threshold {
            // Trigger 1-hour emergency halt
            self.emergency_halt_until = Some(Utc::now() + chrono::Duration::hours(1));
            
            log::error!(
                "EMERGENCY LIQUIDATION HALT: {:.2}% of system collateral liquidated in 10 minutes",
                self.cascade_detection.liquidation_rate_10min * 100.0
            );
            
            return Err(BitStableError::InvalidConfig(
                "Emergency trading halt triggered due to liquidation cascade".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Update system collateral total for percentage calculations
    pub fn update_system_collateral(&mut self, total_collateral_usd: f64) {
        self.cascade_detection.system_collateral_total = total_collateral_usd;
    }
    
    /// Get cascade prevention statistics
    pub fn get_cascade_stats(&self) -> CascadePreventionStats {
        let time_until_halt_lift = self.emergency_halt_until
            .map(|halt_time| halt_time.signed_duration_since(Utc::now()))
            .filter(|duration| duration.num_seconds() > 0);
        
        CascadePreventionStats {
            liquidation_rate_10min: self.cascade_detection.liquidation_rate_10min,
            block_liquidation_volume: self.block_liquidation_volume,
            system_collateral_total: self.cascade_detection.system_collateral_total,
            emergency_halt_active: self.emergency_halt_until.is_some() && 
                                  self.emergency_halt_until.unwrap() > Utc::now(),
            time_until_halt_lift,
            emergency_threshold: self.cascade_detection.emergency_threshold,
            recent_liquidation_events: self.cascade_detection.liquidation_events_10min.len(),
        }
    }
    
    /// Manually trigger emergency halt (governance override)
    pub fn trigger_emergency_halt(&mut self, duration_hours: i64) {
        self.emergency_halt_until = Some(Utc::now() + chrono::Duration::hours(duration_hours));
        log::warn!("Manual emergency halt triggered for {} hours", duration_hours);
    }
    
    /// Clear emergency halt (governance override)
    pub fn clear_emergency_halt(&mut self) {
        self.emergency_halt_until = None;
        log::info!("Emergency halt cleared by governance");
    }
}

/// Statistics for cascade prevention system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadePreventionStats {
    pub liquidation_rate_10min: f64,
    pub block_liquidation_volume: f64,
    pub system_collateral_total: f64,
    pub emergency_halt_active: bool,
    pub time_until_halt_lift: Option<chrono::Duration>,
    pub emergency_threshold: f64,
    pub recent_liquidation_events: usize,
}
