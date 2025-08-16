use bitcoin::{Amount, PublicKey, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::{BitStableError, Result, ProtocolConfig};
use crate::multi_currency::{Currency, ExchangeRates};

/// Protocol insurance fund for handling black swan events and system recapitalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceFund {
    pub balance_btc: Amount,
    pub balance_stable: HashMap<Currency, f64>,
    pub total_contributions: Amount,
    pub total_payouts: Amount,
    pub fee_percentage: f64,           // % of protocol fees that go to insurance
    pub emergency_threshold: f64,      // CR below which fund activates
    pub contribution_history: Vec<InsuranceContribution>,
    pub payout_history: Vec<InsurancePayout>,
    pub last_health_check: DateTime<Utc>,
    pub governance_token_minted: u64,  // Emergency governance tokens minted as last resort
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceContribution {
    pub amount: Amount,
    pub source: ContributionSource,
    pub timestamp: DateTime<Utc>,
    pub transaction_id: Option<Txid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsurancePayout {
    pub amount: Amount,
    pub payout_type: PayoutType,
    pub recipient: Option<PublicKey>,
    pub vault_id: Option<Txid>,
    pub timestamp: DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContributionSource {
    ProtocolFees,
    LiquidationPenalties,
    GovernanceDecision,
    EmergencyContribution,
    StabilityFees,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PayoutType {
    VaultRecapitalization,    // Cover undercollateralized vaults
    SystemDeficit,            // Cover system-wide shortfall
    EmergencyShutdown,        // Compensate users during emergency shutdown
    BadDebtCover,             // Cover bad debt from liquidations
    OracleFailureCompensation, // Compensate for oracle failures
}

impl InsuranceFund {
    pub fn new(config: &ProtocolConfig) -> Self {
        Self {
            balance_btc: Amount::ZERO,
            balance_stable: HashMap::new(),
            total_contributions: Amount::ZERO,
            total_payouts: Amount::ZERO,
            fee_percentage: config.insurance_fund_fee_rate,
            emergency_threshold: 1.05,  // 105% system-wide CR triggers emergency
            contribution_history: Vec::new(),
            payout_history: Vec::new(),
            last_health_check: Utc::now(),
            governance_token_minted: 0,
        }
    }

    /// Contribute protocol fees to the insurance fund
    pub fn contribute_from_fees(&mut self, protocol_fees: Amount, source: ContributionSource) -> Result<()> {
        let contribution_amount = Amount::from_btc(protocol_fees.to_btc() * self.fee_percentage)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid contribution amount: {}", e)))?;

        self.balance_btc += contribution_amount;
        self.total_contributions += contribution_amount;

        let contribution = InsuranceContribution {
            amount: contribution_amount,
            source,
            timestamp: Utc::now(),
            transaction_id: None,
        };

        self.contribution_history.push(contribution);

        log::info!(
            "Insurance fund contribution: {} BTC (new balance: {} BTC)",
            contribution_amount.to_btc(),
            self.balance_btc.to_btc()
        );

        Ok(())
    }

    /// Check if emergency recapitalization is needed based on system health
    pub fn check_emergency_conditions(
        &mut self,
        system_collateral_ratio: f64,
        total_system_debt_usd: f64,
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<EmergencyAction>> {
        self.last_health_check = Utc::now();

        if system_collateral_ratio < self.emergency_threshold {
            let btc_price = exchange_rates.get_btc_price(&Currency::USD)
                .ok_or_else(|| BitStableError::PriceFeedError("USD price not available".to_string()))?;

            let deficit_usd = total_system_debt_usd * (self.emergency_threshold - system_collateral_ratio);
            let deficit_btc = Amount::from_btc(deficit_usd / btc_price)
                .map_err(|e| BitStableError::InvalidConfig(format!("Invalid deficit amount: {}", e)))?;

            if self.balance_btc >= deficit_btc {
                // Fund can cover the deficit
                Ok(Some(EmergencyAction::FundRecapitalization {
                    deficit_amount: deficit_btc,
                    current_fund_balance: self.balance_btc,
                }))
            } else {
                // Need governance token minting
                Ok(Some(EmergencyAction::GovernanceTokenMinting {
                    deficit_amount: deficit_btc,
                    available_fund: self.balance_btc,
                    tokens_to_mint: self.calculate_governance_tokens_needed(deficit_btc),
                }))
            }
        } else {
            Ok(None)
        }
    }

    /// Execute emergency recapitalization
    pub fn execute_emergency_recapitalization(
        &mut self,
        deficit_amount: Amount,
        reason: String,
    ) -> Result<InsurancePayout> {
        if deficit_amount > self.balance_btc {
            return Err(BitStableError::InsufficientCollateral {
                required: deficit_amount.to_btc(),
                provided: self.balance_btc.to_btc(),
            });
        }

        self.balance_btc -= deficit_amount;
        self.total_payouts += deficit_amount;

        let payout = InsurancePayout {
            amount: deficit_amount,
            payout_type: PayoutType::SystemDeficit,
            recipient: None,
            vault_id: None,
            timestamp: Utc::now(),
            reason,
        };

        self.payout_history.push(payout.clone());

        log::error!(
            "Emergency recapitalization executed: {} BTC (remaining balance: {} BTC)",
            deficit_amount.to_btc(),
            self.balance_btc.to_btc()
        );

        Ok(payout)
    }

    /// Mint governance tokens as last resort recapitalization
    pub fn emergency_governance_token_minting(
        &mut self,
        deficit_amount: Amount,
        tokens_to_mint: u64,
    ) -> Result<()> {
        // This would integrate with a governance token system
        self.governance_token_minted += tokens_to_mint;

        let payout = InsurancePayout {
            amount: deficit_amount,
            payout_type: PayoutType::SystemDeficit,
            recipient: None,
            vault_id: None,
            timestamp: Utc::now(),
            reason: format!("Emergency governance token minting: {} tokens", tokens_to_mint),
        };

        self.payout_history.push(payout);

        log::error!(
            "Emergency governance token minting: {} tokens for {} BTC deficit",
            tokens_to_mint,
            deficit_amount.to_btc()
        );

        Ok(())
    }

    /// Cover bad debt from failed liquidations
    pub fn cover_bad_debt(
        &mut self,
        vault_id: Txid,
        bad_debt_amount: Amount,
        vault_owner: PublicKey,
    ) -> Result<InsurancePayout> {
        if bad_debt_amount > self.balance_btc {
            return Err(BitStableError::InsufficientCollateral {
                required: bad_debt_amount.to_btc(),
                provided: self.balance_btc.to_btc(),
            });
        }

        self.balance_btc -= bad_debt_amount;
        self.total_payouts += bad_debt_amount;

        let payout = InsurancePayout {
            amount: bad_debt_amount,
            payout_type: PayoutType::BadDebtCover,
            recipient: Some(vault_owner),
            vault_id: Some(vault_id),
            timestamp: Utc::now(),
            reason: format!("Bad debt coverage for vault {}", vault_id),
        };

        self.payout_history.push(payout.clone());

        log::warn!(
            "Bad debt covered by insurance fund: {} BTC for vault {}",
            bad_debt_amount.to_btc(),
            vault_id
        );

        Ok(payout)
    }

    /// Compensate users during oracle failures
    pub fn compensate_oracle_failure(
        &mut self,
        affected_users: Vec<PublicKey>,
        compensation_per_user: Amount,
    ) -> Result<Vec<InsurancePayout>> {
        let total_compensation = Amount::from_btc(compensation_per_user.to_btc() * affected_users.len() as f64)
            .map_err(|e| BitStableError::InvalidConfig(format!("Invalid compensation amount: {}", e)))?;

        if total_compensation > self.balance_btc {
            return Err(BitStableError::InsufficientCollateral {
                required: total_compensation.to_btc(),
                provided: self.balance_btc.to_btc(),
            });
        }

        let mut payouts = Vec::new();

        for user in affected_users {
            self.balance_btc -= compensation_per_user;
            self.total_payouts += compensation_per_user;

            let payout = InsurancePayout {
                amount: compensation_per_user,
                payout_type: PayoutType::OracleFailureCompensation,
                recipient: Some(user),
                vault_id: None,
                timestamp: Utc::now(),
                reason: "Oracle failure compensation".to_string(),
            };

            self.payout_history.push(payout.clone());
            payouts.push(payout);
        }

        log::warn!(
            "Oracle failure compensation paid: {} BTC to {} users",
            total_compensation.to_btc(),
            payouts.len()
        );

        Ok(payouts)
    }

    /// Calculate governance tokens needed based on deficit
    fn calculate_governance_tokens_needed(&self, deficit_amount: Amount) -> u64 {
        // Simple formula: 1 governance token per 0.01 BTC deficit
        // In practice, this would be more sophisticated
        (deficit_amount.to_btc() / 0.01) as u64
    }

    /// Get current fund health metrics
    pub fn get_fund_health(&self) -> InsuranceFundHealth {
        let contribution_rate = if self.contribution_history.len() > 0 {
            let recent_contributions: f64 = self.contribution_history
                .iter()
                .rev()
                .take(30)  // Last 30 contributions
                .map(|c| c.amount.to_btc())
                .sum();
            recent_contributions / 30.0
        } else {
            0.0
        };

        let payout_rate = if self.payout_history.len() > 0 {
            let recent_payouts: f64 = self.payout_history
                .iter()
                .rev()
                .take(30)  // Last 30 payouts
                .map(|p| p.amount.to_btc())
                .sum();
            recent_payouts / 30.0
        } else {
            0.0
        };

        let coverage_ratio = if self.total_payouts > Amount::ZERO {
            self.balance_btc.to_btc() / self.total_payouts.to_btc()
        } else {
            f64::INFINITY
        };

        InsuranceFundHealth {
            current_balance: self.balance_btc,
            total_contributions: self.total_contributions,
            total_payouts: self.total_payouts,
            net_position: self.total_contributions - self.total_payouts,
            coverage_ratio,
            recent_contribution_rate: contribution_rate,
            recent_payout_rate: payout_rate,
            governance_tokens_minted: self.governance_token_minted,
            last_health_check: self.last_health_check,
        }
    }

    /// Get insurance fund statistics
    pub fn get_statistics(&self) -> InsuranceFundStats {
        let total_contributions_count = self.contribution_history.len();
        let total_payouts_count = self.payout_history.len();

        let avg_contribution = if total_contributions_count > 0 {
            self.total_contributions.to_btc() / total_contributions_count as f64
        } else {
            0.0
        };

        let avg_payout = if total_payouts_count > 0 {
            self.total_payouts.to_btc() / total_payouts_count as f64
        } else {
            0.0
        };

        // Count payouts by type
        let mut payout_breakdown = HashMap::new();
        for payout in &self.payout_history {
            let count = payout_breakdown.entry(payout.payout_type.clone()).or_insert(0);
            *count += 1;
        }

        InsuranceFundStats {
            current_balance: self.balance_btc,
            total_contributions: self.total_contributions,
            total_payouts: self.total_payouts,
            contribution_count: total_contributions_count,
            payout_count: total_payouts_count,
            average_contribution: avg_contribution,
            average_payout: avg_payout,
            payout_breakdown,
            fund_utilization_rate: if self.total_contributions > Amount::ZERO {
                self.total_payouts.to_btc() / self.total_contributions.to_btc()
            } else {
                0.0
            },
        }
    }

    /// Get recent contribution history
    pub fn get_recent_contributions(&self, limit: usize) -> Vec<&InsuranceContribution> {
        self.contribution_history
            .iter()
            .rev()
            .take(limit)
            .collect()
    }

    /// Get recent payout history
    pub fn get_recent_payouts(&self, limit: usize) -> Vec<&InsurancePayout> {
        self.payout_history
            .iter()
            .rev()
            .take(limit)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmergencyAction {
    FundRecapitalization {
        deficit_amount: Amount,
        current_fund_balance: Amount,
    },
    GovernanceTokenMinting {
        deficit_amount: Amount,
        available_fund: Amount,
        tokens_to_mint: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceFundHealth {
    pub current_balance: Amount,
    pub total_contributions: Amount,
    pub total_payouts: Amount,
    pub net_position: Amount,
    pub coverage_ratio: f64,
    pub recent_contribution_rate: f64,
    pub recent_payout_rate: f64,
    pub governance_tokens_minted: u64,
    pub last_health_check: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceFundStats {
    pub current_balance: Amount,
    pub total_contributions: Amount,
    pub total_payouts: Amount,
    pub contribution_count: usize,
    pub payout_count: usize,
    pub average_contribution: f64,
    pub average_payout: f64,
    pub payout_breakdown: HashMap<PayoutType, usize>,
    pub fund_utilization_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;

    #[test]
    fn test_insurance_fund_creation() {
        let config = ProtocolConfig::testnet();
        let fund = InsuranceFund::new(&config);
        
        assert_eq!(fund.balance_btc, Amount::ZERO);
        assert_eq!(fund.fee_percentage, config.insurance_fund_fee_rate);
        assert!(fund.contribution_history.is_empty());
    }

    #[test]
    fn test_fee_contribution() {
        let config = ProtocolConfig::testnet();
        let mut fund = InsuranceFund::new(&config);
        
        let protocol_fees = Amount::from_btc(1.0).unwrap();
        fund.contribute_from_fees(protocol_fees, ContributionSource::ProtocolFees).unwrap();
        
        let expected_contribution = Amount::from_btc(1.0 * config.insurance_fund_fee_rate).unwrap();
        assert_eq!(fund.balance_btc, expected_contribution);
        assert_eq!(fund.contribution_history.len(), 1);
    }

    #[test]
    fn test_emergency_recapitalization() {
        let config = ProtocolConfig::testnet();
        let mut fund = InsuranceFund::new(&config);
        
        // Add some balance first
        fund.balance_btc = Amount::from_btc(10.0).unwrap();
        
        let deficit = Amount::from_btc(5.0).unwrap();
        let payout = fund.execute_emergency_recapitalization(
            deficit, 
            "Test emergency".to_string()
        ).unwrap();
        
        assert_eq!(payout.amount, deficit);
        assert_eq!(fund.balance_btc, Amount::from_btc(5.0).unwrap());
        assert_eq!(fund.payout_history.len(), 1);
    }
}