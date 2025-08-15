# BitStable: A Peer‑to‑Peer Bitcoin‑Collateralized Multi‑Currency Electronic Cash System

**Version 0.9 — August 2025**

---

## Abstract

We propose a system for electronic transactions in which users hold bitcoin but may keep a portion of their balance stable to a currency of their choice (USD, EUR, NGN, …). The mechanism locks BTC as collateral in on‑chain multisig vaults and issues stable balances against that collateral. Peg stability is maintained by price‑indexed collateralization, market‑based liquidations, and a simple fee schedule rather than discretionary redemption. Balances are transferable by public key and auditable; solvency does not depend on bank reserves. The system extends to multiple currencies by quoting BTC across pairs and tracking per‑currency debt buckets. We describe the transactions, the network, the incentive model, and calculations for safe parameterization.

---

## 1. Introduction

Commerce on the Internet benefits from predictable balances. Existing stablecoins largely depend on trusted custodians of fiat reserves. We outline a method that keeps value stable to a selected fiat numeraire while inheriting Bitcoin’s settlement and audit properties. Nodes maintain vaults that hold BTC collateral and track debt; a price oracle network supplies BTC reference prices and FX rates; liquidators restore solvency when needed; users transfer stable balances directly by public key.

The design separates **store of value** (BTC) from **unit of account** (chosen currency). A wallet may show a BTC balance and a stable balance in the local currency. The user selects a target amount to keep stable; an automated controller mints or burns around this target within collateral limits. No central party redeems IOUs for fiat; all issuance is over‑collateralized BTC.

---

## 2. Overview

We define the following roles:

* **Vault owners (minters)** deposit BTC collateral and mint stable value.
* **Holders (users)** keep and transfer stable balances by public key.
* **Liquidators** seize collateral on under‑collateralized vaults and earn a bonus.
* **Oracles** report signed BTC/USD and FX prices under a thresholded median rule.
* **Protocol keys** co‑sign vault escrow spends as policy keys (federated).

State objects:

* **Vault** with collateral `B` (in BTC) and per‑currency debts `D_k` (fiat units).
* **Stable positions** for each public key, composed of slices referencing backing vaults.
* **Price history** of oracle consensus values.

Parameters:

* Minimum collateral ratio `M > 1`, liquidation threshold `L` with `1 < L < M`, liquidation penalty `γ > 0`, stability fee APR `α ≥ 0`.

---

## 3. Transactions

We treat all actions as transactions that update shared state and, when necessary, produce Bitcoin transactions.

### 3.1 Mint

A minter with public key `X` deposits BTC `B` to a vault and mints `d_k` units of stable value in currency `k`. The vault records `B` and `D_k ← D_k + d_k`. The stable manager credits a slice of amount `d_k` to `X`, referencing the vault ID.

### 3.2 Transfer

A holder transfers `x_k` units in currency `k` from `X` to `Y`. The manager moves slices (FIFO) totaling `x_k`, preserving the mapping of each slice to its backing vault. No on‑chain activity is required.

### 3.3 Burn / Redemption

A holder burns `x_k`. The manager reduces slices and returns the list of affected vaults. If policy permits redemption, the custody module co‑signs a vault spend that pays BTC back to the redeemer at the oracle price, less fees.

### 3.4 Liquidation

For a vault with collateral `B` and total USD‑equivalent debt `D = Σ_k D_k · r_k` (where `r_k` converts currency `k` to USD), define collateral ratio at BTC price `P`:

```
CR = (B · P) / D .
```

If `CR < L`, the vault is liquidatable. A liquidator covers `D` by seizing BTC:

```
seized_BTC = (D / P) · (1 + γ) ,
bonus_BTC  = (D / P) · γ .
```

The custody module co‑signs a settlement that pays the liquidator and returns any remainder to the owner.

### 3.5 Close

When `D = 0`, the owner closes the vault and receives all BTC back from escrow.

---

## 4. Price Oracle

Oracles independently fetch prices and sign `(pair, price, timestamp)` messages. Nodes accept a price update when at least `t` oracles report and the value lies within a circuit‑breaker bound relative to the last accepted price. The consensus price is the median of accepted reports. FX pairs extend the mechanism to multiple numeraires. For compact proofs of participation, a digest of individual signatures may be recorded; production systems can use threshold Schnorr.

---

## 5. Vaults and Ratios

Let a vault hold `B` BTC and debt vector `D_k`. With BTC/USD price `P` and FX rates `r_k` to USD, let `D = Σ_k D_k · r_k`. The **minimum collateral ratio** `M` is enforced at mint time. The **liquidation threshold** `L` defines the trigger for liquidation. The **implied liquidation price** of BTC is

```
P_liq = (D · L) / B .
```

Stability fees accrue continuously:

```
D(t + Δ) = D(t) · (1 + α · Δ/365.25) .
```

Fees raise `D` over time; minters must add collateral or burn to maintain health.

---

## 6. Network

Nodes operate as follows:

1. **Price Update:** Collect signed prices from oracles, reject outliers, accept median.
2. **Mint/Burn/Transfer:** Apply user requests to the stable manager, updating positions.
3. **Scan:** For each vault, compute `CR`; if `CR < L`, enqueue a liquidation opportunity.
4. **Liquidate:** When a liquidator commits, create a settlement transaction from escrow.
5. **Broadcast/Confirm:** Broadcast settlement via a Bitcoin node; observe confirmations.
6. **Prune:** Retain recent price history and necessary accounting; archive older records.

The protocol requires only eventual message delivery. On‑chain finality is provided by Bitcoin. Off‑chain state (positions, queues) is reconstructible from persisted logs.

---

## 7. Incentive

* **Minters** gain BTC exposure and liquidity; they pay the stability fee `α` on `D`.
* **Holders** obtain stable purchasing power; optional savings rates can be funded from fees.
* **Liquidators** earn a spread `γ` during stress; timely liquidation restores solvency.
* **Oracles** can be paid per accepted update; misbehavior is filtered by thresholds and bounds.
* **Protocol treasury** may receive a small fee from liquidations to fund operations.

Proper selection of `M`, `L`, `γ`, and `α` aligns incentives: routine operation collects moderate fees; shocks are handled by market‑driven liquidation rather than governance.

---

## 8. Privacy

Public keys identify balances. Transfers move slices without revealing underlying vault owners. On‑chain activity occurs only for escrow funding, liquidation, and closure. As with Bitcoin, reuse of addresses is discouraged; wallets may rotate keys for both BTC and stable positions.

---

## 9. Simplified Solvency Verification

A light client verifies that the system is solvent without operating a full node:

1. Fetch the latest accepted price and FX rates.
2. Request the set of active vault summaries `(id, B, D_k)` and stable supply by currency.
3. Compute overall ratio:

```
R = (Σ_vaults B_v · P) / (Σ_currencies Supply_k · r_k) .
```

If `R ≥ 1`, the system is over‑collateralized. Spot checks may verify escrow UTXOs and settlement transactions via Bitcoin headers (SPV).

---

## 10. Combining and Splitting Value

Stable balances are represented as slices backed by vault IDs. Transfers combine or split slices as necessary. This preserves the provenance of backing and enables deterministic unwind on burns (FIFO).

---

## 11. Storage and Pruning

Nodes keep recent price history and essential accounting. Older records can be archived. Snapshots of vault states and supply can be periodically notarized on‑chain if desired. Pruning reduces disk usage without affecting auditability of current solvency metrics.

---

## 12. Calculations

### 12.1 Safe Mint Capacity

Given collateral `B`, price `P`, and parameters `M` and `L`, the maximum debt `D_max` at mint is

```
D_max = (B · P) / M .
```

The implied liquidation price is `P_liq = (D · L)/B`. The safety margin to current price is

```
σ = 1 − P_liq / P = 1 − (D · L)/(B · P) .
```

### 12.2 Liquidation Bound

Suppose price falls from `P` to `P'` in one step. A vault is liquidatable if

```
B · P' / D < L  ⇔  P' < (D · L)/B .
```

The seized BTC equals `(D/P')·(1+γ)` but is capped by available `B`. For `γ` small and deep books, the liquidator’s expected profit net of fees is approximately `(γ · D)/P'`.

### 12.3 Fee Accrual

Over `T` years, debt grows by `D · α · T`. If prices remain unchanged, the mintable headroom decreases by the same amount; the controller may burn to keep `CR ≥ M`.

---

## 13. Multi‑Currency Extension

For currencies `k ∈ K`, each vault tracks `D_k`. The oracle provides `BTC/USD` and `USD/k`. Define `r_k = USD/k` and compute `D = Σ_k D_k · r_k`. Mint, transfer, burn, liquidation, and close operate per currency code with identical rules. The wallet exposes a control: “keep `T_k` stable in currency `k`,” and a dead‑band prevents churn. The controller adjusts `D_k` by minting/burning subject to `CR ≥ M`.

---

## 14. Security Considerations

* **Bitcoin security:** Settlement inherits Bitcoin’s liveness and finality.
* **Oracle honesty:** A threshold of honest oracles is required; median plus circuit‑breakers reduce manipulation. Threshold Schnorr can harden attestations.
* **Key management:** Escrow uses federated keys. Production deployments distribute protocol keys across independent operators or MPC.
* **Liquidity risk:** Penalty `γ` and thresholds should reflect expected depth; conservative settings favor solvency over capital efficiency.
* **Numeric precision:** Fiat accounting should use fixed‑point types to avoid drift.

---

## 15. Conclusion

We described a peer‑to‑peer method to keep balances stable to arbitrary currencies using BTC collateral and simple rules. The mechanism avoids reliance on bank IOUs, uses market incentives to restore solvency under stress, and retains Bitcoin’s auditability. With modest extensions for FX rates and a portfolio controller, a wallet can hold BTC while spending in the local unit of account.

---

### Appendix A — Suggested Defaults

* `M = 1.50` (150% minimum collateral)
* `L = 1.10` (110% liquidation threshold)
* `γ = 0.05` (5% liquidator bonus)
* `α = 0.02` (2% stability fee APR)
* Oracle threshold `t = 3` of `N` oracles; median with 20% jump limit.

### Appendix B — Pseudocode (Vault Health)

```
function collateral_ratio(B, P, Dk[], rk[]):
    D = 0
    for k in K:
        D += Dk[k] * rk[k]
    if D == 0: return +∞
    return (B * P) / D
```

### Appendix C — Pseudocode (Controller)

```
for each currency k:
    err = target_k - balance_k
    if |err|/target_k > ε and CR >= M:
        if err > 0: mint(k, min(err, headroom))
        else:       burn(k, min(|err|, balance_k))
```

---

**Reference implementation modules that inspired this design**: README (overview), oracle (median + circuit breaker), stable manager (mint/burn/transfer & solvency snapshot), vaults (collateral ratio and fees), liquidation engine (bonus and queue), custody (escrow and settlements).     &#x20;
