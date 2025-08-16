use bitcoin::Amount;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc, Duration};
use crate::{Result, ProtocolConfig};
use crate::multi_currency::{Currency, ExchangeRates};
use crate::{Vault, VaultManager, Oracle};

/// Advanced risk metrics and monitoring system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetricsSystem {
    pub risk_config: RiskConfig,
    pub current_metrics: SystemRiskMetrics,
    pub historical_metrics: VecDeque<SystemRiskMetrics>,
    pub risk_alerts: Vec<RiskAlert>,
    pub stress_test_results: Vec<StressTestResult>,
    pub correlation_matrices: HashMap<String, CorrelationMatrix>,
    pub value_at_risk: ValueAtRiskMetrics,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub var_confidence_level: f64,        // 99% VaR
    pub var_time_horizon_days: u64,       // 1-day VaR
    pub correlation_window_days: u64,     // 30-day correlation window
    pub stress_test_scenarios: Vec<StressTestScenario>,
    pub alert_thresholds: AlertThresholds,
    pub monitoring_frequency_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRiskMetrics {
    pub timestamp: DateTime<Utc>,
    pub system_collateral_ratio: f64,
    pub weighted_average_cr: f64,
    pub liquidation_risk_score: f64,
    pub oracle_risk_score: f64,
    pub concentration_risk: ConcentrationRisk,
    pub liquidity_metrics: LiquidityMetrics,
    pub volatility_metrics: VolatilityMetrics,
    pub correlation_risk: f64,
    pub tail_risk_metrics: TailRiskMetrics,
    pub operational_risk_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationRisk {
    pub largest_vault_percentage: f64,
    pub top_10_vaults_percentage: f64,
    pub herfindahl_index: f64,           // Concentration measure
    pub geographic_concentration: HashMap<String, f64>,
    pub currency_concentration: HashMap<Currency, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityMetrics {
    pub bid_ask_spread: f64,
    pub market_depth: f64,
    pub redemption_capacity: f64,
    pub liquidation_capacity: f64,
    pub stability_pool_ratio: f64,
    pub insurance_fund_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityMetrics {
    pub btc_30day_volatility: f64,
    pub btc_90day_volatility: f64,
    pub volatility_regime: VolatilityRegime,
    pub garch_forecast: f64,             // GARCH model volatility forecast
    pub realized_vs_implied: f64,        // Realized vs implied volatility
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolatilityRegime {
    Low,      // < 20% annualized
    Normal,   // 20-60% annualized
    High,     // 60-100% annualized
    Extreme,  // > 100% annualized
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailRiskMetrics {
    pub value_at_risk_1d: f64,          // 1-day 99% VaR
    pub expected_shortfall: f64,         // Expected loss beyond VaR
    pub maximum_drawdown: f64,
    pub tail_dependence: f64,           // Tail dependence with BTC
    pub extreme_value_parameters: ExtremeValueParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtremeValueParams {
    pub shape_parameter: f64,           // Extreme Value Theory shape parameter
    pub scale_parameter: f64,           // Scale parameter
    pub location_parameter: f64,        // Location parameter
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub system_cr_warning: f64,         // 150%
    pub system_cr_critical: f64,        // 130%
    pub liquidation_risk_warning: f64,  // 0.7
    pub liquidation_risk_critical: f64, // 0.8
    pub concentration_warning: f64,      // 0.3 (30% in single vault)
    pub volatility_warning: f64,        // 80% annualized
    pub var_breach_threshold: f64,      // VaR breach significance
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestScenario {
    pub name: String,
    pub description: String,
    pub btc_price_shock: f64,           // % change in BTC price
    pub volatility_shock: f64,          // Increase in volatility
    pub oracle_failure_rate: f64,      // % of oracles failing
    pub liquidation_delay_hours: u64,  // Liquidation processing delay
    pub correlation_increase: f64,     // Increase in correlations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    pub scenario: StressTestScenario,
    pub timestamp: DateTime<Utc>,
    pub system_survival: bool,
    pub final_system_cr: f64,
    pub liquidated_vaults: usize,
    pub total_losses: Amount,
    pub insurance_fund_utilized: f64,
    pub time_to_recovery: Option<Duration>,
    pub critical_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub alert_type: RiskAlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
    pub auto_resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskAlertType {
    SystemCollateralizationRatio,
    LiquidationRisk,
    ConcentrationRisk,
    VolatilitySpike,
    OracleRisk,
    LiquidityDrain,
    VaRBreach,
    TailRisk,
    CorrelationBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub timestamp: DateTime<Utc>,
    pub window_days: u64,
    pub correlations: HashMap<String, HashMap<String, f64>>,
    pub eigenvalues: Vec<f64>,          // For systemic risk analysis
    pub max_eigenvalue: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueAtRiskMetrics {
    pub parametric_var: f64,
    pub historical_var: f64,
    pub monte_carlo_var: f64,
    pub expected_shortfall: f64,
    pub var_backtesting: VaRBacktesting,
    pub confidence_level: f64,
    pub time_horizon_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaRBacktesting {
    pub total_observations: usize,
    pub var_breaches: usize,
    pub breach_rate: f64,
    pub expected_breach_rate: f64,
    pub kupiec_test_pvalue: f64,        // Kupiec test for VaR accuracy
    pub christoffersen_test_pvalue: f64, // Independence test
}

impl RiskMetricsSystem {
    pub fn new(_config: &ProtocolConfig) -> Self {
        let risk_config = RiskConfig {
            var_confidence_level: 0.99,
            var_time_horizon_days: 1,
            correlation_window_days: 30,
            stress_test_scenarios: Self::default_stress_scenarios(),
            alert_thresholds: AlertThresholds {
                system_cr_warning: 1.50,
                system_cr_critical: 1.30,
                liquidation_risk_warning: 0.7,
                liquidation_risk_critical: 0.8,
                concentration_warning: 0.3,
                volatility_warning: 0.8,
                var_breach_threshold: 0.05,
            },
            monitoring_frequency_minutes: 5,
        };

        Self {
            risk_config,
            current_metrics: SystemRiskMetrics::default(),
            historical_metrics: VecDeque::new(),
            risk_alerts: Vec::new(),
            stress_test_results: Vec::new(),
            correlation_matrices: HashMap::new(),
            value_at_risk: ValueAtRiskMetrics::default(),
            last_update: Utc::now(),
        }
    }

    /// Update all risk metrics
    pub fn update_risk_metrics(
        &mut self,
        vault_manager: &VaultManager,
        exchange_rates: &ExchangeRates,
        oracle_network: &[Oracle],
        price_history: &VecDeque<(DateTime<Utc>, f64)>,
    ) -> Result<Vec<RiskAlert>> {
        let mut new_alerts = Vec::new();

        // Calculate current metrics
        self.current_metrics = self.calculate_system_metrics(
            vault_manager,
            exchange_rates,
            oracle_network,
            price_history,
        )?;

        // Store historical data
        self.historical_metrics.push_back(self.current_metrics.clone());
        if self.historical_metrics.len() > 10000 {
            self.historical_metrics.pop_front();
        }

        // Update VaR metrics
        self.update_var_metrics(price_history)?;

        // Update correlation matrices
        self.update_correlation_matrices(price_history)?;

        // Check for alert conditions
        new_alerts.extend(self.check_alert_conditions()?);

        // Update timestamps
        self.last_update = Utc::now();

        Ok(new_alerts)
    }

    /// Calculate comprehensive system risk metrics
    fn calculate_system_metrics(
        &self,
        vault_manager: &VaultManager,
        exchange_rates: &ExchangeRates,
        oracle_network: &[Oracle],
        price_history: &VecDeque<(DateTime<Utc>, f64)>,
    ) -> Result<SystemRiskMetrics> {
        let vaults = vault_manager.get_active_vaults();
        
        // System collateral ratio
        let total_collateral_value = vaults.iter()
            .map(|v| v.collateral_btc.to_btc() * exchange_rates.get_btc_price(&Currency::USD).unwrap_or(0.0))
            .sum::<f64>();
        
        let total_debt_value = vaults.iter()
            .map(|v| v.debts.total_debt_in_usd(exchange_rates))
            .sum::<f64>();
        
        let system_cr = if total_debt_value > 0.0 {
            total_collateral_value / total_debt_value
        } else {
            f64::INFINITY
        };

        // Weighted average CR
        let weighted_cr = if total_debt_value > 0.0 {
            vaults.iter()
                .map(|v| {
                    let vault_cr = v.collateral_ratio(exchange_rates);
                    let vault_debt = v.debts.total_debt_in_usd(exchange_rates);
                    vault_cr * (vault_debt / total_debt_value)
                })
                .sum::<f64>()
        } else {
            0.0
        };

        // Concentration risk
        let concentration_risk = self.calculate_concentration_risk(&vaults, exchange_rates)?;

        // Liquidation risk score
        let liquidation_risk = self.calculate_liquidation_risk_score(&vaults, exchange_rates)?;

        // Oracle risk score  
        let oracle_risk = self.calculate_oracle_risk_score(oracle_network)?;

        // Volatility metrics
        let volatility_metrics = self.calculate_volatility_metrics(price_history)?;

        // Liquidity metrics
        let liquidity_metrics = self.calculate_liquidity_metrics(&vaults, exchange_rates)?;

        // Tail risk metrics
        let tail_risk_metrics = self.calculate_tail_risk_metrics(price_history)?;

        Ok(SystemRiskMetrics {
            timestamp: Utc::now(),
            system_collateral_ratio: system_cr,
            weighted_average_cr: weighted_cr,
            liquidation_risk_score: liquidation_risk,
            oracle_risk_score: oracle_risk,
            concentration_risk,
            liquidity_metrics,
            volatility_metrics,
            correlation_risk: 0.0, // Would be calculated from correlation matrix
            tail_risk_metrics,
            operational_risk_score: 0.1, // Placeholder
        })
    }

    /// Calculate concentration risk metrics
    fn calculate_concentration_risk(
        &self,
        vaults: &[&Vault],
        exchange_rates: &ExchangeRates,
    ) -> Result<ConcentrationRisk> {
        if vaults.is_empty() {
            return Ok(ConcentrationRisk {
                largest_vault_percentage: 0.0,
                top_10_vaults_percentage: 0.0,
                herfindahl_index: 0.0,
                geographic_concentration: HashMap::new(),
                currency_concentration: HashMap::new(),
            });
        }

        let total_debt: f64 = vaults.iter()
            .map(|v| v.debts.total_debt_in_usd(exchange_rates))
            .sum();

        if total_debt == 0.0 {
            return Ok(ConcentrationRisk {
                largest_vault_percentage: 0.0,
                top_10_vaults_percentage: 0.0,
                herfindahl_index: 0.0,
                geographic_concentration: HashMap::new(),
                currency_concentration: HashMap::new(),
            });
        }

        // Sort vaults by debt size
        let mut vault_debts: Vec<f64> = vaults.iter()
            .map(|v| v.debts.total_debt_in_usd(exchange_rates))
            .collect();
        vault_debts.sort_by(|a, b| b.partial_cmp(a).unwrap());

        // Largest vault percentage
        let largest_vault_percentage = vault_debts[0] / total_debt;

        // Top 10 vaults percentage
        let top_10_percentage = vault_debts.iter().take(10).sum::<f64>() / total_debt;

        // Herfindahl-Hirschman Index
        let hhi: f64 = vault_debts.iter()
            .map(|debt| {
                let share = debt / total_debt;
                share * share
            })
            .sum();

        // Currency concentration
        let mut currency_concentration = HashMap::new();
        for vault in vaults {
            for (currency, debt) in &vault.debts.debts {
                let debt_usd = if currency == &Currency::USD {
                    *debt
                } else {
                    debt * exchange_rates.get_rate_to_usd(currency).unwrap_or(1.0)
                };
                *currency_concentration.entry(currency.clone()).or_insert(0.0) += debt_usd / total_debt;
            }
        }

        Ok(ConcentrationRisk {
            largest_vault_percentage,
            top_10_vaults_percentage: top_10_percentage,
            herfindahl_index: hhi,
            geographic_concentration: HashMap::new(), // Would need geographic data
            currency_concentration,
        })
    }

    /// Calculate liquidation risk score (0-1, higher is riskier)
    fn calculate_liquidation_risk_score(
        &self,
        vaults: &[&Vault],
        exchange_rates: &ExchangeRates,
    ) -> Result<f64> {
        if vaults.is_empty() {
            return Ok(0.0);
        }

        let at_risk_vaults = vaults.iter()
            .filter(|v| v.collateral_ratio(exchange_rates) < 1.5)
            .count();

        let total_at_risk_debt: f64 = vaults.iter()
            .filter(|v| v.collateral_ratio(exchange_rates) < 1.5)
            .map(|v| v.debts.total_debt_in_usd(exchange_rates))
            .sum();

        let total_debt: f64 = vaults.iter()
            .map(|v| v.debts.total_debt_in_usd(exchange_rates))
            .sum();

        let vault_ratio = at_risk_vaults as f64 / vaults.len() as f64;
        let debt_ratio = if total_debt > 0.0 { total_at_risk_debt / total_debt } else { 0.0 };

        // Combined score weighted 30% by vault count, 70% by debt
        Ok(vault_ratio * 0.3 + debt_ratio * 0.7)
    }

    /// Calculate oracle risk score
    fn calculate_oracle_risk_score(&self, oracle_network: &[Oracle]) -> Result<f64> {
        if oracle_network.is_empty() {
            return Ok(1.0); // Maximum risk if no oracles
        }

        let total_oracles = oracle_network.len() as f64;
        let functioning_oracles = oracle_network.iter()
            .filter(|o| o.quality_score > 0.5)
            .count() as f64;

        let quality_score: f64 = oracle_network.iter()
            .map(|o| o.quality_score)
            .sum::<f64>() / total_oracles;

        // Risk increases as functioning oracles decrease and quality decreases
        let availability_risk = 1.0 - (functioning_oracles / total_oracles);
        let quality_risk = 1.0 - quality_score;

        Ok((availability_risk + quality_risk) / 2.0)
    }

    /// Calculate volatility metrics
    fn calculate_volatility_metrics(
        &self,
        price_history: &VecDeque<(DateTime<Utc>, f64)>,
    ) -> Result<VolatilityMetrics> {
        if price_history.len() < 30 {
            return Ok(VolatilityMetrics::default());
        }

        // Calculate 30-day volatility
        let prices_30d: Vec<f64> = price_history.iter()
            .rev()
            .take(30)
            .map(|(_, price)| *price)
            .collect();

        let vol_30d = self.calculate_volatility(&prices_30d)?;

        // Calculate 90-day volatility if enough data
        let vol_90d = if price_history.len() >= 90 {
            let prices_90d: Vec<f64> = price_history.iter()
                .rev()
                .take(90)
                .map(|(_, price)| *price)
                .collect();
            self.calculate_volatility(&prices_90d)?
        } else {
            vol_30d
        };

        // Determine volatility regime
        let regime = match vol_30d {
            v if v < 0.20 => VolatilityRegime::Low,
            v if v < 0.60 => VolatilityRegime::Normal,
            v if v < 1.00 => VolatilityRegime::High,
            _ => VolatilityRegime::Extreme,
        };

        Ok(VolatilityMetrics {
            btc_30day_volatility: vol_30d,
            btc_90day_volatility: vol_90d,
            volatility_regime: regime,
            garch_forecast: vol_30d * 1.1, // Simplified GARCH forecast
            realized_vs_implied: 1.0, // Would need implied volatility data
        })
    }

    /// Calculate annualized volatility from price series
    fn calculate_volatility(&self, prices: &[f64]) -> Result<f64> {
        if prices.len() < 2 {
            return Ok(0.0);
        }

        // Calculate log returns
        let returns: Vec<f64> = prices.windows(2)
            .map(|window| (window[1] / window[0]).ln())
            .collect();

        // Calculate mean return
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;

        // Calculate variance
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;

        // Annualize (assuming daily data)
        let annualized_vol = variance.sqrt() * (365.0_f64).sqrt();

        Ok(annualized_vol)
    }

    /// Calculate liquidity metrics
    fn calculate_liquidity_metrics(
        &self,
        _vaults: &[&Vault],
        _exchange_rates: &ExchangeRates,
    ) -> Result<LiquidityMetrics> {
        // This would integrate with actual market data and pool information
        Ok(LiquidityMetrics {
            bid_ask_spread: 0.001,        // 0.1% spread
            market_depth: 1_000_000.0,    // $1M market depth
            redemption_capacity: 0.8,     // 80% of debt can be redeemed
            liquidation_capacity: 0.9,    // 90% liquidation capacity
            stability_pool_ratio: 0.3,    // 30% of debt in stability pool
            insurance_fund_ratio: 0.05,   // 5% insurance fund ratio
        })
    }

    /// Calculate tail risk metrics
    fn calculate_tail_risk_metrics(
        &self,
        price_history: &VecDeque<(DateTime<Utc>, f64)>,
    ) -> Result<TailRiskMetrics> {
        if price_history.len() < 100 {
            return Ok(TailRiskMetrics::default());
        }

        // Calculate returns by converting VecDeque to Vec for windowing
        let price_vec: Vec<_> = price_history.iter().collect();
        let returns: Vec<f64> = price_vec.windows(2)
            .map(|window| (window[1].1 / window[0].1 - 1.0))
            .collect();

        // Sort returns for quantile calculations
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // 1% VaR (99th percentile of losses)
        let var_index = (sorted_returns.len() as f64 * 0.01) as usize;
        let var_1d = -sorted_returns[var_index]; // Negative for loss

        // Expected Shortfall (average of losses beyond VaR)
        let es = -sorted_returns[..var_index].iter().sum::<f64>() / var_index as f64;

        // Maximum drawdown calculation
        let mut peak = price_history[0].1;
        let mut max_dd = 0.0;
        
        for (_, price) in price_history {
            if *price > peak {
                peak = *price;
            }
            let drawdown = (peak - price) / peak;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        Ok(TailRiskMetrics {
            value_at_risk_1d: var_1d,
            expected_shortfall: es,
            maximum_drawdown: max_dd,
            tail_dependence: 0.5, // Would need correlation analysis
            extreme_value_parameters: ExtremeValueParams {
                shape_parameter: 0.1,
                scale_parameter: 0.02,
                location_parameter: 0.0,
            },
        })
    }

    /// Update Value at Risk metrics
    fn update_var_metrics(&mut self, price_history: &VecDeque<(DateTime<Utc>, f64)>) -> Result<()> {
        if price_history.len() < 100 {
            return Ok(());
        }

        // Calculate returns by converting VecDeque to Vec for windowing
        let price_vec: Vec<_> = price_history.iter().collect();
        let returns: Vec<f64> = price_vec.windows(2)
            .map(|window| (window[1].1 / window[0].1 - 1.0))
            .collect();

        // Parametric VaR (normal distribution assumption)
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let std_dev = {
            let variance = returns.iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>() / (returns.len() - 1) as f64;
            variance.sqrt()
        };
        let z_score = 2.33; // 99% confidence level
        let parametric_var = -(mean_return - z_score * std_dev);

        // Historical VaR
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let var_index = (sorted_returns.len() as f64 * 0.01) as usize;
        let historical_var = -sorted_returns[var_index];

        // Expected Shortfall
        let expected_shortfall = -sorted_returns[..var_index].iter().sum::<f64>() / var_index as f64;

        self.value_at_risk = ValueAtRiskMetrics {
            parametric_var,
            historical_var,
            monte_carlo_var: parametric_var * 1.1, // Simplified Monte Carlo
            expected_shortfall,
            var_backtesting: VaRBacktesting::default(),
            confidence_level: self.risk_config.var_confidence_level,
            time_horizon_days: self.risk_config.var_time_horizon_days,
        };

        Ok(())
    }

    /// Update correlation matrices
    fn update_correlation_matrices(&mut self, _price_history: &VecDeque<(DateTime<Utc>, f64)>) -> Result<()> {
        // This would calculate correlations between different assets/factors
        // Simplified implementation for BTC auto-correlation
        let correlation_matrix = CorrelationMatrix {
            timestamp: Utc::now(),
            window_days: self.risk_config.correlation_window_days,
            correlations: HashMap::new(),
            eigenvalues: vec![1.0],
            max_eigenvalue: 1.0,
        };

        self.correlation_matrices.insert("btc_factors".to_string(), correlation_matrix);
        Ok(())
    }

    /// Check alert conditions
    fn check_alert_conditions(&mut self) -> Result<Vec<RiskAlert>> {
        let mut alerts = Vec::new();

        // System CR alerts
        if self.current_metrics.system_collateral_ratio < self.risk_config.alert_thresholds.system_cr_critical {
            alerts.push(RiskAlert {
                alert_type: RiskAlertType::SystemCollateralizationRatio,
                severity: AlertSeverity::Critical,
                message: format!("System CR below critical threshold: {:.2}%", 
                               self.current_metrics.system_collateral_ratio * 100.0),
                metric_value: self.current_metrics.system_collateral_ratio,
                threshold: self.risk_config.alert_thresholds.system_cr_critical,
                timestamp: Utc::now(),
                acknowledged: false,
                auto_resolved: false,
            });
        }

        // Liquidation risk alerts
        if self.current_metrics.liquidation_risk_score > self.risk_config.alert_thresholds.liquidation_risk_critical {
            alerts.push(RiskAlert {
                alert_type: RiskAlertType::LiquidationRisk,
                severity: AlertSeverity::Critical,
                message: format!("High liquidation risk: {:.1}%", 
                               self.current_metrics.liquidation_risk_score * 100.0),
                metric_value: self.current_metrics.liquidation_risk_score,
                threshold: self.risk_config.alert_thresholds.liquidation_risk_critical,
                timestamp: Utc::now(),
                acknowledged: false,
                auto_resolved: false,
            });
        }

        // Concentration risk alerts
        if self.current_metrics.concentration_risk.largest_vault_percentage > self.risk_config.alert_thresholds.concentration_warning {
            alerts.push(RiskAlert {
                alert_type: RiskAlertType::ConcentrationRisk,
                severity: AlertSeverity::Warning,
                message: format!("High vault concentration: {:.1}%", 
                               self.current_metrics.concentration_risk.largest_vault_percentage * 100.0),
                metric_value: self.current_metrics.concentration_risk.largest_vault_percentage,
                threshold: self.risk_config.alert_thresholds.concentration_warning,
                timestamp: Utc::now(),
                acknowledged: false,
                auto_resolved: false,
            });
        }

        // Volatility alerts
        if self.current_metrics.volatility_metrics.btc_30day_volatility > self.risk_config.alert_thresholds.volatility_warning {
            alerts.push(RiskAlert {
                alert_type: RiskAlertType::VolatilitySpike,
                severity: AlertSeverity::Warning,
                message: format!("High volatility: {:.1}%", 
                               self.current_metrics.volatility_metrics.btc_30day_volatility * 100.0),
                metric_value: self.current_metrics.volatility_metrics.btc_30day_volatility,
                threshold: self.risk_config.alert_thresholds.volatility_warning,
                timestamp: Utc::now(),
                acknowledged: false,
                auto_resolved: false,
            });
        }

        // Store alerts
        for alert in &alerts {
            self.risk_alerts.push(alert.clone());
        }

        // Keep only recent alerts (last 1000)
        if self.risk_alerts.len() > 1000 {
            self.risk_alerts.truncate(1000);
        }

        Ok(alerts)
    }

    /// Run stress tests
    pub fn run_stress_tests(&mut self, system_state: &SystemRiskMetrics) -> Result<Vec<StressTestResult>> {
        let mut results = Vec::new();

        for scenario in &self.risk_config.stress_test_scenarios.clone() {
            let result = self.run_single_stress_test(scenario, system_state)?;
            results.push(result);
        }

        self.stress_test_results.extend(results.clone());

        // Keep only recent stress test results
        if self.stress_test_results.len() > 100 {
            self.stress_test_results.truncate(100);
        }

        Ok(results)
    }

    /// Run a single stress test scenario
    fn run_single_stress_test(
        &self,
        scenario: &StressTestScenario,
        system_state: &SystemRiskMetrics,
    ) -> Result<StressTestResult> {
        // Simulate the stress scenario impact
        let post_shock_cr = system_state.system_collateral_ratio * (1.0 + scenario.btc_price_shock);
        let system_survival = post_shock_cr > 1.0;

        // Estimate liquidated vaults (simplified)
        let liquidated_vaults = if post_shock_cr < 1.3 {
            (100.0 * (1.3 - post_shock_cr) / 0.3) as usize
        } else {
            0
        };

        // Estimate losses (simplified)
        let total_losses = if liquidated_vaults > 0 {
            Amount::from_btc(liquidated_vaults as f64 * 0.1).unwrap_or(Amount::ZERO)
        } else {
            Amount::ZERO
        };

        Ok(StressTestResult {
            scenario: scenario.clone(),
            timestamp: Utc::now(),
            system_survival,
            final_system_cr: post_shock_cr,
            liquidated_vaults,
            total_losses,
            insurance_fund_utilized: if total_losses > Amount::ZERO { 0.8 } else { 0.0 },
            time_to_recovery: if system_survival { Some(Duration::days(7)) } else { None },
            critical_failures: if !system_survival { 
                vec!["System undercollateralized".to_string()] 
            } else { 
                Vec::new() 
            },
        })
    }

    /// Get default stress test scenarios
    fn default_stress_scenarios() -> Vec<StressTestScenario> {
        vec![
            StressTestScenario {
                name: "March 2020 Crash".to_string(),
                description: "50% BTC price drop in 24 hours".to_string(),
                btc_price_shock: -0.5,
                volatility_shock: 2.0,
                oracle_failure_rate: 0.2,
                liquidation_delay_hours: 6,
                correlation_increase: 0.3,
            },
            StressTestScenario {
                name: "Black Swan Event".to_string(),
                description: "75% BTC price drop with system failures".to_string(),
                btc_price_shock: -0.75,
                volatility_shock: 5.0,
                oracle_failure_rate: 0.5,
                liquidation_delay_hours: 24,
                correlation_increase: 0.8,
            },
            StressTestScenario {
                name: "Oracle Failure".to_string(),
                description: "80% oracle failure during moderate price decline".to_string(),
                btc_price_shock: -0.25,
                volatility_shock: 1.5,
                oracle_failure_rate: 0.8,
                liquidation_delay_hours: 12,
                correlation_increase: 0.2,
            },
        ]
    }

    /// Get risk dashboard summary
    pub fn get_risk_dashboard(&self) -> RiskDashboard {
        let recent_alerts = self.risk_alerts.iter()
            .rev()
            .take(10)
            .cloned()
            .collect();

        let stress_test_summary = if let Some(latest_test) = self.stress_test_results.last() {
            Some(StressTestSummary {
                timestamp: latest_test.timestamp,
                scenarios_passed: self.stress_test_results.iter()
                    .filter(|r| r.system_survival)
                    .count(),
                total_scenarios: self.stress_test_results.len(),
                worst_case_cr: self.stress_test_results.iter()
                    .map(|r| r.final_system_cr)
                    .fold(f64::INFINITY, f64::min),
            })
        } else {
            None
        };

        RiskDashboard {
            current_metrics: self.current_metrics.clone(),
            risk_score: self.calculate_overall_risk_score(),
            recent_alerts,
            value_at_risk: self.value_at_risk.clone(),
            stress_test_summary,
            system_health: self.assess_system_health(),
            last_update: self.last_update,
        }
    }

    /// Calculate overall risk score (0-1, higher is riskier)
    fn calculate_overall_risk_score(&self) -> f64 {
        let weights = [0.3, 0.2, 0.2, 0.15, 0.15]; // Weights for different risk factors
        let scores = [
            1.0 - (self.current_metrics.system_collateral_ratio - 1.0).clamp(0.0, 1.0),
            self.current_metrics.liquidation_risk_score,
            self.current_metrics.oracle_risk_score,
            self.current_metrics.concentration_risk.herfindahl_index,
            (self.current_metrics.volatility_metrics.btc_30day_volatility / 2.0).min(1.0),
        ];

        weights.iter()
            .zip(scores.iter())
            .map(|(w, s)| w * s)
            .sum()
    }

    /// Assess overall system health
    fn assess_system_health(&self) -> SystemHealth {
        let risk_score = self.calculate_overall_risk_score();
        
        match risk_score {
            r if r < 0.2 => SystemHealth::Excellent,
            r if r < 0.4 => SystemHealth::Good,
            r if r < 0.6 => SystemHealth::Fair,
            r if r < 0.8 => SystemHealth::Poor,
            _ => SystemHealth::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskDashboard {
    pub current_metrics: SystemRiskMetrics,
    pub risk_score: f64,
    pub recent_alerts: Vec<RiskAlert>,
    pub value_at_risk: ValueAtRiskMetrics,
    pub stress_test_summary: Option<StressTestSummary>,
    pub system_health: SystemHealth,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestSummary {
    pub timestamp: DateTime<Utc>,
    pub scenarios_passed: usize,
    pub total_scenarios: usize,
    pub worst_case_cr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemHealth {
    Excellent,  // Risk score < 0.2
    Good,       // Risk score 0.2-0.4
    Fair,       // Risk score 0.4-0.6
    Poor,       // Risk score 0.6-0.8
    Critical,   // Risk score > 0.8
}

// Default implementations
impl Default for SystemRiskMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            system_collateral_ratio: 2.0,
            weighted_average_cr: 2.0,
            liquidation_risk_score: 0.0,
            oracle_risk_score: 0.0,
            concentration_risk: ConcentrationRisk {
                largest_vault_percentage: 0.0,
                top_10_vaults_percentage: 0.0,
                herfindahl_index: 0.0,
                geographic_concentration: HashMap::new(),
                currency_concentration: HashMap::new(),
            },
            liquidity_metrics: LiquidityMetrics {
                bid_ask_spread: 0.001,
                market_depth: 1_000_000.0,
                redemption_capacity: 0.8,
                liquidation_capacity: 0.9,
                stability_pool_ratio: 0.3,
                insurance_fund_ratio: 0.05,
            },
            volatility_metrics: VolatilityMetrics::default(),
            correlation_risk: 0.0,
            tail_risk_metrics: TailRiskMetrics::default(),
            operational_risk_score: 0.1,
        }
    }
}

impl Default for VolatilityMetrics {
    fn default() -> Self {
        Self {
            btc_30day_volatility: 0.4,
            btc_90day_volatility: 0.4,
            volatility_regime: VolatilityRegime::Normal,
            garch_forecast: 0.4,
            realized_vs_implied: 1.0,
        }
    }
}

impl Default for TailRiskMetrics {
    fn default() -> Self {
        Self {
            value_at_risk_1d: 0.05,
            expected_shortfall: 0.08,
            maximum_drawdown: 0.2,
            tail_dependence: 0.5,
            extreme_value_parameters: ExtremeValueParams {
                shape_parameter: 0.1,
                scale_parameter: 0.02,
                location_parameter: 0.0,
            },
        }
    }
}

impl Default for ValueAtRiskMetrics {
    fn default() -> Self {
        Self {
            parametric_var: 0.05,
            historical_var: 0.05,
            monte_carlo_var: 0.05,
            expected_shortfall: 0.08,
            var_backtesting: VaRBacktesting::default(),
            confidence_level: 0.99,
            time_horizon_days: 1,
        }
    }
}

impl Default for VaRBacktesting {
    fn default() -> Self {
        Self {
            total_observations: 0,
            var_breaches: 0,
            breach_rate: 0.0,
            expected_breach_rate: 0.01,
            kupiec_test_pvalue: 1.0,
            christoffersen_test_pvalue: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolConfig;

    #[test]
    fn test_risk_metrics_creation() {
        let config = ProtocolConfig::testnet();
        let risk_system = RiskMetricsSystem::new(&config);
        
        assert_eq!(risk_system.risk_config.var_confidence_level, 0.99);
        assert!(!risk_system.risk_config.stress_test_scenarios.is_empty());
    }

    #[test]
    fn test_volatility_calculation() {
        let config = ProtocolConfig::testnet();
        let risk_system = RiskMetricsSystem::new(&config);
        
        let prices = vec![100.0, 105.0, 102.0, 110.0, 108.0, 115.0];
        let volatility = risk_system.calculate_volatility(&prices).unwrap();
        
        assert!(volatility > 0.0);
        assert!(volatility < 10.0); // Reasonable range
    }

    #[test]
    fn test_overall_risk_score() {
        let config = ProtocolConfig::testnet();
        let risk_system = RiskMetricsSystem::new(&config);
        
        let risk_score = risk_system.calculate_overall_risk_score();
        assert!(risk_score >= 0.0);
        assert!(risk_score <= 1.0);
    }
}