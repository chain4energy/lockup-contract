use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin,
    Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, Lockup, CONFIG, LOCKUPS};

const CONTRACT_NAME: &str = "crates.io:lockup-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// using 6 decimal places for C4E token (uc4e)
const C4E_1: u128 = 1_000_000;

// Tier thresholds in uc4e
const TIER_1_MIN: Uint128 = Uint128::new(10_000 * C4E_1);
const TIER_2_MIN: Uint128 = Uint128::new(50_000 * C4E_1);
const TIER_3_MIN: Uint128 = Uint128::new(100_000 * C4E_1);
const TIER_4_MIN: Uint128 = Uint128::new(500_000 * C4E_1);
//

// define lockup limit:
const TIER_4_LIMIT: Uint128 = Uint128::new(1_000_000 * C4E_1); // 1 million C4E
//

// Annual Percentage Rates (APR) for each tier
const TIER_1_APR: Decimal = Decimal::percent(2); // 2%
// it doesnt work here (its in 'get_apr_for_amount'):
//const TIER_2_APR: Decimal = Decimal::from_atomics(35u32, 3).unwrap(); // 3.5%
const TIER_3_APR: Decimal = Decimal::percent(5); // 5%
const TIER_4_APR: Decimal = Decimal::percent(8); // 8%
//

// this one in 'calculate_rewards'
//const SECONDS_PER_YEAR: Decimal = Decimal::from_atomics(31_536_000_u128, 0).unwrap(); // 365 * 24 * 60 * 60

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
    let admin = msg
        .admin
        .map_or(Ok(info.sender.clone()), |addr| deps.api.addr_validate(&addr))?;

    let config = Config {
        admin,
        denom: msg.denom,
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
        ExecuteMsg::Lock { duration }  => execute_lock(deps, env, info, duration),
        ExecuteMsg::UnlockPrincipal {} => execute_unlock_principal(deps, env, info),
        ExecuteMsg::ClaimRewards {}    => execute_claim_rewards(deps, env, info),
        ExecuteMsg::DepositRewards {}  => execute_deposit_rewards(deps, info),
    }
}

pub fn execute_lock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // expect only one type of coin to be sent
    if info.funds.len() != 1 || info.funds[0].denom != config.denom {
        return Err(ContractError::InvalidFunds {
            expected_denom: config.denom,
        });
    }

    let principal_amount = info.funds[0].clone();
    if principal_amount.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // check if the user already has a lockup (will allow to extend the lockup later)
    if LOCKUPS.has(deps.storage, info.sender.clone()) {
        return Err(ContractError::AlreadyLocked {});
    }

    // check if the deposit is above the limit (1 million c4e)
    if principal_amount.amount > TIER_4_LIMIT {
        return Err(ContractError::DepositExceedsLimit {});
    }
    // check if the deposit is below the limit (10K C4E)
    if principal_amount.amount < TIER_1_MIN {
        return Err(ContractError::DepositBelowMinimum {});
    }

    let apr = get_apr_for_amount(principal_amount.amount);

    let lockup = Lockup {
        owner: info.sender.clone(),
        principal_amount: principal_amount.clone(),
        unlock_time: env.block.time.plus_seconds(duration),
        annual_percentage_rate: apr,
        last_claim_time: env.block.time,
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
    info: MessageInfo
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
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
pub fn query(
    deps: Deps,
    env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
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
fn get_apr_for_amount(amount: Uint128) -> Decimal {
    if amount >= TIER_4_MIN {
        TIER_4_APR
    } else if amount >= TIER_3_MIN {
        TIER_3_APR
    } else if amount >= TIER_2_MIN {
        Decimal::from_atomics(35u32, 3).unwrap() // 3.5%
    } else if amount >= TIER_1_MIN {
        TIER_1_APR
    } else {
        Decimal::zero()
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
    let time_elapsed_seconds =
        Decimal::from_atomics(effective_end_time.seconds() - lockup.last_claim_time.seconds(), 0)
            .unwrap();

    let year_fraction = time_elapsed_seconds / seconds_per_year;

    // convert principal amount to Decimal for multiplication
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();
    let reward_decimal = principal_decimal * lockup.annual_percentage_rate * year_fraction;

    // convert back to Uint128 - truncate decimal places
    let reward_amount = reward_decimal.atomics() / Uint128::new(10u128.pow(18));

    Ok(Coin::new(reward_amount, denom))
}
