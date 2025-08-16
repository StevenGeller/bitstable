use bitcoin::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use crate::{BitStableError, Result};

/// Governance system for protocol parameter updates and emergency responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceSystem {
    pub proposals: HashMap<u64, Proposal>,
    pub votes: HashMap<u64, ProposalVotes>,
    pub keyholders: Vec<Keyholder>,
    pub voting_config: VotingConfig,
    pub next_proposal_id: u64,
    pub emergency_keyholders: Vec<PublicKey>,
    pub key_rotation_schedule: KeyRotationSchedule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub proposer: PublicKey,
    pub proposal_type: ProposalType,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub voting_deadline: DateTime<Utc>,
    pub execution_deadline: DateTime<Utc>,
    pub status: ProposalStatus,
    pub required_votes: usize,
    pub emergency: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    ParameterChange {
        parameter: String,
        current_value: String,
        new_value: String,
    },
    KeyRotation {
        keys_to_remove: Vec<PublicKey>,
        keys_to_add: Vec<PublicKey>,
    },
    EmergencyShutdown {
        reason: String,
    },
    CircuitBreakerOverride {
        duration_hours: u64,
    },
    OracleAddition {
        oracle_pubkey: PublicKey,
        oracle_endpoint: String,
    },
    OracleRemoval {
        oracle_pubkey: PublicKey,
    },
    InsuranceFundAllocation {
        amount_btc: f64,
        purpose: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Failed,
    Executed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalVotes {
    pub proposal_id: u64,
    pub votes: HashMap<PublicKey, Vote>,
    pub total_weight: f64,
    pub approval_weight: f64,
    pub rejection_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter: PublicKey,
    pub decision: VoteDecision,
    pub weight: f64,
    pub timestamp: DateTime<Utc>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteDecision {
    Approve,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyholder {
    pub pubkey: PublicKey,
    pub weight: f64,                    // Voting weight
    pub role: KeyholderRole,
    pub geographic_region: String,      // For geographic distribution
    pub institution: Option<String>,    // Optional institutional affiliation
    pub added_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub emergency_powers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyholderRole {
    Core,           // Core protocol development team
    Community,      // Community representatives
    Institution,    // Institutional participants
    Technical,      // Technical experts
    Emergency,      // Emergency response only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingConfig {
    pub quorum_threshold: f64,          // Minimum participation for valid vote
    pub approval_threshold: f64,        // Approval threshold for normal proposals
    pub emergency_approval_threshold: f64, // Higher threshold for emergency actions
    pub voting_period_hours: u64,       // Standard voting period
    pub emergency_voting_period_hours: u64, // Expedited voting for emergencies
    pub execution_delay_hours: u64,     // Time-lock before execution
    pub emergency_execution_delay_hours: u64, // Shorter delay for emergencies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationSchedule {
    pub rotation_interval_months: u64,
    pub last_rotation: DateTime<Utc>,
    pub next_rotation: DateTime<Utc>,
    pub rotation_percentage: f64,       // Percentage of keys to rotate
}

impl GovernanceSystem {
    pub fn new() -> Self {
        let voting_config = VotingConfig {
            quorum_threshold: 0.6,          // 60% participation
            approval_threshold: 0.67,       // 67% approval
            emergency_approval_threshold: 0.75, // 75% for emergencies
            voting_period_hours: 168,       // 7 days
            emergency_voting_period_hours: 24, // 24 hours
            execution_delay_hours: 48,      // 48 hour timelock
            emergency_execution_delay_hours: 6, // 6 hour emergency timelock
        };

        let key_rotation_schedule = KeyRotationSchedule {
            rotation_interval_months: 12,   // Annual rotation
            last_rotation: Utc::now(),
            next_rotation: Utc::now() + Duration::days(365),
            rotation_percentage: 0.2,       // Rotate 20% of keys
        };

        Self {
            proposals: HashMap::new(),
            votes: HashMap::new(),
            keyholders: Vec::new(),
            voting_config,
            next_proposal_id: 1,
            emergency_keyholders: Vec::new(),
            key_rotation_schedule,
        }
    }

    /// Add a new keyholder with geographic and role distribution
    pub fn add_keyholder(&mut self, keyholder: Keyholder) -> Result<()> {
        // Check for duplicate
        if self.keyholders.iter().any(|k| k.pubkey == keyholder.pubkey) {
            return Err(BitStableError::InvalidConfig("Keyholder already exists".to_string()));
        }

        // Check geographic distribution
        let region_count = self.keyholders
            .iter()
            .filter(|k| k.geographic_region == keyholder.geographic_region)
            .count();

        if region_count >= 3 {  // Max 3 keyholders per region
            log::warn!("Adding keyholder would exceed regional limit for {}", keyholder.geographic_region);
        }

        if keyholder.emergency_powers {
            self.emergency_keyholders.push(keyholder.pubkey);
        }

        self.keyholders.push(keyholder);
        Ok(())
    }

    /// Create a new governance proposal
    pub fn create_proposal(
        &mut self,
        proposer: PublicKey,
        proposal_type: ProposalType,
        title: String,
        description: String,
        emergency: bool,
    ) -> Result<u64> {
        // Verify proposer is a keyholder
        if !self.keyholders.iter().any(|k| k.pubkey == proposer) {
            return Err(BitStableError::InvalidConfig("Proposer is not a keyholder".to_string()));
        }

        let now = Utc::now();
        let voting_period = if emergency {
            Duration::hours(self.voting_config.emergency_voting_period_hours as i64)
        } else {
            Duration::hours(self.voting_config.voting_period_hours as i64)
        };

        let execution_delay = if emergency {
            Duration::hours(self.voting_config.emergency_execution_delay_hours as i64)
        } else {
            Duration::hours(self.voting_config.execution_delay_hours as i64)
        };

        let required_votes = if emergency {
            (self.keyholders.len() as f64 * self.voting_config.emergency_approval_threshold).ceil() as usize
        } else {
            (self.keyholders.len() as f64 * self.voting_config.approval_threshold).ceil() as usize
        };

        let proposal = Proposal {
            id: self.next_proposal_id,
            proposer,
            proposal_type,
            title,
            description,
            created_at: now,
            voting_deadline: now + voting_period,
            execution_deadline: now + voting_period + execution_delay,
            status: ProposalStatus::Active,
            required_votes,
            emergency,
        };

        self.proposals.insert(self.next_proposal_id, proposal.clone());
        self.votes.insert(self.next_proposal_id, ProposalVotes {
            proposal_id: self.next_proposal_id,
            votes: HashMap::new(),
            total_weight: 0.0,
            approval_weight: 0.0,
            rejection_weight: 0.0,
        });

        let proposal_id = self.next_proposal_id;
        self.next_proposal_id += 1;

        log::info!(
            "Created {} proposal {}: {}",
            if emergency { "emergency" } else { "standard" },
            proposal_id,
            proposal.title
        );

        Ok(proposal_id)
    }

    /// Cast a vote on a proposal
    pub fn cast_vote(
        &mut self,
        proposal_id: u64,
        voter: PublicKey,
        decision: VoteDecision,
        signature: Option<String>,
    ) -> Result<()> {
        // Find voter keyholder info
        let keyholder = self.keyholders.iter_mut()
            .find(|k| k.pubkey == voter)
            .ok_or_else(|| BitStableError::InvalidConfig("Voter is not a keyholder".to_string()))?;

        // Check if proposal exists and is active
        let proposal = self.proposals.get(&proposal_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Proposal not found".to_string()))?;

        if proposal.status != ProposalStatus::Active {
            return Err(BitStableError::InvalidConfig("Proposal is not active".to_string()));
        }

        if Utc::now() > proposal.voting_deadline {
            return Err(BitStableError::InvalidConfig("Voting period has ended".to_string()));
        }

        // Update keyholder activity
        keyholder.last_activity = Utc::now();

        // Record vote
        let vote = Vote {
            voter,
            decision: decision.clone(),
            weight: keyholder.weight,
            timestamp: Utc::now(),
            signature,
        };

        let proposal_votes = self.votes.get_mut(&proposal_id).unwrap();
        
        // Remove previous vote if exists
        if let Some(old_vote) = proposal_votes.votes.insert(voter, vote.clone()) {
            proposal_votes.total_weight -= old_vote.weight;
            match old_vote.decision {
                VoteDecision::Approve => proposal_votes.approval_weight -= old_vote.weight,
                VoteDecision::Reject => proposal_votes.rejection_weight -= old_vote.weight,
                VoteDecision::Abstain => {},
            }
        }

        // Add new vote
        proposal_votes.total_weight += vote.weight;
        match decision {
            VoteDecision::Approve => proposal_votes.approval_weight += vote.weight,
            VoteDecision::Reject => proposal_votes.rejection_weight += vote.weight,
            VoteDecision::Abstain => {},
        }

        log::info!(
            "Vote cast on proposal {}: {:?} with weight {}",
            proposal_id,
            decision,
            vote.weight
        );

        Ok(())
    }

    /// Check and update proposal status based on votes
    pub fn update_proposal_status(&mut self, proposal_id: u64) -> Result<ProposalStatus> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or_else(|| BitStableError::InvalidConfig("Proposal not found".to_string()))?;

        if proposal.status != ProposalStatus::Active {
            return Ok(proposal.status.clone());
        }

        let votes = self.votes.get(&proposal_id).unwrap();
        let total_weight: f64 = self.keyholders.iter().map(|k| k.weight).sum();

        // Check if voting deadline passed
        if Utc::now() > proposal.voting_deadline {
            proposal.status = ProposalStatus::Expired;
            return Ok(proposal.status.clone());
        }

        // Check quorum
        let participation = votes.total_weight / total_weight;
        if participation < self.voting_config.quorum_threshold {
            return Ok(ProposalStatus::Active); // Still waiting for quorum
        }

        // Check approval threshold
        let approval_rate = votes.approval_weight / votes.total_weight;
        let required_threshold = if proposal.emergency {
            self.voting_config.emergency_approval_threshold
        } else {
            self.voting_config.approval_threshold
        };

        if approval_rate >= required_threshold {
            proposal.status = ProposalStatus::Passed;
        } else {
            proposal.status = ProposalStatus::Failed;
        }

        log::info!(
            "Proposal {} status updated to {:?} (approval: {:.2}%, participation: {:.2}%)",
            proposal_id,
            proposal.status,
            approval_rate * 100.0,
            participation * 100.0
        );

        Ok(proposal.status.clone())
    }

    /// Execute a passed proposal after timelock
    pub fn execute_proposal(&mut self, proposal_id: u64) -> Result<ExecutionResult> {
        // Extract proposal data first to avoid borrowing conflicts
        let (proposal_type, _execution_deadline) = {
            let proposal = self.proposals.get(&proposal_id)
                .ok_or_else(|| BitStableError::InvalidConfig("Proposal not found".to_string()))?;

            if proposal.status != ProposalStatus::Passed {
                return Err(BitStableError::InvalidConfig("Proposal has not passed".to_string()));
            }

            if Utc::now() < proposal.execution_deadline {
                return Err(BitStableError::InvalidConfig("Execution timelock has not expired".to_string()));
            }

            (proposal.proposal_type.clone(), proposal.execution_deadline)
        };

        // Now we can safely modify the proposal status
        if let Some(proposal) = self.proposals.get_mut(&proposal_id) {
            proposal.status = ProposalStatus::Executed;
        }

        let result = match &proposal_type {
            ProposalType::ParameterChange { parameter, new_value, .. } => {
                ExecutionResult::ParameterChanged {
                    parameter: parameter.clone(),
                    new_value: new_value.clone(),
                }
            },
            ProposalType::KeyRotation { keys_to_remove, keys_to_add } => {
                self.execute_key_rotation(keys_to_remove.clone(), keys_to_add.clone())?
            },
            ProposalType::EmergencyShutdown { reason } => {
                ExecutionResult::EmergencyShutdown {
                    reason: reason.clone(),
                }
            },
            ProposalType::CircuitBreakerOverride { duration_hours } => {
                ExecutionResult::CircuitBreakerOverride {
                    duration: Duration::hours(*duration_hours as i64),
                }
            },
            ProposalType::OracleAddition { oracle_pubkey, oracle_endpoint } => {
                ExecutionResult::OracleAdded {
                    pubkey: *oracle_pubkey,
                    endpoint: oracle_endpoint.clone(),
                }
            },
            ProposalType::OracleRemoval { oracle_pubkey } => {
                ExecutionResult::OracleRemoved {
                    pubkey: *oracle_pubkey,
                }
            },
            ProposalType::InsuranceFundAllocation { amount_btc, purpose } => {
                ExecutionResult::InsuranceFundAllocated {
                    amount: *amount_btc,
                    purpose: purpose.clone(),
                }
            },
        };

        log::info!("Executed proposal {}: {:?}", proposal_id, result);
        Ok(result)
    }

    /// Execute key rotation
    fn execute_key_rotation(
        &mut self,
        keys_to_remove: Vec<PublicKey>,
        keys_to_add: Vec<PublicKey>,
    ) -> Result<ExecutionResult> {
        // Remove keys
        for key in &keys_to_remove {
            self.keyholders.retain(|k| k.pubkey != *key);
            self.emergency_keyholders.retain(|k| k != key);
        }

        // Add new keys would require separate keyholder creation
        // This is a placeholder for the actual key addition logic

        self.key_rotation_schedule.last_rotation = Utc::now();
        self.key_rotation_schedule.next_rotation = Utc::now() + 
            Duration::days(30 * self.key_rotation_schedule.rotation_interval_months as i64);

        Ok(ExecutionResult::KeyRotationCompleted {
            removed_keys: keys_to_remove,
            added_keys: keys_to_add,
        })
    }

    /// Get governance statistics
    pub fn get_governance_stats(&self) -> GovernanceStats {
        let active_proposals = self.proposals.values()
            .filter(|p| p.status == ProposalStatus::Active)
            .count();

        let executed_proposals = self.proposals.values()
            .filter(|p| p.status == ProposalStatus::Executed)
            .count();

        let total_keyholders = self.keyholders.len();
        let emergency_keyholders = self.emergency_keyholders.len();

        // Calculate geographic distribution
        let mut geographic_distribution = HashMap::new();
        for keyholder in &self.keyholders {
            *geographic_distribution.entry(keyholder.geographic_region.clone()).or_insert(0) += 1;
        }

        GovernanceStats {
            total_proposals: self.proposals.len(),
            active_proposals,
            executed_proposals,
            total_keyholders,
            emergency_keyholders,
            geographic_distribution,
            next_key_rotation: self.key_rotation_schedule.next_rotation,
            governance_config: self.voting_config.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    ParameterChanged {
        parameter: String,
        new_value: String,
    },
    KeyRotationCompleted {
        removed_keys: Vec<PublicKey>,
        added_keys: Vec<PublicKey>,
    },
    EmergencyShutdown {
        reason: String,
    },
    CircuitBreakerOverride {
        duration: Duration,
    },
    OracleAdded {
        pubkey: PublicKey,
        endpoint: String,
    },
    OracleRemoved {
        pubkey: PublicKey,
    },
    InsuranceFundAllocated {
        amount: f64,
        purpose: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStats {
    pub total_proposals: usize,
    pub active_proposals: usize,
    pub executed_proposals: usize,
    pub total_keyholders: usize,
    pub emergency_keyholders: usize,
    pub geographic_distribution: HashMap<String, usize>,
    pub next_key_rotation: DateTime<Utc>,
    pub governance_config: VotingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};

    #[test]
    fn test_governance_system_creation() {
        let gov = GovernanceSystem::new();
        assert_eq!(gov.next_proposal_id, 1);
        assert!(gov.keyholders.is_empty());
        assert_eq!(gov.voting_config.quorum_threshold, 0.6);
    }

    #[test]
    fn test_add_keyholder() {
        let mut gov = GovernanceSystem::new();
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let pubkey = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));

        let keyholder = Keyholder {
            pubkey,
            weight: 1.0,
            role: KeyholderRole::Core,
            geographic_region: "North America".to_string(),
            institution: None,
            added_at: Utc::now(),
            last_activity: Utc::now(),
            emergency_powers: false,
        };

        gov.add_keyholder(keyholder).unwrap();
        assert_eq!(gov.keyholders.len(), 1);
    }

    #[test]
    fn test_create_proposal() {
        let mut gov = GovernanceSystem::new();
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let pubkey = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));

        // Add keyholder first
        let keyholder = Keyholder {
            pubkey,
            weight: 1.0,
            role: KeyholderRole::Core,
            geographic_region: "North America".to_string(),
            institution: None,
            added_at: Utc::now(),
            last_activity: Utc::now(),
            emergency_powers: false,
        };
        gov.add_keyholder(keyholder).unwrap();

        let proposal_id = gov.create_proposal(
            pubkey,
            ProposalType::ParameterChange {
                parameter: "liquidation_threshold".to_string(),
                current_value: "1.25".to_string(),
                new_value: "1.30".to_string(),
            },
            "Increase liquidation threshold".to_string(),
            "Increase safety margin".to_string(),
            false,
        ).unwrap();

        assert_eq!(proposal_id, 1);
        assert!(gov.proposals.contains_key(&1));
    }
}