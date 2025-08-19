use super::common::*;
use crate::msg::{ExecuteMsg, QueryMsg};
use crate::state::Lockup;
use cosmwasm_std::{coin, Coin, Decimal, Uint128, BlockInfo, StdResult};
use cw_multi_test::Executor;

const C4E_1: u128 = 1_000_000;
const PERCENTAGE: u128 = 8;

#[test]
fn test_lock_and_claim_flow() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds (Tier 4: 500k C4E -> 8% APR)
    let lock_amount = 500_000 * C4E_1;
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: 31_536_000 }, // 1 year
        &[coin(lock_amount, DENOM)],
    ).unwrap();

    // 3. verify lockup details
    let lockup: Lockup = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    ).unwrap();
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    assert_eq!(lockup.annual_percentage_rate, Decimal::percent(PERCENTAGE as u64));

    // 4. advance time by half a year
    app.update_block(|block| {
        block.time = block.time.plus_seconds(31_536_000 / 2);
    });

    // 5. query claimable rewards
    let rewards: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();
    // expected: 500k * PERCENTAGE / 2 = 4000 C4E
    let expected_rewards = lock_amount * PERCENTAGE / 100 / 2;
    println!("Claimable rewards (half a year): {}", rewards.amount);
    println!("Expected rewards (half a year): {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 6. user claims rewards
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRewards {},
        &[],
    ).unwrap();

    // 7. verify user balance increased
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // balance = initial - lock + rewards_claimed
    let expected_balance = 1_000_000_000_000_000 - lock_amount + expected_rewards;
    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 8. advance time past the unlock period
    app.update_block(|block: &mut BlockInfo| {
        block.time = block.time.plus_seconds(31_536_000 / 2 + 1);
    });

    // 9. unlock principal
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UnlockPrincipal {},
        &[],
    ).unwrap();

    // 10. verify final balance
    // the user should get:
    // - back their initial funds (1_000_000_000_000)
    // - plus total rewards earned (exactly 8% of lock_amount for a full year)
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let total_rewards = lock_amount * PERCENTAGE / 100;
    let expected_final_balance = 1_000_000_000_000_000u128 + total_rewards;

    println!("Final balance: {}", final_user_balance.amount);
    println!("Expected final balance: {}", expected_final_balance);
    println!("Total rewards expected: {}", total_rewards);

    // since rewards are calculated precisely, allow for some rounding difference
    let balance_diff = if final_user_balance.amount.u128() > expected_final_balance {
        final_user_balance.amount.u128() - expected_final_balance
    } else {
        expected_final_balance - final_user_balance.amount.u128()
    };

    // allow up to 50 units difference for rounding (just in case)
    assert!(balance_diff <= 50, "Balance difference too large: {}", balance_diff);

    // 11. verify lockup is gone
    let res: StdResult<Lockup> = app.wrap().query_wasm_smart(
        contract_addr,
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    );
    assert!(res.is_err());
}

#[test]
fn test_rewards_stop_after_lockup_period() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds for 1 year (Tier 4: 500k C4E -> 8% APR)
    let lock_amount = 500_000 * C4E_1;
    let lock_duration = 31_536_000; // 1 year in seconds
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount, DENOM)],
    ).unwrap();

    // 3. advance time exactly to the unlock time
    app.update_block(|block| {
        block.time = block.time.plus_seconds(lock_duration);
    });

    // 4. query claimable rewards at unlock time
    let rewards_at_unlock: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();

    // Expected: exactly 1 year of rewards = 500k * 8% = 40k C4E
    let expected_full_year_rewards = lock_amount * PERCENTAGE / 100;
    println!("Rewards at unlock time: {}", rewards_at_unlock.amount);
    println!("Expected full year rewards: {}", expected_full_year_rewards);

    // Allow small rounding differences
    let diff = if rewards_at_unlock.amount.u128() > expected_full_year_rewards {
        rewards_at_unlock.amount.u128() - expected_full_year_rewards
    } else {
        expected_full_year_rewards - rewards_at_unlock.amount.u128()
    };
    assert!(diff <= 1000, "Rewards at unlock time should be approximately full year rewards");

    // 5. advance time significantly past the unlock period (2 additional years)
    app.update_block(|block| {
        block.time = block.time.plus_seconds(2 * 31_536_000); // 2 more years
    });

    // 6. query claimable rewards after additional time
    let rewards_after_extended_time: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();

    println!("Rewards after 2 additional years: {}", rewards_after_extended_time.amount);

    // rewards should NOT have increased beyond the unlock time
    assert_eq!(
        rewards_at_unlock.amount,
        rewards_after_extended_time.amount,
        "Rewards should not accumulate after lockup period ends"
    );

    // 7. user claims rewards
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRewards {},
        &[],
    ).unwrap();

    // 8. advance time even more (another year)
    app.update_block(|block| {
        block.time = block.time.plus_seconds(31_536_000); // 1 more year
    });

    // 9. query claimable rewards after claiming and more time passing
    let rewards_after_claim_and_time: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();

    println!("Rewards after claim and more time: {}", rewards_after_claim_and_time.amount);

    // should be zero since no new rewards should accumulate after unlock time
    assert_eq!(
        rewards_after_claim_and_time.amount,
        Uint128::zero(),
        "No new rewards should accumulate after lockup period and claiming"
    );

    // 10. finally unlock principal - should work even years after lockup period
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UnlockPrincipal {},
        &[],
    ).unwrap();

    // 11. verify user got back their principal amount
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // should have: initial balance - locked amount + full year rewards + principal back
    let expected_final_balance = 1_000_000_000_000_000u128 - lock_amount + expected_full_year_rewards + lock_amount;

    println!("Final balance: {}", final_user_balance.amount);
    println!("Expected final balance: {}", expected_final_balance);

    let balance_diff = if final_user_balance.amount.u128() > expected_final_balance {
        final_user_balance.amount.u128() - expected_final_balance
    } else {
        expected_final_balance - final_user_balance.amount.u128()
    };

    assert!(balance_diff <= 2000, "Final balance should match expected amount");
}

#[test]
fn test_claim_rewards_after_lockup_period_ends() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds for 1 year
    let lock_amount = 500_000 * C4E_1;
    let lock_duration = 31_536_000; // 1 year in seconds
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount, DENOM)],
    ).unwrap();

    // 3. advance time way past the unlock period (3 years total)
    app.update_block(|block| {
        block.time = block.time.plus_seconds(3 * 31_536_000);
    });

    // 4. user tries to claim rewards 2 years after lockup period ended
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRewards {},
        &[],
    ).unwrap();

    // 5. check user balance - should only get 1 year worth of rewards, not 3 years
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let expected_rewards = lock_amount * PERCENTAGE / 100; // only 1 year worth
    let expected_balance = 1_000_000_000_000_000u128 - lock_amount + expected_rewards;

    println!("User balance after claiming: {}", user_balance.amount);
    println!("Expected balance (with 1 year rewards only): {}", expected_balance);

    let balance_diff = if user_balance.amount.u128() > expected_balance {
        user_balance.amount.u128() - expected_balance
    } else {
        expected_balance - user_balance.amount.u128()
    };

    assert!(balance_diff <= 2000, "User should only get 1 year worth of rewards, not more");

    // 6. verify no more rewards are claimable
    let remaining_rewards: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();

    assert_eq!(remaining_rewards.amount, Uint128::zero(), "No more rewards should be claimable");
}

#[test]
fn test_deposit_with_lockup() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds for 1 year
    let lock_amount = 500_000 * C4E_1;
    let lock_duration = 31_536_000; // 1 year in seconds
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount, DENOM)],
    ).unwrap();

    let lock_amount2 = 100_000 * C4E_1;

    // 3. verify lockup details
    let lockup: Lockup = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    ).unwrap();
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    assert_eq!(lockup.annual_percentage_rate, Decimal::percent(PERCENTAGE as u64));

    // 4. user tries to lock more funds
    // should return an error "Account already has an active lockup"
    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount2, DENOM)],
    );
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("Account already has an active lockup"),
        "Unexpected error message: {}",
        err_msg
    );

}

#[test]
fn deposit_above_limit() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds for 1 year
    let lock_amount = 1_050_000 * C4E_1; // deposits amount higher than the available limit (1 million C4E)
    let lock_duration = 31_536_000; // 1 year in seconds

    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount, DENOM)],
    );

    // 3. transaction should return an error:
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("Deposit exceeds maximum limit"),
        "Unexpected error message: {}",
        err_msg
    );
}

#[test]
fn deposit_below_limit() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds for 1 year
    let lock_amount = 5_000 * C4E_1; // deposits amount lower than possible (10K C4E)
    let lock_duration = 31_536_000; // 1 year in seconds

    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: lock_duration },
        &[coin(lock_amount, DENOM)],
    );

    // 3. transaction should return an error:
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("Deposit below minimum required amount"),
        "Unexpected error message: {}",
        err_msg
    );
}


#[test]
fn test_lock_and_claim_for_tier3() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addrewsses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    let deposit_amount = coin(500_000 * C4E_1, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();

    // 2. user locks funds (Tier 2: 50k C4E -> 3.5% APR)
    let lock_amount: u128 = 50_000 * C4E_1;
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock { duration: 31_536_000 }, // 1 year
        &[coin(lock_amount, DENOM)],
    ).unwrap();

    // 3. verify lockup details
    let lockup: Lockup = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    ).unwrap();
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    //assert_eq!(lockup.annual_percentage_rate, Decimal::percent(PERCENTAGE2 as u64));
    // 3.5% APR should be exactly 0.035
    assert_eq!(lockup.annual_percentage_rate, Decimal::from_ratio(35u128, 1000u128));
    // 4. advance time by half
    app.update_block(|block| {
        block.time = block.time.plus_seconds(31_536_000 / 2);
    });

    // 5. query claimable rewards
    let rewards: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap();
    // expected: 50k * 3.5% / 2 = 875 C4E
    let expected_rewards = lock_amount * 35 / 1000 / 2; // 3.5% as 35/1000
    println!("Claimable rewards (half a year): {}", rewards.amount);
    println!("Expected rewards (half a year): {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 6. user claims rewards
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRewards {},
        &[],
    ).unwrap();

    // 7. verify user balance increased
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // balance = initial - lock + rewards_claimed
    let expected_balance = 1_000_000_000_000_000 - lock_amount + expected_rewards;
    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 8. advance time past the unlock period
    app.update_block(|block: &mut BlockInfo| {
        block.time = block.time.plus_seconds(31_536_000 / 2 + 1);
    });

    // 9. unlock principal
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UnlockPrincipal {},
        &[],
    ).unwrap();

    // 10. verify final balance
    // the user should get:
    // - back their initial funds (1_000_000_000_000)
    // - plus total rewards earned (exactly 3.5% of lock_amount for a full year)
    // - 3.5% of 50 000 = 1 750
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let total_rewards = lock_amount * 35 / 1000;
    let expected_final_balance = 1_000_000_000_000_000u128 + total_rewards;

    println!("Final balance: {}", final_user_balance.amount);
    println!("Expected final balance: {}", expected_final_balance);
    println!("Total rewards expected: {}", total_rewards);

    // since rewards are calculated precisely, allow for some rounding difference
    let balance_diff = if final_user_balance.amount.u128() > expected_final_balance {
        final_user_balance.amount.u128() - expected_final_balance
    } else {
        expected_final_balance - final_user_balance.amount.u128()
    };

    // allow up to 50 units difference for rounding (just in case)
    assert!(balance_diff <= 50, "Balance difference too large: {}", balance_diff);

    // 11. verify lockup is gone
    let res: StdResult<Lockup> = app.wrap().query_wasm_smart(
        contract_addr,
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    );
    assert!(res.is_err());

}
