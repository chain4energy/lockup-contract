# ChargEra Lockup Contract Code Overview

## 1. Introduction

This document provides a detailed overview of the `lockup-contract` CosmWasm smart contract. The contract is designed to manage time-locked token deposits with a tiered reward system and progressive APR increases over time. It is built with security, clarity, and flexibility in mind, providing a robust solution for incentivizing long-term token holding.

The core logic has been significantly refactored into smaller, more manageable helper functions to improve readability, maintainability, and security.

## 2. Contract Architecture

The contract follows a standard CosmWasm architecture, with clear separation of concerns between state, messages, and core logic.

### 2.1. Core Data Structures

#### `Config` (in `src/state.rs`)
The `Config` struct stores the contract's immutable parameters, set during instantiation.

```rust
#[cw_serde]
pub struct Config {
    pub admins: Vec<Addr>,
    pub denom: String,
    pub lockup_duration_seconds: u64,
    pub tier_config: TierConfig,
}
```
- **`admins`**: A list of addresses with administrative privileges (e.g., for depositing rewards). Supports multiple admins for enhanced security.
- **`denom`**: The native token denomination for all deposits and rewards (e.g., `uc4e`).
- **`lockup_duration_seconds`**: The fixed lockup period for all new deposits.
- **`tier_config`**: A nested struct defining the rules for reward tiers.

#### `TierConfig` (in `src/state.rs`)
This struct defines the parameters for the tiered reward system.

```rust
#[cw_serde]
pub struct TierConfig {
    pub tier_1_min: Uint128,
    pub tier_2_min: Uint128,
    pub tier_3_min: Uint128,
    pub tier_4_min: Uint128,
    pub tier_4_limit: Uint128,
    pub tier_1_apr: Decimal,
    pub tier_2_apr: Decimal,
    pub tier_3_apr: Decimal,
    pub tier_4_apr: Decimal,
    pub percentage_increase_per_year: Decimal,
    pub max_percentage_increase: Decimal,
}
```
- **Tier Minimums**: `tier_1_min` to `tier_4_min` define the minimum deposit amount required to qualify for each tier.
- **Deposit Limit**: `tier_4_limit` sets the maximum total deposit allowed per user.
- **APRs**: `tier_1_apr` to `tier_4_apr` define the base Annual Percentage Rate for each tier.
- **Progressive APR**: `percentage_increase_per_year` and `max_percentage_increase` define how the APR increases for lockups held beyond the initial lockup period.

#### `Lockup` (in `src/state.rs`)
This struct stores the state of an individual user's lockup.

```rust
#[cw_serde]
pub struct Lockup {
    pub owner: Addr,
    pub principal_amount: Coin,
    pub unlock_time: Timestamp,
    pub annual_percentage_rate: Decimal,
    pub last_claim_time: Timestamp,
    pub start_time: Timestamp,
}
```
- **`owner`**: The user's address.
- **`principal_amount`**: The total amount of tokens locked by the user.
- **`unlock_time`**: The timestamp when the principal can be withdrawn.
- **`annual_percentage_rate`**: The base APR for the user's current tier.
- **`last_claim_time`**: The timestamp of the last reward claim, used to calculate pending rewards.
- **`start_time`**: The timestamp of the initial deposit, preserved across tier upgrades.

### 2.2. State Management

- **`CONFIG: Item<Config>`**: A singleton `Item` that stores the contract's configuration.
- **`LOCKUPS: Map<Addr, Lockup>`**: A `Map` that stores all active lockups, keyed by the owner's address. This enforces a one-lockup-per-address rule.

## 3. Core Functions

### 3.1. `instantiate()`
The `instantiate` function initializes the contract with the parameters defined in `InstantiateMsg`.

**Key Logic:**
1. **Admin Configuration**:
   - If `admins` are provided, it validates and stores the list of admin addresses.
   - If `admins` is `None`, it defaults to the message sender (`info.sender`) as the sole admin.
2. **Configuration Storage**: It saves the `Config` object to the `CONFIG` state item.
3. **Event Emission**: Emits an `instantiate` event with the configured admins and token denomination.

### 3.2. `execute()`
The `execute` function routes all state-changing operations. The logic is broken down into several helper functions for clarity and security.

#### `ExecuteMsg::Lock {}`
This function allows users to deposit tokens or upgrade an existing deposit.

**Key Logic:**
1. **Fund Validation**:
   - `validate_lock_funds`: Ensures exactly one type of coin is sent, it matches the configured `denom`, and the amount is not zero.
   - `validate_deposit_amount`: Rejects deposits below the minimum (`tier_1_min`) or if the new total would exceed the maximum (`tier_4_limit`).
2. **Fund Sufficiency Check (`check_sufficient_reward_funds`)**:
   - This is a critical security feature. Before accepting a new deposit, the contract verifies it has enough funds to cover its reward obligations.
   - It calculates the **monthly** reward requirement for the new deposit (using the maximum possible APR) and adds it to the total monthly requirement for existing lockups.
   - If the available reward funds (total balance minus all principals and pending rewards) are insufficient, the transaction is rejected. This prevents the contract from becoming insolvent.
3. **Tier Upgrade Logic (`handle_lockup_upgrade`)**:
   - If the user already has a lockup, the new deposit is added to the principal.
   - **Auto-Claim**: Before the upgrade, any pending rewards are automatically claimed and sent to the user via the `auto_claim_rewards` helper. This simplifies the logic by ensuring a clean state before the principal and APR are modified.
   - The contract recalculates the user's tier. If the new total qualifies for a higher tier, the APR is upgraded via `determine_upgrade_apr`.
   - **Crucially, the original `unlock_time` and `start_time` are preserved**, ensuring that tier upgrades do not reset the lockup clock.
4. **New Lockup Creation (`create_new_lockup_params`)**:
   - For a new user, it creates a `Lockup` object, setting the `unlock_time` to `current_time + lockup_duration_seconds`.
5. **State Update**: Saves the new or updated `Lockup` object to the `LOCKUPS` map.

#### `ExecuteMsg::ClaimRewards {}`
Allows users to claim their accrued rewards without withdrawing their principal.

**Key Logic:**
1. **Reward Calculation**: Calls the `calculate_rewards()` helper function to determine the amount of pending rewards.
2. **Validation**:
   - Rejects the claim if the reward amount is zero.
   - **Checks against available rewards**: Ensures the contract has sufficient liquid funds to pay the claim without touching other users' principal or accrued rewards, using the `query_available_rewards` helper.
3. **State Update**: Updates the `last_claim_time` in the user's `Lockup` object to the current block time.
4. **Fund Transfer**: Sends the calculated reward amount to the user via a `BankMsg`.

#### `ExecuteMsg::UnlockPrincipal {}`
Allows users to withdraw their principal and any final pending rewards after the lockup period has ended.

**Key Logic:**
1. **Time Lock Check**: Ensures `current_time` is greater than or equal to the `unlock_time`.
2. **Final Reward Calculation**: Calculates any remaining rewards accrued since the last claim.
3. **Available Funds Check**: Ensures the contract has enough available rewards to pay out the final pending rewards.
4. **State Deletion**: **Removes the `Lockup` object from the `LOCKUPS` map**. This is an irreversible action.
5. **Fund Transfer**: Sends both the principal amount and the final rewards to the user.

#### `ExecuteMsg::DepositRewards {}`
An admin-only function to fund the contract's reward pool.

**Key Logic:**
1. **Authorization**: Checks if the message sender is in the `config.admins` list.
2. **Fund Validation**: Ensures that funds were actually sent with the message.
3. **No State Change**: The funds are automatically added to the contract's balance by the bank module.

### 3.3. `query()`
The `query` function provides read-only access to the contract's state. Most queries that iterate over all lockups now use the `fold_lockups` helper for efficiency.

- **`GetConfig {}`**: Returns the `Config` object.
- **`GetLockup { address }`**: Returns the `Lockup` details for a specific user.
- **`GetClaimableRewards { address }`**: Returns the currently claimable rewards for a user by calling `calculate_rewards()`.
- **`GetAllLockups {}`**: Paginates through all lockups and returns a list of `LockupInfo` objects, including the calculated tier for each lockup.
- **`GetDepositedRewards {}` / `GetAvailableRewards {}`**: Calculates and returns the amount of funds available for reward payouts. The calculation is: `Total Contract Balance - Total Principal Locked - Total Pending Rewards`.
- **`GetRewardsPerYear {}`**: Calculates the total yearly reward obligation for all active lockups, using `calculate_yearly_rewards_for_lockup` for each.
- **`CheckCoinAvailability {}`**: A boolean query that returns `true` if the available rewards are sufficient to cover the total yearly reward obligations.

## 4. Reward Calculation (`calculate_rewards()`)

This is the most complex part of the contract, refactored for clarity and accuracy, especially for multi-year calculations with progressive APR.

**Key Logic:**
1. **Time Segmentation**:
   - It determines if the calculation period (from `last_claim_time` to `current_time`) falls within a single "lockup year" or spans multiple years by comparing `get_year_index()` for both timestamps.
2. **Simple vs. Progressive Calculation**:
   - **`calculate_simple_rewards`**: If the period is within a single year, a straightforward calculation is used with the lockup's base APR.
   - **`calculate_progressive_rewards`**: If the period spans multiple years, the function iterates through each year segment.
3. **Progressive APR Calculation (`calculate_apr_for_year`)**:
   - For each year segment, it calculates the correct effective APR.
   - For the first year (`year_index == 0`), the base `annual_percentage_rate` is used.
   - For subsequent years, the APR is increased by `percentage_increase_per_year` for each full year that has passed since the `start_time`.
   - The total APR is capped at `base_apr + max_percentage_increase`.
4. **Summation**: It sums the rewards from each segment to get the total.
5. **Precision**: All calculations are performed using `Decimal` types to maintain precision, and the final result is converted to `Uint128`.

## 5. Security Considerations

- **Admin Controls**: All administrative functions are strictly permissioned. The use of a `Vec<Addr>` for admins mitigates the risk of a single point of failure.
- **Fund Safety and Insolvency Prevention**:
    - The `check_sufficient_reward_funds` check during `execute_lock` prevents the contract from accepting new deposits if it cannot guarantee future reward payouts for at least one month.
    - The `available_rewards` check during `claim_rewards` and `unlock_principal` ensures that payouts do not compromise funds committed to other users (principals and accrued rewards).
- **Reentrancy**: Not a concern in the single-threaded CosmWasm environment.
- **Time Lock**: The `unlock_time` is strictly enforced, preventing premature withdrawal of principal.
- **Data Integrity**: The use of `cw_storage_plus` provides a robust and type-safe way to manage state.
- **Error Handling**: The `ensure!` macro is used for concise and clear validation checks, improving code readability.
- **Irreversible Operations**: The `UnlockPrincipal` operation, which deletes the lockup state, is irreversible.
- **No Upgrades**: The contract does not currently have an upgrade mechanism. Any changes would require deploying a new contract and migrating the state.
