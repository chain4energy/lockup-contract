use super::common::*;
use crate::msg::{ExecuteMsg, QueryMsg};
use crate::state::Lockup;
use cosmwasm_std::{coin, Coin, Decimal, Uint128, StdResult, Addr};
use cw_multi_test::{Executor, App};

const C4E_1: u128 = 1_000_000;

#[test]
fn test_lock_and_claim_flow() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds (Tier 4: 500k C4E -> 8% APR)
    let lock_amount = 500_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    // 3. verify lockup details
    let lockup = query_lockup(&app, &contract_addr, &user_addr);
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    assert_eq!(lockup.annual_percentage_rate, Decimal::percent(8 as u64));

    // 4. advance time by half a year
    advance_time(&mut app, 31_536_000 / 2);

    // 5. query claimable rewards
    let rewards = query_rewards(&app, &contract_addr, &user_addr);

    // expected: 500k * 8 / 2 = 4000 C4E
    let expected_rewards = lock_amount * 8 / 100 / 2;
    println!("Claimable rewards (half a year):   {}", rewards.amount);
    println!("Expected rewards (half a year):    {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 6. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 7. verify user balance increased
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // balance = initial - lock + rewards_claimed
    let expected_balance = 1_000_000_000_000_000 - lock_amount + expected_rewards;
    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 8. advance time past the unlock period
    advance_time(&mut app, 31_536_000 / 2 + 1);

    // 9. unlock principal
    unlock_lockup(&mut app, &contract_addr, &user_addr);

    // 10. verify final balance
    // the user should get:
    // - back their initial funds (1_000_000_000_000)
    // - plus total rewards earned (exactly 8% of lock_amount for a full year)
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let total_rewards = lock_amount * 8 / 100;
    let expected_final_balance = 1_000_000_000_000_000u128 + total_rewards;

    println!("Final balance:            {}", final_user_balance.amount);
    println!("Expected final balance:   {}", expected_final_balance);
    println!("Total rewards expected:   {}", total_rewards);

    // check final balance
    assert_eq!(final_user_balance.amount, Uint128::new(expected_final_balance));

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
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds for 1 year (Tier 4: 500k C4E -> 8% APR)
    let lock_amount = 500_000 * C4E_1;
    let lock_duration = 31_536_000; // 1 year in seconds
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    // 3. advance time exactly to the unlock time
    advance_time(&mut app, lock_duration);

    // 4. query claimable rewards at unlock time
    let rewards_at_unlock: Coin = query_rewards(&app, &contract_addr, &user_addr);

    // Expected: exactly 1 year of rewards = 500k * 8% = 40k C4E
    let expected_full_year_rewards = lock_amount * 8 / 100;
    println!("Rewards at unlock time:       {}", rewards_at_unlock.amount);
    println!("Expected full year rewards:   {}", expected_full_year_rewards);

    // check rewards amount
    assert_eq!(rewards_at_unlock.amount, Uint128::new(expected_full_year_rewards));

    // 5. advance time significantly past the unlock period (2 additional years)
    advance_time(&mut app, 2 * 31_536_000); // 2 more years

    // 6. query claimable rewards after additional time
    let rewards_after_extended_time: Coin = query_rewards(&app, &contract_addr, &user_addr);

    println!("Rewards after 2 additional years: {}", rewards_after_extended_time.amount);

    // rewards should NOT have increased beyond the unlock time
    assert_eq!(
        rewards_at_unlock.amount,
        rewards_after_extended_time.amount,
        "Rewards should not accumulate after lockup period ends"
    );

    // 7. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 8. advance time even more (another year)
    advance_time(&mut app, 31_536_000); // 1 more year

    // 9. query claimable rewards after claiming and more time passing
    let rewards_after_claim_and_time: Coin = query_rewards(&app, &contract_addr, &user_addr);

    println!("Rewards after claim and more time: {}", rewards_after_claim_and_time.amount);

    // should be zero since no new rewards should accumulate after unlock time
    assert_eq!(
        rewards_after_claim_and_time.amount,
        Uint128::zero(),
        "No new rewards should accumulate after lockup period and claiming"
    );

    // 10. finally unlock principal - should work even years after lockup period
    unlock_lockup(&mut app, &contract_addr, &user_addr);

    // 11. verify user got back their principal amount
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // should have: initial balance - locked amount + full year rewards + principal back
    let expected_final_balance = 1_000_000_000_000_000u128 - lock_amount + expected_full_year_rewards + lock_amount;

    println!("Final balance:            {}", final_user_balance.amount);
    println!("Expected final balance:   {}", expected_final_balance);

    assert_eq!(final_user_balance.amount, Uint128::new(expected_final_balance));
}

#[test]
fn test_claim_rewards_after_lockup_period_ends() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds for 1 year
    let lock_amount = 500_000 * C4E_1;
    let _lock_duration = 31_536_000; // 1 year in seconds
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    // 3. advance time way past the unlock period (3 years total)
    advance_time(&mut app, 3 * 31_536_000);

    // 4. user tries to claim rewards 2 years after lockup period ended
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 5. check user balance - should only get 1 year worth of rewards, not 3 years
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let expected_rewards = lock_amount * 8 / 100; // only 1 year worth
    let expected_balance = 1_000_000_000_000_000u128 - lock_amount + expected_rewards;

    println!("User balance after claiming:                  {}", user_balance.amount);
    println!("Expected balance (with 1 year rewards only):  {}", expected_balance);

    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 6. verify no more rewards are claimable
    let remaining_rewards: Coin = query_rewards(&app, &contract_addr, &user_addr);

    assert_eq!(remaining_rewards.amount, Uint128::zero(), "No more rewards should be claimable");
}

#[test]
fn test_deposit_with_lockup() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds for 1 year
    let lock_amount = 100_000 * C4E_1;
    let _lock_duration = 31_536_000; // 1 year in seconds
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    let lock_amount2 = 400_000 * C4E_1;

    // 3. verify lockup details
    let lockup: Lockup = query_lockup(&app, &contract_addr, &user_addr);

    println!("APR: {}", lockup.annual_percentage_rate);
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    assert_eq!(lockup.annual_percentage_rate, Decimal::percent(5 as u64));

    // advance time by half a year
    advance_time(&mut app, 31_536_000 / 2);

    // 4. user claims rewards
    // the reward should be at 5% APR
    let rewards: Coin = query_rewards(&app, &contract_addr, &user_addr);

    let expected_rewards = lock_amount * 5 / 100 / 2;
    // should be 2.5% of 100 000 C4E = 2500
    println!("Claimable rewards (half a year): {}", rewards.amount);
    println!("Expected rewards (half a year) : {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 5. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 6. verify user balance increased
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // balance = initial - lock + rewards_claimed
    let expected_balance = 1_000_000_000_000_000 - lock_amount + expected_rewards;
    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 7. user tries to lock more funds
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount2);


    println!("User locked original amount:      {}", lock_amount);
    println!("User locked additional amount:    {}", lock_amount2);
    let expected_total_lock_amount = lock_amount + lock_amount2;

    // 8. verify lockup details
    let lockup: Lockup = query_lockup(&app, &contract_addr, &user_addr);

    println!("Final APR: {}", lockup.annual_percentage_rate);

    assert_eq!(lockup.principal_amount.amount, Uint128::new(expected_total_lock_amount));
    assert_eq!(lockup.annual_percentage_rate, Decimal::percent(8 as u64));

    // 9. advance block time until the end of the lockup
    advance_time(&mut app, 31_536_000 / 2 + 1);

    // 10. check claimable balance
    // the reward should be at 8% APR
    // should be 4% of 500 000 C4E = 20 000 C4E
    let rewards: Coin = query_rewards(&app, &contract_addr, &user_addr);

    let expected_rewards = expected_total_lock_amount * 8 / 100 / 2;

    println!("Claimable rewards: {}", rewards.amount);
    println!("Expected rewards:  {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 11. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // calculate expected rewards from the entire lockup
    // First half: 100k at 5% for 6 months = 2.5k (already claimed)
    // Second half: 500k total at 8% for 6 months = 20k
    let final_expected_rewards = lock_amount * 5 / 100 / 2 + expected_total_lock_amount * 8 / 100 / 2;

    // 12. unlock principal
    unlock_lockup(&mut app, &contract_addr, &user_addr);

    // 13. verify final balance
    // should be the original amount plus locked amount plus all rewards = 1 000 000 000 + 2 500 + 20 000 = 1 000 022 500
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();

    let expected_final_balance = 1_000_000_000_000_000 + final_expected_rewards;

    println!("Final user balance:        {}", final_user_balance.amount);
    println!("Expected final balance:    {}", expected_final_balance);

    assert_eq!(final_user_balance.amount, Uint128::new(expected_final_balance));

}

#[test]
fn deposit_above_limit() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds for 1 year
    let lock_amount = 1_050_000 * C4E_1; // deposits amount higher than the available limit (1 million C4E)
    let _lock_duration = 31_536_000; // 1 year in seconds

    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock {},
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
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds for 1 year
    let lock_amount = 5_000 * C4E_1; // deposits amount lower than possible (10K C4E)
    let _lock_duration = 31_536_000; // 1 year in seconds

    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock {},
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
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 500_000 * C4E_1);

    // 2. user locks funds (Tier 2: 50k C4E -> 3.5% APR)
    let lock_amount: u128 = 50_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    // 3. verify lockup details
    let lockup: Lockup = query_lockup(&app, &contract_addr, &user_addr);
    assert_eq!(lockup.principal_amount.amount, Uint128::new(lock_amount));
    //assert_eq!(lockup.annual_percentage_rate, Decimal::percent(PERCENTAGE2 as u64));

    // 3.5% APR should be exactly 0.035
    assert_eq!(lockup.annual_percentage_rate, Decimal::from_ratio(35u128, 1000u128));
    // 4. advance time by half
    advance_time(&mut app, 31_536_000 / 2);

    // 5. query claimable rewards
    let rewards: Coin = query_rewards(&app, &contract_addr, &user_addr);

    // expected: 50k * 3.5% / 2 = 875 C4E
    let expected_rewards = lock_amount * 35 / 1000 / 2; // 3.5% as 35/1000
    println!("Claimable rewards (half a year):  {}", rewards.amount);
    println!("Expected rewards (half a year):   {}", expected_rewards);
    assert_eq!(rewards.amount, Uint128::new(expected_rewards));

    // 6. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 7. verify user balance increased
    let user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // balance = initial - lock + rewards_claimed
    let expected_balance = 1_000_000_000_000_000 - lock_amount + expected_rewards;
    assert_eq!(user_balance.amount, Uint128::new(expected_balance));

    // 8. advance time past the unlock period
    advance_time(&mut app, 31_536_000);

    // 9. unlock principal
    unlock_lockup(&mut app, &contract_addr, &user_addr);

    // 10. verify final balance
    // the user should get:
    // - back their initial funds (1_000_000_000_000)
    // - plus total rewards earned (exactly 3.5% of lock_amount for a full year)
    // - 3.5% of 50 000 = 1 750
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    let total_rewards = lock_amount * 35 / 1000;
    let expected_final_balance = 1_000_000_000_000_000u128 + total_rewards;

    println!("Final balance:            {}", final_user_balance.amount);
    println!("Expected final balance:   {}", expected_final_balance);
    println!("Total rewards expected:   {}", total_rewards);

    // check final balance
    assert_eq!(final_user_balance.amount, Uint128::new(expected_final_balance));

    // 11. verify lockup is gone
    let res: StdResult<Lockup> = app.wrap().query_wasm_smart(
        contract_addr,
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    );
    assert!(res.is_err());

}


// helper functions
fn deposit_rewards(app: &mut App, contract_addr: &Addr, admin_addr: &Addr, amount: u128) {
    let deposit_amount = coin(amount, DENOM);
    app.execute_contract(
        admin_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositRewards {},
        &[deposit_amount],
    ).unwrap();
}

fn lock_funds(app: &mut App, contract_addr: &Addr, user_addr: &Addr, amount: u128) {
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock {},
        &[coin(amount, DENOM)],
    ).unwrap();
}

fn claim_rewards(app: &mut App, contract_addr: &Addr, user_addr: &Addr) {
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRewards {},
        &[],
    ).unwrap();
}

fn unlock_lockup(app: &mut App, contract_addr: &Addr, user_addr: &Addr) {
    app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UnlockPrincipal {},
        &[],
    ).unwrap();
}

fn query_rewards(app: &App, contract_addr: &Addr, user_addr: &Addr) -> Coin {
    app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetClaimableRewards { address: user_addr.to_string() }
    ).unwrap()
}

fn query_lockup(app: &App, contract_addr: &Addr, user_addr: &Addr) -> Lockup {
    app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetLockup { address: user_addr.to_string() }
    ).unwrap()
}

fn advance_time(app: &mut App, seconds: u64) {
    app.update_block(|block| {
        block.time = block.time.plus_seconds(seconds);
    });
}
