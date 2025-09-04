# ChargEra Lockup Contract Test Suite Overview

## 1. Introduction

The test suite for the `lockup-contract` is comprehensive, ensuring the correctness and security of all core functionalities. The tests are located in `src/tests/` and leverage the `cw-multi-test` framework, which provides a mock environment for simulating blockchain interactions without needing a live network.

## 2. Test Setup (`src/tests/common.rs`)

A common setup module provides helper functions to streamline test creation.

- **`lockup_contract()`**: Wraps the contract's entry points (`instantiate`, `execute`, `query`) into a `ContractWrapper` that can be used by `cw-multi-test`.
- **`proper_instantiate()`**: A helper function that sets up a standard test environment. It:
  - Creates an `App` instance with predefined admin and user accounts.
  - Initializes user balances.
  - Stores the contract code.
  - Instantiates the contract with a default `TierConfig`.
  - Returns the `App` and the `contract_addr` for use in tests.
- **Helper Functions**: A suite of helpers like `deposit_rewards`, `lock_funds`, `claim_rewards`, `unlock_lockup`, `query_rewards`, `query_lockup`, and `advance_time` simplify test logic by abstracting common contract interactions.

This setup ensures that tests are run in a consistent and realistic environment.

## 3. Unit and Integration Tests

### 3.1. Instantiation Tests (`src/tests/instantiate.rs`)

These tests verify that the contract is initialized correctly.

- **`test_proper_initialization()`**:
  - **Purpose**: To ensure that a standard instantiation with a single default admin works as expected.
  - **Validation**: Asserts that the stored `admins` list and `denom` match the instantiation message.

- **`test_multiple_admins_initialization()`**:
  - **Purpose**: To verify the multi-admin functionality.
  - **Validation**: Instantiates the contract with a list of three admin addresses and asserts that the `admins` list is stored correctly.

### 3.2. Core Functionality Tests (`src/tests/lock_and_claim.rs`)

This is the largest test file, covering the main user flows and edge cases.

- **`test_lock_and_claim_flow()`**:
  - **Purpose**: An end-to-end test of the primary user journey.
  - **Flow**:
    1. Admin deposits rewards.
    2. A user locks a `tier_4` amount.
    3. Verifies the lockup details (APR, amount).
    4. Advances time by half a year and verifies the calculated rewards are correct.
    5. User claims rewards, and the balance change is verified.
    6. Advances time past the unlock date.
    7. User unlocks the principal, and the test asserts that the final rewards and principal are correctly transferred.
    8. Verifies that the lockup has been deleted from state.

- **Deposit and Fund Validation Tests**:
  - **`deposit_below_limit()`**: Asserts that a deposit smaller than `tier_1_min` is rejected with a `DepositBelowMinimum` error.
  - **`deposit_above_limit()`**: Asserts that a deposit larger than `tier_4_limit` is rejected with a `DepositExceedsLimit` error.
  - **`test_lock_insufficient_reward_funds()`**:
    - **Purpose**: To test the critical `check_sufficient_reward_funds` logic.
    - **Flow**: The admin deposits an amount that is insufficient to cover one month of rewards for a new lockup. The test asserts that the `lock` transaction is rejected with an `InsufficientRewardFunds` error.
  - **`test_missing_reward_amount` / `test_missing_reward_amount_2`**:
    - **Purpose**: To test insolvency protection during claims and unlocks.
    - **Flow**: A user locks a large amount, and time is advanced significantly (20 years) to accrue a large amount of rewards that the contract cannot pay. The test asserts that both `ClaimRewards` and `UnlockPrincipal` fail with a `CannotClaimRewards` error, protecting the contract's existing funds.

- **Reward Calculation and Tier Upgrade Tests**:
  - **`test_progressive_apr_10year()`**: Verifies the correctness of the progressive APR calculation over a 10-year period, including the capping mechanism.
  - **`test_rewards_continue_after_lockup_period()`**: Ensures that rewards continue to accrue even after the `unlock_time` has passed.
  - **`test_deposit_with_lockup()`**:
    - **Purpose**: Tests the tier upgrade flow.
    - **Flow**: A user locks an initial amount, claims rewards, then deposits more to upgrade to a higher tier. It verifies that the APR is correctly updated and that subsequent reward calculations use the new APR. It also confirms that the auto-claim mechanism works during the upgrade.
  - **`test_claim_rewards_after_new_deposit()`**: A complex scenario testing multiple tier upgrades within a single year, ensuring that rewards are calculated correctly for each period with its corresponding APR.
  - **`test_start_time_preservation_on_tier_upgrade()`**: Critically verifies that upgrading a tier does not reset the lockup's `start_time`, which is essential for the progressive APR calculation.
  - **`test_lockup_past_period_end()`**: Ensures a user cannot add to their lockup after the `unlock_time` has passed, asserting a `PastLockupPeriod` error.

### 3.3. Query Tests

- **`test_get_all_lockups_query()`**: Tests the `GetAllLockups` query by creating multiple lockups and verifying the response.
- **`test_deposit_rewards_query()`**: Tests the `GetDepositedRewards` query by depositing funds and verifying the returned amount.
- **`test_get_sum_lockups_and_deposits()`**: Verifies the query that returns the total number of lockups and the sum of all principal amounts.
- **`test_coin_availability_insufficient_funds()`**: Tests the `CheckCoinAvailability` query, ensuring it returns `false` when the reward pool is insufficient for the yearly commitment.

## 4. Test Coverage Summary

The test suite provides excellent coverage of the contract's logic:
- **Happy Paths**: Standard user flows like locking, claiming, upgrading, and unlocking are thoroughly tested.
- **Edge Cases**: Zero-reward claims, deposits outside limits, calculations over long periods, and claims against insufficient funds are covered.
- **Security**: Authorization (admin-only functions) and the critical fund sufficiency and insolvency checks are explicitly tested.
- **State Changes**: Tests verify that state is correctly created, updated, and deleted.
- **Queries**: All major query endpoints are validated to ensure they return accurate data.

The use of `cw-multi-test` allows for precise control over block time and balances, making it possible to simulate complex scenarios and validate the time-dependent reward logic with high confidence.
