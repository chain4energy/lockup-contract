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
    // rewards past the first year will now include the 1% and 2% APR increase
    let expected_three_year_rewards =
        lock_amount * 8 / 100 +
        lock_amount * 9 / 100 + // Year 2: 9% APR
        lock_amount * 10 / 100; // Year 3: 10% APR

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

    // should be 1 year worth of rewards since rewards continue to accumulate (with extra APR)
    let expected_one_more_year_rewards = lock_amount * 11 / 100; // 1 year at 11% APR
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
    let expected_final_balance = 1_000_000_000_000_000u128 + total_rewards_claimed;

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
    let expected_rewards =
        lock_amount * 8 / 100 + // 1 year at 8% APR
        lock_amount * 9 / 100 + // 2 years at 9% APR
        lock_amount * 10 / 100;  // 3 years at 10% APR
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

    // check if theres only one lockup after second deposit
    let (lockup_count, _): (Uint128, Coin) = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetSumLockupsAndDeposits {}
    ).unwrap();

    println!("Total lockups in contract: {}", lockup_count);
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


#[test]
fn test_progressive_apr_10year() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 1_000_000 * C4E_1);

    // 2. user locks 100k C4E (Tier 3: 5% base APR)
    let lock_amount = 100_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    // 3. advance time by 10 years
    advance_time(&mut app, 10 * 31_536_000); // 10 years

    let expected_rewards =
        lock_amount * 5 / 100 +  // Year 1: 5%
        lock_amount * 6 / 100 +  // Year 2: 6%
        lock_amount * 7 / 100 +  // Year 3: 7%
        lock_amount * 8 / 100 +  // Year 4: 8%
        lock_amount * 9 / 100 +  // Year 5: 9%
        lock_amount * 10 / 100 + // Year 6: 10%
        lock_amount * 11 / 100 + // Year 7: 11%
        lock_amount * 12 / 100 + // Year 8: 12%
        lock_amount * 13 / 100 + // Year 9: 13%
        lock_amount * 14 / 100;  // Year 10: 14%
    // = 95_000_000_000

    // 4. query rewards
    let rewards = query_rewards(&app, &contract_addr, &user_addr);
    println!("Total rewards after 10 years: {}", rewards.amount);
    println!("Expected rewards after 10 years: {}", expected_rewards);

    // 5. rewards should match expected rewards
   assert_eq!(rewards.amount, Uint128::new(expected_rewards));
}

#[test]
fn test_deposit_rewards_query() {
    let (mut app, contract_addr) = proper_instantiate();
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits rewards
    deposit_rewards(&mut app, &contract_addr, &admin_addr, 750_000 * C4E_1);

    //2. query deposited rewards
    let deposited_rewards: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetDepositedRewards {}
    ).unwrap();

    println!("Deposited rewards:            {} {}", deposited_rewards.amount, deposited_rewards.denom);

    // should match the deposited amount
    assert_eq!(deposited_rewards.amount, Uint128::new(750_000 * C4E_1));
    assert_eq!(deposited_rewards.denom, DENOM.to_string());

    //3. user locks funds
    let lock_amount = 100_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user_addr, lock_amount);

    //4. query deposited rewards again, should remain unchanged
    let deposited_rewards_after_lock: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetDepositedRewards {}
    ).unwrap();

    // 5. advance time by half a year
    advance_time(&mut app, 31_536_000 / 2);

    // check user rewards
    let user_rewards: Coin = query_rewards(&app, &contract_addr, &user_addr);

    // expected: 100k * 5% / 2 = 2,500 C4E
    let expected_user_rewards = lock_amount * 5 / 100 / 2;
    assert_eq!(user_rewards.amount, Uint128::new(expected_user_rewards));

    // query deposited rewards again, should now subtract pending rewards
    let deposited_rewards_after_time: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetDepositedRewards {}
    ).unwrap();

    println!("Deposited rewards after lock: {} {}", deposited_rewards_after_lock.amount, deposited_rewards_after_lock.denom);
    println!("Deposited rewards after time: {} {}", deposited_rewards_after_time.amount, deposited_rewards_after_time.denom);
    println!("User pending rewards:         {} {}", user_rewards.amount, user_rewards.denom);

    // the rewards per year calculation always considers the highest APR for the tier so 5% base + 10% extra = 15% APR
    // so for 100k C4E locked, the rewards per year should be 15k C4E
    let rewards_per_year: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetAllRewards {}
    ).unwrap();

    let expected_rewards_per_year = 100_000 * C4E_1 * 5 / 100;

    println!("Expected rewards per year:    {}", expected_rewards_per_year);
    println!("Rewards distributed per year: {} {}", rewards_per_year.amount, rewards_per_year.denom);

    // 6. test coin availability check - should return true since we have plenty of funds
    let is_sufficient_funds: bool = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::CheckCoinAvailability {}
    ).unwrap();

    println!("Sufficient funds available:   {}", is_sufficient_funds);
    assert!(is_sufficient_funds, "Should have sufficient funds for rewards");

    advance_time(&mut app, 31_536_000 / 2);

    let user_rewards_after_full_year: Coin = query_rewards(&app, &contract_addr, &user_addr);
    println!("User rewards after full year: {} {}", user_rewards_after_full_year.amount, user_rewards_after_full_year.denom);

    // should match the original deposited amount minus the pending rewards
    assert_eq!(deposited_rewards_after_lock.amount, Uint128::new(750_000 * C4E_1));
    assert_eq!(deposited_rewards_after_lock.denom, DENOM.to_string());

    // available rewards should be original deposit minus pending rewards
    let expected_available_rewards = 750_000 * C4E_1 - expected_user_rewards;
    assert_eq!(deposited_rewards_after_time.amount, Uint128::new(expected_available_rewards));
    assert_eq!(deposited_rewards_after_time.denom, DENOM.to_string());
}

#[test]
fn test_coin_availability_insufficient_funds() {
    let (mut app, contract_addr) = proper_instantiate();
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits a small amount of rewards (not enough for yearly requirements)
    let small_deposit = 1_000 * C4E_1; // Only 1k C4E
    deposit_rewards(&mut app, &contract_addr, &admin_addr, small_deposit);

    // 2. try to lock a large amount that will require more yearly rewards than deposited
    let large_lock_amount = 500_000 * C4E_1; // 500k C4E (Tier 4: 8% APR)

    // this should fail due to insufficient reward funds
    // required yearly rewards = 500k * (8% + 10% max increase) = 500k * 18% = 90k C4E
    // available rewards = 1k C4E (way less than required)
    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock {},
        &[coin(large_lock_amount, DENOM)],
    );

    // should fail due to insufficient reward funds
    assert!(result.is_err(), "Lock should fail due to insufficient reward funds");

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Insufficient reward funds"),
            "Error should mention insufficient reward funds");

    // 3. test coin availability check - should return true since no lockups exist
    let is_sufficient_funds: bool = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::CheckCoinAvailability {}
    ).unwrap();

    assert!(is_sufficient_funds, "Should have sufficient funds since no lockups exist");

    let deposited_rewards: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetDepositedRewards {}
    ).unwrap();

    let required_per_year: Coin = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetAllRewards {}
    ).unwrap();

    println!("Small deposit amount:         {}", small_deposit);
    println!("Large lock amount:            {}", large_lock_amount);
    println!("Available rewards:            {} {}", deposited_rewards.amount, deposited_rewards.denom);
    println!("Required per year:            {} {}", required_per_year.amount, required_per_year.denom);
    println!("Sufficient funds available:   {}", is_sufficient_funds);

    // since no lockup happened, there should be no required rewards
    assert_eq!(deposited_rewards.amount, Uint128::new(small_deposit));
    assert_eq!(required_per_year.amount, Uint128::zero());
}

#[test]
fn test_get_sum_lockups_and_deposits() {
    let (mut app, contract_addr) = proper_instantiate();

    // get proper addresses
    let admin_addr = app.api().addr_make(ADMIN);
    let user1_addr = app.api().addr_make(USER_1);
    let user2_addr = app.api().addr_make("user2");

    // add user2 with initial balance
    app.sudo(cw_multi_test::SudoMsg::Bank(
        cw_multi_test::BankSudo::Mint {
            to_address: user2_addr.to_string(),
            amount: vec![coin(1_000_000_000_000_000u128, DENOM)],
        },
    )).unwrap();

    // 1. admin deposits rewards
    let rewards_deposited = 1_000_000 * C4E_1;
    deposit_rewards(&mut app, &contract_addr, &admin_addr, rewards_deposited);

    // 2. initially should have 0 lockups and 0 principal amount
    let (lockup_count, principal_sum): (Uint128, Coin) = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetSumLockupsAndDeposits {}
    ).unwrap();

    assert_eq!(lockup_count, Uint128::zero());
    assert_eq!(principal_sum.amount, Uint128::zero()); // No principal locked yet
    assert_eq!(principal_sum.denom, DENOM);

    // 3. user1 locks 100k C4E (Tier 3)
    let lock_amount1 = 100_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user1_addr, lock_amount1);

    // 4. user2 locks 200k C4E (Tier 3)
    let lock_amount2 = 200_000 * C4E_1;
    lock_funds(&mut app, &contract_addr, &user2_addr, lock_amount2);

    // 5. query after lockups
    let (lockup_count, principal_sum): (Uint128, Coin) = app.wrap().query_wasm_smart(
        contract_addr.clone(),
        &QueryMsg::GetSumLockupsAndDeposits {}
    ).unwrap();

    // should have 2 lockups now
    assert_eq!(lockup_count, Uint128::new(2));

    // advance time by half a year, nothing should change in principal sum
    advance_time(&mut app, 31_536_000 / 2);

    // total principal should only be the sum of locked amounts
    let expected_principal_total = lock_amount1 + lock_amount2;
    assert_eq!(principal_sum.amount, Uint128::new(expected_principal_total));
    assert_eq!(principal_sum.denom, DENOM);

    println!("Lockup count: {}", lockup_count);
    println!("Total principal locked: {}", principal_sum.amount);
    println!("Expected principal total: {}", expected_principal_total);
}


#[test]
fn test_lock_insufficient_reward_funds() {
    let (mut app, contract_addr) = proper_instantiate();
    let admin_addr = app.api().addr_make(ADMIN);
    let user_addr = app.api().addr_make(USER_1);

    // 1. admin deposits minimal rewards (only 1k C4E)
    let minimal_deposit = 1_000 * C4E_1;
    deposit_rewards(&mut app, &contract_addr, &admin_addr, minimal_deposit);

    // 2. try to lock a large amount that would require more yearly rewards than available
    let large_lock_amount = 500_000 * C4E_1; // 500k C4E (Tier 4)

    // 3. attempt should fail
    let result = app.execute_contract(
        user_addr.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Lock {},
        &[coin(large_lock_amount, DENOM)],
    );

    // 4. should fail with insufficient funds error
    assert!(result.is_err(), "Lock should fail due to insufficient reward funds");

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Insufficient reward funds"),
            "Error should mention insufficient reward funds");
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
