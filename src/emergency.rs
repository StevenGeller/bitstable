use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use crate::{BitStableError, Result, ProtocolConfig};
use crate::multi_currency::{Currency, ExchangeRates};
use crate::governance::{GovernanceSystem, ProposalType};

/// Emergency shutdown system for protocol-wide crisis management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyShutdownSystem {
    pub shutdown_state: ShutdownState,
    pub shutdown_triggers: Vec<ShutdownTrigger>,
    pub emergency_config: EmergencyConfig,
    pub shutdown_history: Vec<ShutdownEvent>,
    pub asset_redemption_pool: HashMap<Currency, f64>,
    pub collateral_redemption_pool: Amount,
    pub user_claims: HashMap<PublicKey, UserClaim>,
    pub governance_override_active: bool,
    pub last_health_check: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShutdownState {
    Normal,
    AlertLevel1,        // Warning state
    AlertLevel2,        // High alert
    AlertLevel3,        // Critical alert
    EmergencyShutdown,  // Full shutdown
    SettlementMode,     // Post-shutdown settlement
    Resolved,           // Crisis resolved
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownTrigger {
    pub trigger_type: TriggerType,
    pub threshold: f64,
    pub current_value: f64,
    pub triggered: bool,
    pub triggered_at: Option<DateTime<Utc>>,
    pub alert_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    SystemCollateralizationRatio,
    OracleFailureRate,
    LiquidationCascadeSize,
    InsuranceFundDepletion,
    GovernanceDeadlock,
    SecurityBreach,
    RegulatoryShutdown,
    BlackSwanEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyConfig {
    pub auto_shutdown_threshold: f64,    // CR below which auto-shutdown triggers
    pub manual_override_required: bool,   // Require governance for shutdown
    pub settlement_period_days: u64,     // Time for users to claim assets
    pub emergency_contacts: Vec<String>, // Emergency contact addresses
    pub pause_operations: bool,           // Pause all operations except redemption
    pub oracle_failure_threshold: f64,   // % of oracles failing triggers alert
    pub max_liquidation_size: f64,       // Max liquidation size before alert
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownEvent {
    pub event_type: ShutdownEventType,
    pub timestamp: DateTime<Utc>,
    pub trigger_reason: String,
    pub system_state_snapshot: SystemStateSnapshot,
    pub governance_proposal_id: Option<u64>,
    pub triggered_by: Option<PublicKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShutdownEventType {
    AlertRaised,
    AlertEscalated,
    EmergencyShutdownTriggered,
    SettlementBegan,
    SystemRestored,
    ManualOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStateSnapshot {
    pub timestamp: DateTime<Utc>,
    pub system_collateral_ratio: f64,
    pub total_debt_usd: f64,
    pub total_collateral_btc: f64,
    pub active_vaults: usize,
    pub oracle_failures: usize,
    pub insurance_fund_balance: Amount,
    pub stability_pool_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaim {
    pub user: PublicKey,
    pub stable_balances: HashMap<Currency, f64>,
    pub vault_collateral: HashMap<Txid, Amount>,
    pub stability_pool_deposits: HashMap<Currency, f64>,
    pub total_claim_value_usd: f64,
    pub claim_submitted: bool,
    pub claim_processed: bool,
    pub payout_amount: Amount,
    pub payout_timestamp: Option<DateTime<Utc>>,
}

impl EmergencyShutdownSystem {
    pub fn new(_config: &ProtocolConfig) -> Self {
        let emergency_config = EmergencyConfig {
            auto_shutdown_threshold: 1.05,     // 105% system CR
            manual_override_required: true,
            settlement_period_days: 30,        // 30 days to claim
            emergency_contacts: vec![
                "emergency@bitstable.org".to_string(),
                "crisis@bitstable.org".to_string(),
            ],
            pause_operations: false,
            oracle_failure_threshold: 0.4,    // 40% oracle failure rate
            max_liquidation_size: 10_000_000.0, // $10M liquidation triggers alert
        };

        let shutdown_triggers = vec![
            ShutdownTrigger {
                trigger_type: TriggerType::SystemCollateralizationRatio,
                threshold: 1.20,
                current_value: 0.0,
                triggered: false,
                triggered_at: None,
                alert_level: 1,
            },
            ShutdownTrigger {
                trigger_type: TriggerType::OracleFailureRate,
                threshold: 0.3,
                current_value: 0.0,
                triggered: false,
                triggered_at: None,
                alert_level: 2,
            },
            ShutdownTrigger {
                trigger_type: TriggerType::InsuranceFundDepletion,
                threshold: 0.2, // 20% of original fund remaining
                current_value: 0.5, // Start with 50% instead of 100%
                triggered: false,
                triggered_at: None,
                alert_level: 2,
            },
        ];

        Self {
            shutdown_state: ShutdownState::Normal,
            shutdown_triggers,
            emergency_config,
            shutdown_history: Vec::new(),
            asset_redemption_pool: HashMap::new(),
            collateral_redemption_pool: Amount::ZERO,
            user_claims: HashMap::new(),
            governance_override_active: false,
            last_health_check: Utc::now(),
        }
    }

    /// Perform system health check and update triggers
    pub fn check_system_health(
        &mut self,
        system_state: SystemStateSnapshot,
        governance_system: &mut GovernanceSystem,
    ) -> Result<Vec<AlertAction>> {
        self.last_health_check = Utc::now();
        let mut actions = Vec::new();

        // Collect trigger changes first
        let mut triggered_items = Vec::new();
        let mut resolved_items = Vec::new();
        
        // Update trigger values
        for trigger in &mut self.shutdown_triggers {
            let _previous_value = trigger.current_value;
            
            trigger.current_value = match trigger.trigger_type {
                TriggerType::SystemCollateralizationRatio => system_state.system_collateral_ratio,
                TriggerType::OracleFailureRate => {
                    if system_state.oracle_failures > 0 {
                        system_state.oracle_failures as f64 / 5.0 // Assuming 5 oracles
                    } else {
                        0.0
                    }
                },
                TriggerType::InsuranceFundDepletion => {
                    // This would need integration with actual insurance fund data
                    0.1 // Placeholder - below threshold so not triggering
                },
                TriggerType::LiquidationCascadeSize => {
                    // Would need integration with liquidation engine
                    0.0 // Placeholder
                },
                _ => trigger.current_value, // Keep existing values for other types
            };

            // Check if trigger conditions are met
            let should_trigger = match trigger.trigger_type {
                TriggerType::SystemCollateralizationRatio => {
                    trigger.current_value < trigger.threshold
                },
                TriggerType::OracleFailureRate | TriggerType::InsuranceFundDepletion => {
                    trigger.current_value > trigger.threshold
                },
                _ => false,
            };

            if should_trigger && !trigger.triggered {
                trigger.triggered = true;
                trigger.triggered_at = Some(Utc::now());
                triggered_items.push(trigger.clone());
            } else if !should_trigger && trigger.triggered {
                // Trigger resolved
                trigger.triggered = false;
                trigger.triggered_at = None;
                resolved_items.push(trigger.clone());
            }
        }
        
        // Handle triggered items
        for trigger in triggered_items {
            actions.push(self.handle_trigger_activation(
                trigger,
                system_state.clone(),
                governance_system,
            )?);
        }
        
        // Handle resolved items
        for trigger in resolved_items {
            actions.push(AlertAction::TriggerResolved {
                trigger_type: trigger.trigger_type.clone(),
                resolved_at: Utc::now(),
            });
        }

        // Update shutdown state based on active triggers
        self.update_shutdown_state(&system_state)?;

        Ok(actions)
    }

    /// Handle trigger activation
    fn handle_trigger_activation(
        &mut self,
        trigger: ShutdownTrigger,
        system_state: SystemStateSnapshot,
        governance_system: &mut GovernanceSystem,
    ) -> Result<AlertAction> {
        let alert_action = match trigger.alert_level {
            1 => {
                self.escalate_to_alert_level_1(&trigger, &system_state)?
            },
            2 => {
                self.escalate_to_alert_level_2(&trigger, &system_state)?
            },
            3 => {
                self.escalate_to_emergency_shutdown(&trigger, &system_state, governance_system)?
            },
            _ => AlertAction::MonitoringOnly,
        };

        let event = ShutdownEvent {
            event_type: ShutdownEventType::AlertRaised,
            timestamp: Utc::now(),
            trigger_reason: format!("{:?} triggered with value {}", trigger.trigger_type, trigger.current_value),
            system_state_snapshot: system_state,
            governance_proposal_id: None,
            triggered_by: None,
        };

        self.shutdown_history.push(event);

        Ok(alert_action)
    }

    /// Escalate to alert level 1
    fn escalate_to_alert_level_1(
        &mut self,
        trigger: &ShutdownTrigger,
        _system_state: &SystemStateSnapshot,
    ) -> Result<AlertAction> {
        if self.shutdown_state == ShutdownState::Normal {
            self.shutdown_state = ShutdownState::AlertLevel1;
        }

        Ok(AlertAction::AlertLevel1 {
            trigger_type: trigger.trigger_type.clone(),
            message: format!("Alert Level 1: {:?} threshold breached", trigger.trigger_type),
            recommended_actions: vec![
                "Monitor system closely".to_string(),
                "Prepare contingency plans".to_string(),
                "Increase oracle monitoring".to_string(),
            ],
        })
    }

    /// Escalate to alert level 2
    fn escalate_to_alert_level_2(
        &mut self,
        trigger: &ShutdownTrigger,
        _system_state: &SystemStateSnapshot,
    ) -> Result<AlertAction> {
        if matches!(self.shutdown_state, ShutdownState::Normal | ShutdownState::AlertLevel1) {
            self.shutdown_state = ShutdownState::AlertLevel2;
        }

        Ok(AlertAction::AlertLevel2 {
            trigger_type: trigger.trigger_type.clone(),
            message: format!("Alert Level 2: Critical threshold breached for {:?}", trigger.trigger_type),
            emergency_contacts_notified: self.emergency_config.emergency_contacts.clone(),
            immediate_actions: vec![
                "Notify emergency contacts".to_string(),
                "Prepare for possible shutdown".to_string(),
                "Activate emergency procedures".to_string(),
            ],
        })
    }

    /// Escalate to emergency shutdown
    fn escalate_to_emergency_shutdown(
        &mut self,
        trigger: &ShutdownTrigger,
        system_state: &SystemStateSnapshot,
        governance_system: &mut GovernanceSystem,
    ) -> Result<AlertAction> {
        if self.emergency_config.manual_override_required && !self.governance_override_active {
            // Create emergency governance proposal
            let proposal_id = governance_system.create_proposal(
                governance_system.keyholders[0].pubkey, // Emergency proposer
                ProposalType::EmergencyShutdown {
                    reason: format!("Automatic trigger: {:?}", trigger.trigger_type),
                },
                "Emergency System Shutdown".to_string(),
                format!("Emergency shutdown triggered by {:?} threshold breach", trigger.trigger_type),
                true, // Emergency proposal
            )?;

            Ok(AlertAction::GovernanceProposalCreated {
                proposal_id,
                trigger_type: trigger.trigger_type.clone(),
                requires_immediate_vote: true,
            })
        } else {
            // Execute immediate shutdown
            self.execute_emergency_shutdown(
                format!("Automatic trigger: {:?}", trigger.trigger_type),
                Some(system_state.clone()),
                None,
            )?;

            Ok(AlertAction::EmergencyShutdownExecuted {
                trigger_type: trigger.trigger_type.clone(),
                shutdown_reason: format!("Automatic trigger: {:?}", trigger.trigger_type),
                settlement_deadline: Utc::now() + Duration::days(self.emergency_config.settlement_period_days as i64),
            })
        }
    }

    /// Execute emergency shutdown
    pub fn execute_emergency_shutdown(
        &mut self,
        reason: String,
        system_state: Option<SystemStateSnapshot>,
        triggered_by: Option<PublicKey>,
    ) -> Result<()> {
        self.shutdown_state = ShutdownState::EmergencyShutdown;

        let event = ShutdownEvent {
            event_type: ShutdownEventType::EmergencyShutdownTriggered,
            timestamp: Utc::now(),
            trigger_reason: reason,
            system_state_snapshot: system_state.unwrap_or_else(|| self.create_current_snapshot()),
            governance_proposal_id: None,
            triggered_by,
        };

        self.shutdown_history.push(event);

        // Begin settlement mode preparation
        self.prepare_settlement_mode()?;

        log::error!("EMERGENCY SHUTDOWN EXECUTED: {}", self.shutdown_history.last().unwrap().trigger_reason);

        Ok(())
    }

    /// Prepare settlement mode for user claims
    fn prepare_settlement_mode(&mut self) -> Result<()> {
        self.shutdown_state = ShutdownState::SettlementMode;

        let event = ShutdownEvent {
            event_type: ShutdownEventType::SettlementBegan,
            timestamp: Utc::now(),
            trigger_reason: "Settlement mode initiated".to_string(),
            system_state_snapshot: self.create_current_snapshot(),
            governance_proposal_id: None,
            triggered_by: None,
        };

        self.shutdown_history.push(event);

        log::info!("Settlement mode activated. Users have {} days to submit claims.", 
                  self.emergency_config.settlement_period_days);

        Ok(())
    }

    /// Submit user claim for settlement
    pub fn submit_user_claim(
        &mut self,
        user: PublicKey,
        stable_balances: HashMap<Currency, f64>,
        vault_collateral: HashMap<Txid, Amount>,
        stability_pool_deposits: HashMap<Currency, f64>,
        exchange_rates: &ExchangeRates,
    ) -> Result<ClaimResult> {
        if self.shutdown_state != ShutdownState::SettlementMode {
            return Err(BitStableError::InvalidConfig("System not in settlement mode".to_string()));
        }

        // Calculate total claim value in USD
        let mut total_value_usd = 0.0;

        // Add stable coin values
        for (currency, amount) in &stable_balances {
            let rate = exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
            total_value_usd += amount * rate;
        }

        // Add vault collateral values
        let btc_price = exchange_rates.get_btc_price(&Currency::USD).unwrap_or(50000.0);
        for (_, collateral) in &vault_collateral {
            total_value_usd += collateral.to_btc() * btc_price;
        }

        // Add stability pool deposits
        for (currency, amount) in &stability_pool_deposits {
            let rate = exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0);
            total_value_usd += amount * rate;
        }

        let claim = UserClaim {
            user,
            stable_balances,
            vault_collateral,
            stability_pool_deposits,
            total_claim_value_usd: total_value_usd,
            claim_submitted: true,
            claim_processed: false,
            payout_amount: Amount::ZERO,
            payout_timestamp: None,
        };

        self.user_claims.insert(user, claim.clone());

        log::info!("User claim submitted: {} claiming ${:.2}", user, total_value_usd);

        Ok(ClaimResult {
            user,
            claim_id: user, // Using pubkey as claim ID
            total_claim_value_usd: total_value_usd,
            estimated_payout_btc: Amount::from_btc(total_value_usd / btc_price).unwrap_or(Amount::ZERO),
            claim_submitted_at: Utc::now(),
            expected_processing_time: Duration::days(7),
        })
    }

    /// Process user claims and calculate payouts
    pub fn process_claims(&mut self, available_collateral: Amount) -> Result<Vec<ClaimPayout>> {
        if self.shutdown_state != ShutdownState::SettlementMode {
            return Err(BitStableError::InvalidConfig("System not in settlement mode".to_string()));
        }

        let mut payouts = Vec::new();
        let total_claims_usd: f64 = self.user_claims.values()
            .map(|c| c.total_claim_value_usd)
            .sum();

        if total_claims_usd == 0.0 {
            return Ok(payouts);
        }

        // Calculate pro-rata distribution
        let available_btc = available_collateral.to_btc();
        
        for (user, claim) in &mut self.user_claims {
            if claim.claim_submitted && !claim.claim_processed {
                let payout_ratio = claim.total_claim_value_usd / total_claims_usd;
                let payout_btc = available_btc * payout_ratio;
                let payout_amount = Amount::from_btc(payout_btc)
                    .map_err(|e| BitStableError::InvalidConfig(format!("Invalid payout amount: {}", e)))?;

                claim.payout_amount = payout_amount;
                claim.claim_processed = true;
                claim.payout_timestamp = Some(Utc::now());

                payouts.push(ClaimPayout {
                    user: *user,
                    original_claim_usd: claim.total_claim_value_usd,
                    payout_amount,
                    payout_ratio,
                    processed_at: Utc::now(),
                });
            }
        }

        log::info!("Processed {} claims with total payout of {} BTC", payouts.len(), available_btc);

        Ok(payouts)
    }

    /// Update shutdown state based on triggers
    fn update_shutdown_state(&mut self, _system_state: &SystemStateSnapshot) -> Result<()> {
        let active_level_3 = self.shutdown_triggers.iter()
            .any(|t| t.triggered && t.alert_level >= 3);
        let active_level_2 = self.shutdown_triggers.iter()
            .any(|t| t.triggered && t.alert_level >= 2);
        let active_level_1 = self.shutdown_triggers.iter()
            .any(|t| t.triggered && t.alert_level >= 1);

        // Don't downgrade from emergency states
        if matches!(self.shutdown_state, ShutdownState::EmergencyShutdown | ShutdownState::SettlementMode) {
            return Ok(());
        }

        if active_level_3 {
            self.shutdown_state = ShutdownState::AlertLevel3;
        } else if active_level_2 {
            self.shutdown_state = ShutdownState::AlertLevel2;
        } else if active_level_1 {
            self.shutdown_state = ShutdownState::AlertLevel1;
        } else {
            self.shutdown_state = ShutdownState::Normal;
        }

        Ok(())
    }

    /// Create current system snapshot
    fn create_current_snapshot(&self) -> SystemStateSnapshot {
        // This would integrate with actual system state
        SystemStateSnapshot {
            timestamp: Utc::now(),
            system_collateral_ratio: 1.5,
            total_debt_usd: 1_000_000.0,
            total_collateral_btc: 20.0,
            active_vaults: 100,
            oracle_failures: 0,
            insurance_fund_balance: Amount::from_btc(5.0).unwrap_or(Amount::ZERO),
            stability_pool_size: 500_000.0,
        }
    }

    /// Get emergency system status
    pub fn get_emergency_status(&self) -> EmergencyStatus {
        let active_triggers: Vec<&ShutdownTrigger> = self.shutdown_triggers.iter()
            .filter(|t| t.triggered)
            .collect();

        let time_since_last_check = Utc::now()
            .signed_duration_since(self.last_health_check)
            .num_minutes();

        EmergencyStatus {
            current_state: self.shutdown_state.clone(),
            active_triggers: active_triggers.len(),
            triggered_systems: active_triggers.iter().map(|t| t.trigger_type.clone()).collect(),
            governance_override_active: self.governance_override_active,
            settlement_active: self.shutdown_state == ShutdownState::SettlementMode,
            pending_claims: self.user_claims.len(),
            processed_claims: self.user_claims.values().filter(|c| c.claim_processed).count(),
            last_health_check: self.last_health_check,
            health_check_status: if time_since_last_check < 5 { "Current" } else { "Stale" }.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertAction {
    MonitoringOnly,
    AlertLevel1 {
        trigger_type: TriggerType,
        message: String,
        recommended_actions: Vec<String>,
    },
    AlertLevel2 {
        trigger_type: TriggerType,
        message: String,
        emergency_contacts_notified: Vec<String>,
        immediate_actions: Vec<String>,
    },
    GovernanceProposalCreated {
        proposal_id: u64,
        trigger_type: TriggerType,
        requires_immediate_vote: bool,
    },
    EmergencyShutdownExecuted {
        trigger_type: TriggerType,
        shutdown_reason: String,
        settlement_deadline: DateTime<Utc>,
    },
    TriggerResolved {
        trigger_type: TriggerType,
        resolved_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResult {
    pub user: PublicKey,
    pub claim_id: PublicKey,
    pub total_claim_value_usd: f64,
    pub estimated_payout_btc: Amount,
    pub claim_submitted_at: DateTime<Utc>,
    pub expected_processing_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimPayout {
    pub user: PublicKey,
    pub original_claim_usd: f64,
    pub payout_amount: Amount,
    pub payout_ratio: f64,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyStatus {
    pub current_state: ShutdownState,
    pub active_triggers: usize,
    pub triggered_systems: Vec<TriggerType>,
    pub governance_override_active: bool,
    pub settlement_active: bool,
    pub pending_claims: usize,
    pub processed_claims: usize,
    pub last_health_check: DateTime<Utc>,
    pub health_check_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;

    #[test]
    fn test_emergency_system_creation() {
        let config = ProtocolConfig::testnet();
        let emergency_system = EmergencyShutdownSystem::new(&config);
        
        assert_eq!(emergency_system.shutdown_state, ShutdownState::Normal);
        assert_eq!(emergency_system.shutdown_triggers.len(), 3);
        assert!(emergency_system.user_claims.is_empty());
    }

    #[test]
    fn test_trigger_activation() {
        let config = ProtocolConfig::testnet();
        let mut emergency_system = EmergencyShutdownSystem::new(&config);
        
        let system_state = SystemStateSnapshot {
            timestamp: Utc::now(),
            system_collateral_ratio: 1.15, // Below 1.20 threshold
            total_debt_usd: 1_000_000.0,
            total_collateral_btc: 20.0,
            active_vaults: 100,
            oracle_failures: 0,
            insurance_fund_balance: Amount::ZERO,
            stability_pool_size: 500_000.0,
        };

        let mut governance = crate::governance::GovernanceSystem::new();
        let actions = emergency_system.check_system_health(system_state, &mut governance).unwrap();
        
        assert!(!actions.is_empty());
        assert_eq!(emergency_system.shutdown_state, ShutdownState::AlertLevel1);
    }
}