use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
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
    // otherwise validate the provided address
    let admin = msg.admin.map_or(Ok(info.sender.clone()), |addr| {
        deps.api.addr_validate(&addr)
    })?;

    // configurable lockup time, tier levels and rewards
    let config = Config {
        admin,
        denom: msg.denom,
        lockup_duration_seconds: msg.lockup_duration_seconds,
        tier_config: msg.tier_config,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", config.admin.to_string())
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
    let (unlock_time, preserve_last_claim_time, apr) =
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
            (
                existing_lockup.unlock_time,
                existing_lockup.last_claim_time,
                apr,
            )
        } else {
            // continue initial lockup normally
            let apr = get_apr_for_amount(principal_amount.amount, &config.tier_config);
            // new lockup gets full duration from current time
            (
                env.block.time.plus_seconds(config.lockup_duration_seconds),
                env.block.time,
                apr,
            )
        };

    let lockup = Lockup {
        owner: info.sender.clone(),
        principal_amount: principal_amount.clone(),
        unlock_time,
        annual_percentage_rate: apr,
        last_claim_time: preserve_last_claim_time,
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

    let rewards = calculate_rewards(&lockup, env.block.time, &config.denom)?;
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
    let pending_rewards = calculate_rewards(&lockup, env.block.time, &config.denom)?;

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

    // only admin can deposit rewards
    if info.sender != config.admin {
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
            let rewards = calculate_rewards(&lockup, env.block.time, &config.denom)?;
            to_json_binary(&rewards)
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

fn calculate_rewards(
    lockup: &Lockup,
    current_time: cosmwasm_std::Timestamp,
    denom: &str,
) -> StdResult<Coin> {
    if lockup.annual_percentage_rate.is_zero() {
        return Ok(Coin::new(0u128, denom));
    }
    if current_time <= lockup.last_claim_time {
        return Ok(Coin::new(0u128, denom));
    }

    let seconds_per_year = Decimal::from_atomics(31_536_000u128, 0).unwrap();

    // determine the effective end time for reward calculation
    // rewards should only accumulate up to the unlock_time, not beyond
    let effective_end_time = if current_time > lockup.unlock_time {
        lockup.unlock_time
    } else {
        current_time
    };

    // if the effective end time is before or equal to last claim time, no rewards
    if effective_end_time <= lockup.last_claim_time {
        return Ok(Coin::new(0u128, denom));
    }

    // safe subtraction
    let time_elapsed_seconds = Decimal::from_atomics(
        effective_end_time.seconds() - lockup.last_claim_time.seconds(),
        0,
    )
    .unwrap();

    let year_fraction = time_elapsed_seconds / seconds_per_year;

    // convert principal amount to Decimal for multiplication
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();
    let reward_decimal = principal_decimal * lockup.annual_percentage_rate * year_fraction;

    // convert back to Uint128 - truncate decimal places
    let reward_amount = reward_decimal.atomics() / Uint128::new(10u128.pow(18));

    Ok(Coin::new(reward_amount, denom))
}
