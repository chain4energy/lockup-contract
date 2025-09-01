# ChargEra Lockup Contract Test Suite Overview

## 1. Introduction

The test suite for the `lockup-contract` is comprehensive, ensuring the correctness and security of all core functionalities. The tests are divided into unit tests and integration tests, located in `src/tests/` and `src/integration_tests.rs` respectively. The tests use the `cw-multi-test` framework, which provides a mock environment for simulating blockchain interactions without needing a live network.

## 2. Test Setup (`src/tests/common.rs`)

A common setup module provides helper functions to streamline test creation.

- **`lockup_contract()`**: Wraps the contract's entry points (`instantiate`, `execute`, `query`) into a `ContractWrapper` that can be used by `cw-multi-test`.
- **`proper_instantiate()`**: A helper function that sets up a standard test environment. It:
  - Creates an `App` instance with predefined admin and user accounts.
  - Initializes user balances.
  - Stores the contract code.
  - Instantiates the contract with a default `TierConfig`.
  - Returns the `App` and the `contract_addr` for use in tests.

This setup ensures that tests are run in a consistent and realistic environment.

## 3. Unit and Integration Tests

### 3.1. Instantiation Tests (`src/tests/instantiate.rs`)

These tests verify that the contract is initialized correctly.

- **`test_proper_initialization()`**:
  - **Purpose**: To ensure that a standard instantiation with a single default admin works as expected.
  - **Validation**:
    - It calls `proper_instantiate()`.
    - It queries the contract's configuration using `QueryMsg::GetConfig {}`.
    - It asserts that the stored `admins` list contains only the expected admin address.
    - It asserts that the `denom` is correctly stored.

- **`test_multiple_admins_initialization()`**:
  - **Purpose**: To verify the multi-admin functionality.
  - **Validation**:
    - It instantiates the contract with a list of three admin addresses.
    - It queries the configuration.
    - It asserts that the `admins` list has a length of 3 and contains all three specified admin addresses.

### 3.2. Core Functionality Tests (`src/tests/lock_and_claim.rs`)

This is the largest test file, covering the main user flows and edge cases.

- **`test_lock_and_claim_flow()`**:
  - **Purpose**: An end-to-end test of the primary user journey.
  - **Flow**:
    1. A user locks a `tier_1` amount.
    2. The test verifies the lockup details (APR, unlock time) are correct.
    3. Time is advanced using `app.update_block()`.
    4. The user claims rewards, and the test asserts the claimed amount is correct.
    5. The user upgrades to `tier_2` by depositing more tokens.
    6. The test verifies that the APR is updated but the `start_time` and `unlock_time` are preserved.
    7. Time is advanced past the unlock date.
    8. The user unlocks the principal, and the test asserts that the final rewards and principal are correctly transferred.
    9. It verifies that the lockup has been deleted from state.

- **Deposit and Fund Validation Tests**:
  - **`deposit_below_limit()`**: Asserts that a deposit smaller than `tier_1_min` is rejected with a `DepositBelowMinimum` error.
  - **`deposit_above_limit()`**: Asserts that a deposit larger than `tier_4_limit` is rejected with a `DepositExceedsLimit` error.
  - **`test_lock_insufficient_reward_funds()`**:
    - **Purpose**: To test the critical fund sufficiency check.
    - **Flow**:
      1. The contract is instantiated with an empty reward pool.
      2. A user attempts to lock funds.
      3. The test asserts that the transaction is rejected with a "Insufficient reward funds" error.

- **Reward Calculation Tests**:
  - **`test_progressive_apr_10year()`**:
    - **Purpose**: To verify the correctness of the progressive APR calculation over a long period.
    - **Flow**:
      1. A user locks funds.
      2. The test advances time year by year for 10 years.
      3. In each year, it calculates the expected rewards based on the progressively increasing APR.
      4. It executes a `ClaimRewards` message and asserts that the claimed amount matches the expected calculation for that year.
      5. It also verifies that the APR caps correctly at the `max_percentage_increase`.
  - **`test_rewards_continue_after_lockup_period()`**: Ensures that rewards continue to accrue even after the `unlock_time` has passed, as long as the principal has not been withdrawn.

- **Tier Upgrade Tests**:
  - **`test_start_time_preservation_on_tier_upgrade()`**:
    - **Purpose**: To ensure that upgrading a tier does not reset the lockup's start date, which is critical for the progressive APR calculation.
    - **Flow**:
      1. A user locks funds.
      2. Time is advanced.
      3. The user upgrades their lockup to a higher tier.
      4. The test queries the lockup state and asserts that the `start_time` remains unchanged from the initial deposit.

### 3.3. Query Tests

Many tests implicitly validate queries, but some are specifically designed for them.

- **`test_get_all_lockups_query()`**:
  - **Purpose**: To test the `GetAllLockups` query.
  - **Flow**:
    1. Multiple users lock funds.
    2. The test calls the `GetAllLockups` query.
    3. It asserts that the returned list contains the correct number of lockups and that the data (address, amount, tier) for each is accurate.

- **`test_deposit_rewards_query()`**:
  - **Purpose**: To test the admin-only `DepositRewards` function and the `GetAvailableRewards` query.
  - **Flow**:
    1. The admin deposits funds into the reward pool.
    2. The test queries the available rewards and asserts that the amount matches the deposit.
    3. A user locks funds, which commits some of the rewards.
    4. The test queries the available rewards again and asserts that the amount has decreased accordingly.

## 4. Test Coverage Summary

The test suite provides excellent coverage of the contract's logic:
- **Usual Cases**: Standard user flows like locking, claiming, and unlocking are thoroughly tested.
- **Edge Cases**: Zero-reward claims, deposits outside limits, and calculations over long periods are covered.
- **Security**: Authorization (admin-only functions) and the critical fund sufficiency check are explicitly tested.
- **State Changes**: Tests verify that state is correctly created, updated, and deleted.
- **Queries**: All major query endpoints are validated to ensure they return accurate data.

The use of `cw-multi-test` allows for precise control over block time and balances, making it possible to simulate complex scenarios and validate the time-dependent reward logic with high confidence.
