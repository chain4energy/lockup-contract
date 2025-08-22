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
    advance_time(&mut app, 31_536_000 / 2);

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
fn test_rewards_continue_after_lockup_period() {
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

    // rewards should NOW continue to accumulate even after unlock time (3 years total)
    let expected_three_year_rewards = lock_amount * 8 / 100 * 3; // 3 years at 8% APR
    assert_eq!(
        rewards_after_extended_time.amount,
        Uint128::new(expected_three_year_rewards),
        "Rewards should continue to accumulate after lockup period ends"
    );

    // 7. user claims rewards
    claim_rewards(&mut app, &contract_addr, &user_addr);

    // 8. advance time even more (another year)
    advance_time(&mut app, 31_536_000); // 1 more year

    // 9. query claimable rewards after claiming and more time passing
    let rewards_after_claim_and_time: Coin = query_rewards(&app, &contract_addr, &user_addr);

    println!("Rewards after claim and more time: {}", rewards_after_claim_and_time.amount);

    // should be 1 year worth of rewards since rewards continue to accumulate
    let expected_one_more_year_rewards = lock_amount * 8 / 100; // 1 year at 8% APR
    assert_eq!(
        rewards_after_claim_and_time.amount,
        Uint128::new(expected_one_more_year_rewards),
        "New rewards should accumulate after claiming"
    );

    // 10. finally unlock principal - should work even years after lockup period
    unlock_lockup(&mut app, &contract_addr, &user_addr);

    // 11. verify user got back their principal amount
    let final_user_balance = app.wrap().query_balance(&user_addr, DENOM).unwrap();
    // should have: initial balance - locked amount + 3 years rewards + 1 year rewards + principal back
    // Total rewards: 3 years (claimed) + 1 year (new accumulation) = 4 years worth
    let total_rewards_claimed = expected_three_year_rewards + expected_one_more_year_rewards;
    let expected_final_balance = 1_000_000_000_000_000u128 - lock_amount + total_rewards_claimed + lock_amount;

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
    let expected_rewards = lock_amount * (8*3) / 100; // 3 years at 8% APR
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
    advance_time(&mut app, 31_536_000 / 2);

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
    advance_time(&mut app, 31_536_000 / 2);

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


#[test]
fn test_get_all_lockups_query() {
    use crate::msg::AllLockupsResponse;

    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user1_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 1_000_000 * C4E_1);

    // 2. user locks 100k C4E (Tier 3: 5% APR)
    let lock_amount1 = 100_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user1_addr, lock_amount1);

    // 3. query all lockups
    let all_lockups: AllLockupsResponse = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetAllLockups {}
    ).unwrap();

    // 4. verify the response
    assert_eq!(all_lockups.lockups.len(), 1);

    // find user1 lockup
    let user1_lockup = &all_lockups.lockups[0];
    assert_eq!(user1_lockup.address, user1_addr.to_string());
    assert_eq!(user1_lockup.principal_amount, Uint128::new(lock_amount1));
    assert_eq!(user1_lockup.annual_percentage_rate, Decimal::percent(5));
    assert_eq!(user1_lockup.tier, 3);

    println!("All lockups query test passed!");
    println!("User: {}", user1_lockup.address);
    println!("Amount: {}", user1_lockup.principal_amount);
    println!("APR: {}%", user1_lockup.annual_percentage_rate * Decimal::from_atomics(100u128, 0).unwrap());
    println!("Tier: {}", user1_lockup.tier);
    println!("Start Time: {}", user1_lockup.start_time);

    // verify that start_time is reasonable
    // should be close to current block time
    assert!(user1_lockup.start_time.seconds() > 0, "Start time should be set");
}

#[test]
fn test_start_time_preservation_on_tier_upgrade() {
    use crate::msg::AllLockupsResponse;

    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 1_000_000 * C4E_1);

    // 2. user locks initial amount (Tier 1: 10k C4E)
    let initial_amount = 10_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, initial_amount);

    // 3. query to get the initial start time
    let initial_lockups: AllLockupsResponse = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetAllLockups {}
    ).unwrap();
    let initial_start_time = initial_lockups.lockups[0].start_time;

    // 4. advance time a bit
    advance_time(&mut app, 1000); // 1000 seconds

    // 5. user locks more funds to upgrade tier (total: 500k = Tier 4)
    let additional_amount = 490_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, additional_amount);

    // 6. query again to verify start time is preserved
    let updated_lockups: AllLockupsResponse = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetAllLockups {}
    ).unwrap();

    let updated_lockup = &updated_lockups.lockups[0];

    // verify the tier was upgraded
    assert_eq!(updated_lockup.tier, 4);
    assert_eq!(updated_lockup.annual_percentage_rate, Decimal::percent(8));
    assert_eq!(updated_lockup.principal_amount, Uint128::new(initial_amount + additional_amount));

    // verify start time was preserved
    assert_eq!(updated_lockup.start_time, initial_start_time,
               "Start time should be preserved when upgrading tiers");

    println!("Tier upgrade preserves start time test passed!");
    println!("Initial start time: {}", initial_start_time);
    println!("Updated start time: {}", updated_lockup.start_time);
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
