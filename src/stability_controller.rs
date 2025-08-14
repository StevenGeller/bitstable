use bitcoin::PublicKey;
use serde::{Deserialize, Serialize};
use crate::{Result, BitStableError};
use crate::multi_currency::{Currency, ExchangeRates};

/// Stability controller that manages "Keep X stable" autopilot functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityController {
    pub holder: PublicKey,
    pub target_currency: Currency,
    pub target_amount: f64,
    pub target_percentage: Option<f64>,  // Alternative: keep X% stable
    pub rebalance_threshold: f64,        // Only rebalance if deviation > threshold (e.g., 2%)
    pub enabled: bool,
}

impl StabilityController {
    pub fn new(holder: PublicKey, currency: Currency, amount: f64) -> Self {
        Self {
            holder,
            target_currency: currency,
            target_amount: amount,
            target_percentage: None,
            rebalance_threshold: 0.02,  // 2% default
            enabled: true,
        }
    }

    pub fn new_percentage(holder: PublicKey, currency: Currency, percentage: f64) -> Self {
        Self {
            holder,
            target_currency: currency,
            target_amount: 0.0,
            target_percentage: Some(percentage),
            rebalance_threshold: 0.02,
            enabled: true,
        }
    }

    /// Calculate how much to mint or burn to reach target
    pub fn calculate_rebalance(
        &self,
        current_stable_balance: f64,
        btc_balance: f64,
        exchange_rates: &ExchangeRates,
    ) -> RebalanceAction {
        if !self.enabled {
            return RebalanceAction::None;
        }

        let target = if let Some(percentage) = self.target_percentage {
            // Calculate target based on percentage of total portfolio
            let btc_price = exchange_rates.calculate_btc_price(&self.target_currency, 
                exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0));
            let btc_value = btc_balance * btc_price;
            let total_value = btc_value + current_stable_balance;
            total_value * (percentage / 100.0)
        } else {
            self.target_amount
        };

        let deviation = (current_stable_balance - target).abs() / target.max(1.0);
        
        // Only rebalance if deviation exceeds threshold
        if deviation < self.rebalance_threshold {
            return RebalanceAction::None;
        }

        if current_stable_balance < target {
            RebalanceAction::Mint {
                currency: self.target_currency.clone(),
                amount: target - current_stable_balance,
            }
        } else if current_stable_balance > target {
            RebalanceAction::Burn {
                currency: self.target_currency.clone(),
                amount: current_stable_balance - target,
            }
        } else {
            RebalanceAction::None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RebalanceAction {
    None,
    Mint { currency: Currency, amount: f64 },
    Burn { currency: Currency, amount: f64 },
}

/// Portfolio manager that handles multiple stability controllers
pub struct PortfolioManager {
    controllers: Vec<StabilityController>,
}

impl PortfolioManager {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
        }
    }

    pub fn add_controller(&mut self, controller: StabilityController) {
        self.controllers.push(controller);
    }

    pub fn remove_controller(&mut self, holder: PublicKey, currency: &Currency) {
        self.controllers.retain(|c| !(c.holder == holder && c.target_currency == *currency));
    }

    pub fn get_controller(&self, holder: PublicKey, currency: &Currency) -> Option<&StabilityController> {
        self.controllers.iter()
            .find(|c| c.holder == holder && c.target_currency == *currency)
    }

    pub fn get_holder_controllers(&self, holder: PublicKey) -> Vec<&StabilityController> {
        self.controllers.iter()
            .filter(|c| c.holder == holder)
            .collect()
    }

    /// Process all controllers and return required actions
    pub fn process_rebalancing(
        &self,
        balances: &PortfolioBalances,
        exchange_rates: &ExchangeRates,
    ) -> Vec<(PublicKey, RebalanceAction)> {
        let mut actions = Vec::new();

        for controller in &self.controllers {
            if let Some(holder_balance) = balances.get(&controller.holder) {
                let stable_balance = holder_balance.stable_balances
                    .get(&controller.target_currency)
                    .copied()
                    .unwrap_or(0.0);

                let action = controller.calculate_rebalance(
                    stable_balance,
                    holder_balance.btc_balance,
                    exchange_rates,
                );

                if !matches!(action, RebalanceAction::None) {
                    actions.push((controller.holder, action));
                }
            }
        }

        actions
    }
}

/// Portfolio balances for rebalancing calculations
pub type PortfolioBalances = std::collections::HashMap<PublicKey, HolderBalance>;

#[derive(Debug, Clone)]
pub struct HolderBalance {
    pub btc_balance: f64,  // In BTC
    pub stable_balances: std::collections::HashMap<Currency, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::{PrivateKey, Network};

    #[test]
    fn test_stability_controller() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let holder = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        let controller = StabilityController::new(holder, Currency::USD, 1000.0);
        
        let mut exchange_rates = ExchangeRates::new();
        exchange_rates.update_btc_price(Currency::USD, 100000.0);
        
        // Test when current balance is below target
        let action = controller.calculate_rebalance(800.0, 1.0, &exchange_rates);
        match action {
            RebalanceAction::Mint { amount, .. } => assert_eq!(amount, 200.0),
            _ => panic!("Expected Mint action"),
        }
        
        // Test when current balance is above target
        let action = controller.calculate_rebalance(1200.0, 1.0, &exchange_rates);
        match action {
            RebalanceAction::Burn { amount, .. } => assert_eq!(amount, 200.0),
            _ => panic!("Expected Burn action"),
        }
        
        // Test when within threshold
        let action = controller.calculate_rebalance(1010.0, 1.0, &exchange_rates);
        assert!(matches!(action, RebalanceAction::None));
    }

    #[test]
    fn test_percentage_based_controller() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let holder = PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, Network::Testnet));
        
        // Keep 40% of portfolio stable
        let controller = StabilityController::new_percentage(holder, Currency::USD, 40.0);
        
        let mut exchange_rates = ExchangeRates::new();
        exchange_rates.update_btc_price(Currency::USD, 100000.0);
        
        // Portfolio: 1 BTC ($100k) + $50k stable = $150k total
        // Target: 40% of $150k = $60k stable
        let action = controller.calculate_rebalance(50000.0, 1.0, &exchange_rates);
        match action {
            RebalanceAction::Mint { amount, .. } => assert_eq!(amount, 10000.0),
            _ => panic!("Expected Mint action"),
        }
    }
}
