use cosmwasm_std::{
    entry_point, to_json_binary, ensure, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128,
};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, AllLockupsResponse, LockupInfo};
use crate::state::{Config, Lockup, TierConfig, CONFIG, LOCKUPS};

const CONTRACT_NAME:    &str = "lockup-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const SECONDS_PER_YEAR:  u64 = 31_536_000;
const DECIMAL_PRECISION: u32 = 18;

// structure to hold lockup parameters
#[derive(Debug)]
struct LockupParams {
    unlock_time: cosmwasm_std::Timestamp,
    last_claim_time: cosmwasm_std::Timestamp,
    start_time: cosmwasm_std::Timestamp,
    apr: Decimal,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // use message sender as admin if not specified,
    // otherwise validate the provided addresses
    let admins = if let Some(admin_strings) = msg.admins {
        // validate all provided admin addresses
        let mut validated_admins = Vec::new();
        for admin_str in admin_strings {
            let admin_addr = deps.api.addr_validate(&admin_str)?;
            validated_admins.push(admin_addr);
        }
        validated_admins
    } else {
        // if no admins specified, use message sender as the only admin
        vec![info.sender.clone()]
    };

    // configurable lockup time, tier levels and rewards
    let config = Config {
        admins,
        denom: msg.denom,
        lockup_duration_seconds: msg.lockup_duration_seconds,
        tier_config: msg.tier_config,
    };
    CONFIG.save(deps.storage, &config)?;

    // create a comma-separated string of admin addresses
    let admins_str = config.admins.iter()
        .map(|addr| addr.to_string())
        .collect::<Vec<_>>()
        .join(",");

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admins", admins_str)
        .add_attribute("denom", config.denom))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Lock {}             => execute_lock(deps, env, info),
        ExecuteMsg::UnlockPrincipal {}  => execute_unlock_principal(deps, env, info),
        ExecuteMsg::ClaimRewards {}     => execute_claim_rewards(deps, env, info),
        ExecuteMsg::DepositRewards {}   => execute_deposit_rewards(deps, info),
    }
}

pub fn execute_lock(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // validate funds
    validate_lock_funds(&info.funds, &config.denom)?;

    let mut principal_amount = info.funds[0].clone();

    // validate deposit amount
    validate_deposit_amount(principal_amount.amount, &config.tier_config)?;

    // check sufficient funds for rewards
    check_sufficient_reward_funds(
        deps.as_ref(),
        env.clone(),
        &config,
        principal_amount.amount,
    )?;

    // handle existing lockup upgrade or create new lockup
    let (lockup_params, response) = if LOCKUPS.has(deps.storage, info.sender.clone()) {
        handle_lockup_upgrade(
            deps.storage,
            &env,
            &info.sender,
            &mut principal_amount,
            &config,
        )?
    } else {
        let params = create_new_lockup_params(&env, principal_amount.amount, &config);
        (params, Response::new())
    };

    // create and save the lockup
    let lockup = Lockup {
        owner: info.sender.clone(),
        principal_amount: principal_amount.clone(),
        unlock_time: lockup_params.unlock_time,
        annual_percentage_rate: lockup_params.apr,
        last_claim_time: lockup_params.last_claim_time,
        start_time: lockup_params.start_time,
    };
    LOCKUPS.save(deps.storage, info.sender.clone(), &lockup)?;

    Ok(response
        .add_attribute("method", "lock")
        .add_attribute("owner", info.sender)
        .add_attribute("amount", principal_amount.amount)
        .add_attribute("apr", lockup_params.apr.to_string()))
}

pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut lockup = LOCKUPS.load(deps.storage, info.sender.clone())?;
    let config = CONFIG.load(deps.storage)?;

    let rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;
    let available_rewards = query_available_rewards(deps.as_ref(), env.clone(), &config)?;

    ensure!(
        rewards.amount > Uint128::zero(),
        ContractError::ZeroRewards {}
    );

    ensure!(
        available_rewards.amount >= rewards.amount,
        ContractError::CannotClaimRewards {
            claim_amount:     rewards.amount,
            claim_denom:      rewards.denom.clone(),
            available_amount: available_rewards.amount,
            available_denom:  available_rewards.denom.clone(),
        }
    );

    // update the last claim time before sending funds
    lockup.last_claim_time = env.block.time;
    LOCKUPS.save(deps.storage, info.sender.clone(), &lockup)?;

    let bank_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![rewards.clone()],
    };

    Ok(Response::new()
        .add_message(bank_msg)
        .add_attribute("method", "claim_rewards")
        .add_attribute("owner", info.sender)
        .add_attribute("rewards_claimed", rewards.amount))
}

pub fn execute_unlock_principal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let lockup = LOCKUPS.load(deps.storage, info.sender.clone())?;
    let config = CONFIG.load(deps.storage)?;

    // check if the lockup period has passed
    ensure!(
        env.block.time >= lockup.unlock_time,
        ContractError::StillLocked { unlock_time: lockup.unlock_time }
    );

    // calculate final pending rewards
    let pending_rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;
    let available_rewards = query_available_rewards(deps.as_ref(), env.clone(), &config)?;

    // make sure contract has enough funds to pay out pending rewards
    // do not check for principal as it should always be available and its not counted in the available rewards pool
    ensure!(
        available_rewards.amount >= pending_rewards.amount,
        ContractError::CannotClaimRewards {
            claim_amount:     pending_rewards.amount,
            claim_denom:      available_rewards.denom.clone(),
            available_amount: available_rewards.amount,
            available_denom:  pending_rewards.denom.clone(),
        }
    );

    // final action, remove lockup from storage
    LOCKUPS.remove(deps.storage, info.sender.clone());  // TODO: dont remove lockup add a flag that tells its claimed

    // prepare to send both original and final rewards
    let mut total_payout = vec![lockup.principal_amount];
    if !pending_rewards.amount.is_zero() {
        total_payout.push(pending_rewards.clone());
    }

    let bank_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: total_payout,
    };

    Ok(Response::new()
        .add_message(bank_msg)
        .add_attribute("method", "unlock_principal")
        .add_attribute("owner", info.sender)
        .add_attribute("final_rewards_claimed", pending_rewards.amount))
}

// TODO: remove this later
// it is used in all tests currently
pub fn execute_deposit_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // only admins can deposit rewards
    ensure!(config.admins.contains(&info.sender), ContractError::Unauthorized {});
    ensure!(!info.funds.is_empty(), ContractError::NoFundsForDeposit {});

    // the funds are automatically added to the contracts balance
    // this function serves as a permissioned entry point
    Ok(Response::new()
        .add_attribute("method", "deposit_rewards")
        .add_attribute("depositor", info.sender)
        .add_attribute("amount", info.funds[0].amount.to_string())
        .add_attribute("denom", info.funds[0].denom.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetLockup { address } => {
            let addr = deps.api.addr_validate(&address)?;
            to_json_binary(&LOCKUPS.load(deps.storage, addr)?)
        }
        QueryMsg::GetClaimableRewards { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let lockup = LOCKUPS.load(deps.storage, addr)?;
            let config = CONFIG.load(deps.storage)?;
            let rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;
            to_json_binary(&rewards)
        }
        QueryMsg::GetAllLockups {} => {
            let config = CONFIG.load(deps.storage)?;
            let all_lockups = query_all_lockups(deps, &config)?;
            to_json_binary(&all_lockups)
        }
        QueryMsg::GetDepositedRewards {} => {
            let config = CONFIG.load(deps.storage)?;
            let deposited_rewards = query_available_rewards(deps, env, &config)?;
            to_json_binary(&deposited_rewards)
        }
        QueryMsg::GetSumLockupsAndDeposits {} => {
            let config = CONFIG.load(deps.storage)?;
            let sum_data = query_sum_lockups_and_deposits(deps, env, &config)?;
            to_json_binary(&sum_data)
        }
        QueryMsg::GetAllRewards {} => {
            let config = CONFIG.load(deps.storage)?;
            let all_rewards = query_rewards_per_year(deps, &config)?;
            to_json_binary(&all_rewards)
        }
        QueryMsg::CheckCoinAvailability {  } => {
            let is_sufficient = check_coin_availability(deps, env)?;
            to_json_binary(&is_sufficient)
        }
    }
}




// === HELPER FUNCTIONS ===

fn get_apr_for_amount(amount: Uint128, tier_config: &TierConfig) -> Decimal {
    match get_tier_for_amount(amount, tier_config) {
        4 => tier_config.tier_4_apr,
        3 => tier_config.tier_3_apr,
        2 => tier_config.tier_2_apr,
        1 => tier_config.tier_1_apr,
        _ => Decimal::zero(),
    }
}

fn get_tier_for_amount(amount: Uint128, tier_config: &TierConfig) -> u8 {
    match () {
        _ if amount >= tier_config.tier_4_min => 4,
        _ if amount >= tier_config.tier_3_min => 3,
        _ if amount >= tier_config.tier_2_min => 2,
        _ if amount >= tier_config.tier_1_min => 1,
        _ => 0,
    }
}

// iterate through all lockups
fn fold_lockups<T, F>(
    deps: Deps,
    initial: T,
    mut folder: F,
) -> StdResult<T>
where
    F: FnMut(T, &Lockup) -> StdResult<T>,
{
    LOCKUPS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .try_fold(initial, |acc, item| {
            let (_, lockup) = item?;
            folder(acc, &lockup)
        })
}




// =========== QUERY HELPERS ===========

fn query_all_lockups(deps: Deps, config: &Config) -> StdResult<AllLockupsResponse> {
    let lockups: StdResult<Vec<LockupInfo>> = LOCKUPS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|item| {
            let (addr, lockup) = item?;
            let tier = get_tier_for_amount(lockup.principal_amount.amount, &config.tier_config);
            Ok(LockupInfo {
                address: addr.to_string(),
                start_time: lockup.start_time,
                principal_amount: lockup.principal_amount.amount,
                annual_percentage_rate: lockup.annual_percentage_rate,
                tier,
            })
        })
        .collect();

    Ok(AllLockupsResponse {
        lockups: lockups?,
    })
}

fn query_sum_lockups_and_deposits(deps: Deps, _env: Env, config: &Config) -> StdResult<(Uint128, Coin)> {
    let (count, total_principal) = fold_lockups(
        deps,
        (0u128, Uint128::zero()),
        |acc, lockup| {
            let (count, total) = acc;
            Ok((count + 1, total + lockup.principal_amount.amount))
        },
    )?;

    let principal_coin = Coin::new(total_principal, &config.denom);
    Ok((Uint128::new(count), principal_coin))
}

// outputs the available amount of rewards for distribution, ignoring principal deposits and unclaimed pending rewards
fn query_available_rewards(deps: Deps, env: Env, config: &Config) -> StdResult<Coin> {
    // get total contract balance
    let total_balance = deps.querier.query_balance(env.contract.address, &config.denom)?;

    // calculate total principal amounts and pending rewards for all lockups
    let (total_principal, total_pending_rewards) = fold_lockups(
        deps,
        (Uint128::zero(), Uint128::zero()),
        |acc, lockup| {
            let (principal_sum, rewards_sum) = acc;
            let pending_rewards = calculate_rewards(lockup, env.block.time, &config.denom, &config.tier_config)?;
            Ok((
                principal_sum + lockup.principal_amount.amount,
                rewards_sum + pending_rewards.amount,
            ))
        },
    )?;

    // Available rewards = Total balance - Total principal amounts - Total pending rewards
    let total_committed = total_principal + total_pending_rewards;
    let available_rewards_amount = total_balance.amount.checked_sub(total_committed)
        .unwrap_or(Uint128::zero()); // handle edge case where committed might exceed balance

    Ok(Coin::new(available_rewards_amount, &config.denom))
}

fn query_rewards_per_year(deps: Deps, config: &Config) -> StdResult<Coin> {
    let total_yearly_rewards = fold_lockups(
        deps,
        Uint128::zero(),
        |acc, lockup| {
            let lockup_yearly_rewards = calculate_yearly_rewards_for_lockup(lockup, &config.tier_config)?;
            Ok(acc + lockup_yearly_rewards)
        },
    )?;

    Ok(Coin::new(total_yearly_rewards, &config.denom))
}

fn calculate_yearly_rewards_for_lockup(lockup: &Lockup, tier_config: &TierConfig) -> StdResult<Uint128> {
    let start_time = lockup.start_time;
    let one_year_later = cosmwasm_std::Timestamp::from_seconds(
        start_time.seconds() + SECONDS_PER_YEAR
    );

    let simulated_lockup = Lockup {
        owner: lockup.owner.clone(),
        principal_amount: lockup.principal_amount.clone(),
        annual_percentage_rate: lockup.annual_percentage_rate,
        start_time: lockup.start_time,
        unlock_time: lockup.unlock_time,
        last_claim_time: start_time,
    };

    let yearly_rewards = calculate_rewards(
        &simulated_lockup,
        one_year_later,
        &lockup.principal_amount.denom,
        tier_config,
    )?;

    Ok(yearly_rewards.amount)
}

fn check_coin_availability(deps: Deps, env: Env ) -> StdResult<bool> {
    let config = CONFIG.load(deps.storage)?;
    let coin = query_available_rewards(deps, env, &config)?;
    let required_amount = query_rewards_per_year(deps, &config)?.amount;

    Ok(coin.amount >= required_amount)
}

// ==============================================





// =========== REWARD CALCULATION HELPERS ===========

fn seconds_to_decimal(seconds: u64) -> Decimal {
    Decimal::from_atomics(seconds, 0).unwrap()
}

fn decimal_to_uint128(decimal: Decimal) -> Uint128 {
    decimal.atomics() / Uint128::new(10u128.pow(DECIMAL_PRECISION))
}

fn get_year_index(time_elapsed: u64) -> u64 {
    time_elapsed / SECONDS_PER_YEAR
}

fn get_year_end_seconds(start_seconds: u64, year_index: u64) -> u64 {
    start_seconds + ((year_index + 1) * SECONDS_PER_YEAR)
}

// ==================================





// =========== REWARD CALCULATION LOGIC ===========

// calculate rewards for a specific time period with a given APR
fn calculate_period_rewards(
    principal_decimal: Decimal,
    apr: Decimal,
    duration_seconds: u64,
) -> Decimal {
    let seconds_per_year = seconds_to_decimal(SECONDS_PER_YEAR);
    let period_duration = seconds_to_decimal(duration_seconds);
    let year_fraction = period_duration / seconds_per_year;

    principal_decimal * apr * year_fraction
}

// calculate effective APR for a given year, applying progressive increases
fn calculate_apr_for_year(
    base_apr: Decimal,
    tier_config: &TierConfig,
    year_index: u64,
) -> Decimal {
    if year_index == 0 {
        // first year: use base APR
        base_apr
    } else {
        // after first year: apply progressive increases
        let years_past_first = seconds_to_decimal(year_index);
        let additional_percentage = years_past_first * tier_config.percentage_increase_per_year;

        // cap at the additional percentage
        let capped_additional = additional_percentage.min(tier_config.max_percentage_increase);

        base_apr + capped_additional
    }
}

// calculate rewards for same year period
fn calculate_simple_rewards(
    lockup: &Lockup,
    duration_seconds: u64,
) -> Uint128 {
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();
    let rewards = calculate_period_rewards(
        principal_decimal,
        lockup.annual_percentage_rate,
        duration_seconds,
    );
    decimal_to_uint128(rewards)
}

// calculate rewards across multiple years with progressive APR increases
fn calculate_progressive_rewards(
    lockup: &Lockup,
    last_claim_seconds: u64,
    current_seconds: u64,
    tier_config: &TierConfig,
) -> Uint128 {
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();
    let start_seconds = lockup.start_time.seconds();
    let mut total_rewards = Decimal::zero();
    let mut calculation_start = last_claim_seconds;

    while calculation_start < current_seconds {
        let time_since_start = calculation_start - start_seconds;
        let year_index = get_year_index(time_since_start);

        let effective_apr = calculate_apr_for_year(
            lockup.annual_percentage_rate,
            tier_config,
            year_index,
        );

        // find the end of current year or calculation period
        let year_end_seconds = get_year_end_seconds(start_seconds, year_index);
        let period_end = year_end_seconds.min(current_seconds);

        // calculate rewards for this period
        let period_duration = period_end - calculation_start;
        let period_rewards = calculate_period_rewards(
            principal_decimal,
            effective_apr,
            period_duration,
        );

        total_rewards += period_rewards;
        calculation_start = period_end;
    }

    decimal_to_uint128(total_rewards)
}


fn calculate_rewards(
    lockup: &Lockup,
    current_time: cosmwasm_std::Timestamp,
    denom: &str,
    tier_config: &TierConfig,
) -> StdResult<Coin> {
    if lockup.annual_percentage_rate.is_zero() {
        return Ok(Coin::new(0u128, denom));
    }
    if current_time <= lockup.last_claim_time {
        return Ok(Coin::new(0u128, denom));
    }

    let start_seconds = lockup.start_time.seconds();
    let last_claim_seconds = lockup.last_claim_time.seconds();
    let current_seconds = current_time.seconds();

    // calculate which year we are currently in and which year we last claimed
    let time_since_start = current_seconds - start_seconds;
    let last_claim_time_since_start = last_claim_seconds - start_seconds;

    let current_year = get_year_index(time_since_start);
    let last_claim_year = get_year_index(last_claim_time_since_start);

    let reward_amount = if current_year == last_claim_year {
        // standard APR
        calculate_simple_rewards(lockup, current_seconds - last_claim_seconds)
    } else {
        // progressive APR
        calculate_progressive_rewards(lockup, last_claim_seconds, current_seconds, tier_config)
    };

    Ok(Coin::new(reward_amount, denom))
}

fn check_sufficient_reward_funds(
    deps: Deps,
    env: Env,
    config: &Config,
    new_principal_amount: Uint128,
) -> Result<(), ContractError> {
    // get current available rewards and yearly requirements
    let available_rewards = query_available_rewards(deps, env, config)?;
    let current_required_yearly = query_rewards_per_year(deps, config)?;

    // calculate additional yearly rewards for the new lockup at maximum APR
    let new_apr = get_apr_for_amount(new_principal_amount, &config.tier_config);
    let max_apr = new_apr + config.tier_config.max_percentage_increase;

    let additional_yearly_rewards = calculate_period_rewards(
        Decimal::from_atomics(new_principal_amount, 0)
            .map_err(|_| ContractError::InvalidPrincipalAmount {  })?,
        max_apr,
        SECONDS_PER_YEAR,
    );

    let additional_yearly_amount = decimal_to_uint128(additional_yearly_rewards);

    // convert to monthly requirements
    let months_per_year = 12u128;
    let current_required_monthly = current_required_yearly.amount / Uint128::new(months_per_year);
    let additional_required_monthly = additional_yearly_amount / Uint128::new(months_per_year);
    let total_monthly_required = current_required_monthly + additional_required_monthly;

    // calculate available funds after accounting for new principal
    let available_after_principal = available_rewards.amount
        .checked_sub(new_principal_amount)
        .unwrap_or(Uint128::zero());

    // check if we have sufficient funds
    ensure!(
        available_after_principal >= total_monthly_required,
        ContractError::InsufficientRewardFunds {
            available_amount: available_after_principal.u128(),
            available_denom: available_rewards.denom.clone(),
            required_amount: total_monthly_required.u128(),
            required_denom: config.denom.clone(),
        }
    );

    Ok(())
}

// ========================





// ========= LOCKUP UTILITIES ==========

// validate the funds sent with the lock transaction
fn validate_lock_funds(funds: &[Coin], expected_denom: &str) -> Result<(), ContractError> {
    ensure!(
        funds.len() == 1 && funds[0].denom == expected_denom,
        ContractError::InvalidFunds { expected_denom: expected_denom.to_string() }
    );

    ensure!(
        !funds[0].amount.is_zero(),
        ContractError::ZeroAmount {}
    );

    Ok(())
}

// validate deposit amount is within tier limits
fn validate_deposit_amount(amount: Uint128, tier_config: &TierConfig) -> Result<(), ContractError> {
    ensure!(
        amount >= tier_config.tier_1_min,
        ContractError::DepositBelowMinimum {}
    );

    ensure!(
        amount <= tier_config.tier_4_limit,
        ContractError::DepositExceedsLimit {}
    );

    Ok(())
}

// create parameters for a new lockup
fn create_new_lockup_params(env: &Env, amount: Uint128, config: &Config) -> LockupParams {
    let apr = get_apr_for_amount(amount, &config.tier_config);
    let current_time = env.block.time;

    LockupParams {
        unlock_time: current_time.plus_seconds(config.lockup_duration_seconds),
        last_claim_time: current_time,
        start_time: current_time,
        apr,
    }
}

// handle lockup upgrade when user already has an existing lockup
fn handle_lockup_upgrade(
    storage: &mut dyn cosmwasm_std::Storage,
    env: &Env,
    sender: &cosmwasm_std::Addr,
    principal_amount: &mut Coin,
    config: &Config,
) -> Result<(LockupParams, Response), ContractError> {
    let mut existing_lockup = LOCKUPS.load(storage, sender.clone())?;

    // validate lockup period hasnt ended
    ensure!(
        env.block.time.seconds() <= existing_lockup.unlock_time.seconds(),
        ContractError::PastLockupPeriod {}
    );

    let existing_tier = get_tier_for_amount(existing_lockup.principal_amount.amount, &config.tier_config);
    let combined_amount = principal_amount.amount + existing_lockup.principal_amount.amount;

    // validate combined deposit doesnt exceed limit
    ensure!(
        combined_amount <= config.tier_config.tier_4_limit,
        ContractError::DepositExceedsLimit {}
    );

    // Auto-claim any existing rewards using the dedicated function
    let (response, _claimed_rewards) = auto_claim_rewards(
        &mut existing_lockup,
        env,
        sender,
        config,
    )?;

    // check if amount allows for tier upgrade
    let apr = determine_upgrade_apr(
        principal_amount.amount,
        existing_lockup.principal_amount.amount,
        existing_tier,
        &config.tier_config,
    );

    // update principal amount to combined amount
    principal_amount.amount = combined_amount;

    let params = LockupParams {
        unlock_time: existing_lockup.unlock_time,
        last_claim_time: existing_lockup.last_claim_time,
        start_time: existing_lockup.start_time,
        apr,
    };

    Ok((params, response))
}

// determine APR based on tier upgrade
fn determine_upgrade_apr(
    new_amount: Uint128,
    existing_amount: Uint128,
    existing_tier: u8,
    tier_config: &TierConfig,
) -> Decimal {
    let combined_amount = new_amount + existing_amount;
    let new_tier = get_tier_for_amount(combined_amount, tier_config);

    if new_tier > existing_tier {
        // update APR
        get_apr_for_amount(combined_amount, tier_config)
    } else {
        // dont update APR
        get_apr_for_amount(existing_amount, tier_config)
    }
}

// autoclaim any existing rewards
// temporary feature to prevent from false reward calculations
fn auto_claim_rewards(
    lockup: &mut Lockup,
    env: &Env,
    sender: &cosmwasm_std::Addr,
    config: &Config,
) -> Result<(Response, Coin), ContractError> {
    let claimable_rewards = calculate_rewards(
        lockup,
        env.block.time,
        &config.denom,
        &config.tier_config
    )?;

    if claimable_rewards.amount.is_zero() {
        return Ok((Response::new(), claimable_rewards));
    }

    lockup.last_claim_time = env.block.time;

    let bank_msg = BankMsg::Send {
        to_address: sender.to_string(),
        amount: vec![claimable_rewards.clone()],
    };

    let response = Response::new()
        .add_message(bank_msg)
        .add_attribute("rewards_auto_claimed", claimable_rewards.amount);

    Ok((response, claimable_rewards))
}

// ========================
