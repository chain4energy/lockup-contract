use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, AllLockupsResponse, LockupInfo};
use crate::state::{Config, Lockup, TierConfig, CONFIG, LOCKUPS};

const CONTRACT_NAME:    &str = "crates.io:lockup-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

    // expect only one type of coin to be sent
    if info.funds.len() != 1 || info.funds[0].denom != config.denom {
        return Err(ContractError::InvalidFunds {
            expected_denom: config.denom,
        });
    }

    // validate principal amount
    // check if the amount is not zero just in case
    let mut principal_amount = info.funds[0].clone();
    if principal_amount.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // check if we have sufficient funds for rewards
    let available_rewards = query_available_rewards(deps.as_ref(), env.clone(), &config)?;
    let current_required = query_rewards_per_year(deps.as_ref(), &config)?;

    // calculate additional monthly rewards required for this new lockup
    let new_apr = get_apr_for_amount(principal_amount.amount, &config.tier_config);
    let max_apr = new_apr + config.tier_config.max_percentage_increase;
    let principal_decimal = Decimal::from_atomics(principal_amount.amount, 0).unwrap();
    let additional_yearly_rewards = principal_decimal * max_apr;
    let additional_monthly_rewards = additional_yearly_rewards / Decimal::from_atomics(12u128, 0).unwrap();
    let additional_required = additional_monthly_rewards.atomics() / Uint128::new(10u128.pow(18));

    let total_required = current_required.amount + additional_required;

    // adjust available rewards by excluding the principal amount
    let adjusted_available = available_rewards.amount.checked_sub(principal_amount.amount)
        .unwrap_or(Uint128::zero());

    if adjusted_available < total_required {
        return Err(ContractError::Std(StdError::generic_err(
            format!("Insufficient reward funds: available {} {}, required {} {}",
                    adjusted_available, available_rewards.denom,
                    total_required, config.denom)
        )));
    }


    // check if the lockup time has passed
    if env.block.time.seconds() < config.lockup_duration_seconds {
        return Err(ContractError::PastLockupPeriod {});
    }

    // check if the deposit is below the limit
    if principal_amount.amount < config.tier_config.tier_1_min {
        return Err(ContractError::DepositBelowMinimum {});
    }

    // check if the deposit is above the limit
    if principal_amount.amount > config.tier_config.tier_4_limit {
        return Err(ContractError::DepositExceedsLimit {});
    }

    // check if the user already has a lockup (for the tier upgrade system)
    let (unlock_time, preserve_last_claim_time, preserve_start_time, apr) =
        if LOCKUPS.has(deps.storage, info.sender.clone()) {

            // load existing lockup, and check current tier
            let existing_lockup = LOCKUPS.load(deps.storage, info.sender.clone())?;
            let existing_tier =
                get_tier_for_amount(existing_lockup.principal_amount.amount, &config.tier_config);

            // check if the new deposit is above the limit
            if principal_amount.amount + existing_lockup.principal_amount.amount
                > config.tier_config.tier_4_limit {
                return Err(ContractError::DepositExceedsLimit {});
            }

            // check if the new deposit allows for new tier
            let apr = if get_tier_for_amount(
                existing_lockup.principal_amount.amount + principal_amount.amount,
                &config.tier_config,
            ) > existing_tier {
                // set new apr
                get_apr_for_amount(
                    existing_lockup.principal_amount.amount + principal_amount.amount,
                    &config.tier_config,
                )
            } else {
                // dont update apr
                get_apr_for_amount(principal_amount.amount, &config.tier_config)
            };

            // update the lockup balance
            principal_amount.amount += existing_lockup.principal_amount.amount;

            // calculate remaining time: keep the original unlock time, dont reset to full duration
            // also preserve the original start time
            (
                existing_lockup.unlock_time,
                existing_lockup.last_claim_time,
                existing_lockup.start_time,
                apr,
            )
        } else {
            // continue initial lockup normally
            let apr = get_apr_for_amount(principal_amount.amount, &config.tier_config);
            // new lockup gets full duration from current time
            (
                env.block.time.plus_seconds(config.lockup_duration_seconds),
                env.block.time,
                env.block.time, // start time is current time for new lockups
                apr,
            )
        };

    let lockup = Lockup {
        owner: info.sender.clone(),
        principal_amount: principal_amount.clone(),
        unlock_time,
        annual_percentage_rate: apr,
        last_claim_time: preserve_last_claim_time,
        start_time: preserve_start_time,
    };
    LOCKUPS.save(deps.storage, info.sender.clone(), &lockup)?;

    Ok(Response::new()
        .add_attribute("method", "lock")
        .add_attribute("owner", info.sender)
        .add_attribute("amount", principal_amount.amount)
        .add_attribute("apr", apr.to_string()))
}

pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut lockup = LOCKUPS.load(deps.storage, info.sender.clone())?;
    let config = CONFIG.load(deps.storage)?;

    let rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;
    if rewards.amount.is_zero() {
        return Err(ContractError::ZeroRewards {});
    }

    // undate the last claim time before sending funds
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
    if env.block.time < lockup.unlock_time {
        return Err(ContractError::StillLocked {
            unlock_time: lockup.unlock_time,
        });
    }

    // calculate final pending rewards
    let pending_rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;

    // final action, remove lockup from storage
    LOCKUPS.remove(deps.storage, info.sender.clone());

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

pub fn execute_deposit_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // only admins can deposit rewards
    if !config.admins.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }
    if info.funds.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(
            "No funds sent for deposit",
        )));
    }

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
    // count lockups and sum their principal amounts
    let mut lockup_count = 0u128;
    let mut total_principal = Uint128::zero();

    for item in LOCKUPS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, lockup) = item?;
        lockup_count += 1;
        total_principal += lockup.principal_amount
        .amount;
    }

    let principal_coin = Coin::new(total_principal, &config.denom);

    Ok((Uint128::new(lockup_count), principal_coin))
}

fn query_available_rewards(deps: Deps, env: Env, config: &Config) -> StdResult<Coin> {
    // get total contract balance
    let total_balance = deps.querier.query_balance(env.contract.address, &config.denom)?;

    // calculate total principal amounts and pending rewards for all lockups
    let mut total_principal = Uint128::zero();
    let mut total_pending_rewards = Uint128::zero();

    for item in LOCKUPS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, lockup) = item?;
        total_principal += lockup.principal_amount.amount;

        // calculate pending rewards for this lockup
        let pending_rewards = calculate_rewards(&lockup, env.block.time, &config.denom, &config.tier_config)?;
        total_pending_rewards += pending_rewards.amount;
    }

    // Available rewards = Total balance - Total principal amounts - Total pending rewards
    let total_committed = total_principal + total_pending_rewards;
    let available_rewards_amount = total_balance.amount.checked_sub(total_committed)
        .unwrap_or(Uint128::zero()); // handle edge case where committed might exceed balance

    Ok(Coin::new(available_rewards_amount, &config.denom))
}

fn query_rewards_per_year(deps: Deps, config: &Config) -> StdResult<Coin> {
    let mut total_yearly_rewards = Uint128::zero();

    // iterate through all lockups
    // return true if sufficient funds are available, false otherwise
    for item in LOCKUPS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, lockup) = item?;

        // calculate yearly rewards for this specific lockup
        // considering the progressive APR increases
        let lockup_yearly_rewards = calculate_yearly_rewards_for_lockup(&lockup, &config.tier_config)?;

        total_yearly_rewards += lockup_yearly_rewards;
    }

    Ok(Coin::new(total_yearly_rewards, &config.denom))
}

fn calculate_yearly_rewards_for_lockup(lockup: &Lockup, tier_config: &TierConfig) -> StdResult<Uint128> {
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();

    // calculate the maximum possible APR after all increases
    // ensuring we have sufficient reserves
    let max_additional_percentage = tier_config.max_percentage_increase;
    let max_apr = lockup.annual_percentage_rate + max_additional_percentage;
    let max_yearly_rewards = principal_decimal * max_apr;

    // return the maximum yearly rewards to ensure sufficient reserves
    let reward_amount = max_yearly_rewards.atomics() / Uint128::new(10u128.pow(18));

    Ok(reward_amount)
}

fn check_coin_availability(deps: Deps, env: Env ) -> StdResult<bool> {
    let config = CONFIG.load(deps.storage)?;
    let coin = query_available_rewards(deps, env, &config)?;
    let required_amount = query_rewards_per_year(deps, &config)?.amount;

    Ok(coin.amount >= required_amount)
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

    let seconds_per_year = Decimal::from_atomics(31_536_000u128, 0).unwrap();
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();

    let start_seconds = lockup.start_time.seconds();
    let last_claim_seconds = lockup.last_claim_time.seconds();
    let current_seconds = current_time.seconds();

    // calculate which year we are currently in and which year we last claimed
    let time_since_start = current_seconds - start_seconds;
    let last_claim_time_since_start = last_claim_seconds - start_seconds;

    let current_year = time_since_start / 31_536_000; // 0-indexed years
    let last_claim_year = last_claim_time_since_start / 31_536_000; // 0-indexed years

    // if same year, use simple calculation
    if current_year == last_claim_year {
        let years_since_start_decimal = Decimal::from_atomics(time_since_start, 0).unwrap() / seconds_per_year;

        let effective_apr = if years_since_start_decimal < Decimal::one() {
            lockup.annual_percentage_rate
        } else {
            let years_past_first = years_since_start_decimal.floor();
            let additional_percentage = years_past_first * tier_config.percentage_increase_per_year;
            let capped_additional = if additional_percentage >= tier_config.max_percentage_increase {
                tier_config.max_percentage_increase
            } else {
                additional_percentage
            };
            lockup.annual_percentage_rate + capped_additional
        };

        let time_elapsed_seconds = Decimal::from_atomics(current_seconds - last_claim_seconds, 0).unwrap();
        let year_fraction = time_elapsed_seconds / seconds_per_year;
        let reward_decimal = principal_decimal * effective_apr * year_fraction;
        let reward_amount = reward_decimal.atomics() / Uint128::new(10u128.pow(18));

        return Ok(Coin::new(reward_amount, denom));
    }

    // calculate year by year
    let mut total_rewards = Decimal::zero();
    let mut calculation_start = last_claim_seconds;

    while calculation_start < current_seconds {
        let time_since_start_calc = calculation_start - start_seconds;
        let year_index = time_since_start_calc / 31_536_000; // 0-indexed year

        // calculate effective APR for this year
        let effective_apr = if year_index == 0 {
            // first year (year 0): use base APR
            lockup.annual_percentage_rate
        } else {
            // after first year: apply progressive increases
            let years_past_first = Decimal::from_atomics(year_index, 0).unwrap();
            let additional_percentage = years_past_first * tier_config.percentage_increase_per_year;

            let capped_additional = if additional_percentage >= tier_config.max_percentage_increase {
                tier_config.max_percentage_increase
            } else {
                additional_percentage
            };

            lockup.annual_percentage_rate + capped_additional
        };

        // find the end of current year or the end of calculation period
        let year_end_seconds = start_seconds + ((year_index + 1) * 31_536_000);
        let period_end = if year_end_seconds > current_seconds {
            current_seconds
        } else {
            year_end_seconds
        };

        // calculate rewards for this period
        let period_duration = period_end - calculation_start;
        let period_fraction = Decimal::from_atomics(period_duration, 0).unwrap() / seconds_per_year;
        let period_rewards = principal_decimal * effective_apr * period_fraction;

        total_rewards += period_rewards;

        // move to next period
        calculation_start = period_end;
    }

    // convert back to Uint128
    let reward_amount = total_rewards.atomics() / Uint128::new(10u128.pow(18));

    Ok(Coin::new(reward_amount, denom))
}
