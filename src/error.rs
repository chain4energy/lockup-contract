use cosmwasm_std::{Uint128, StdError, Timestamp};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Amount cannot be zero")]
    ZeroAmount {},

    #[error("No rewards to claim at this time")]
    ZeroRewards {},

    #[error("Deposit exceeds maximum limit")]
    DepositExceedsLimit {},

    #[error("Deposit below minimum required amount")]
    DepositBelowMinimum {},

    #[error("Past lockup period")]
    PastLockupPeriod {},

    #[error("No funds for deposit")]
    NoFundsForDeposit {},

    #[error("Invalid principal amount")]
    InvalidPrincipalAmount {},

    #[error("Funds are still locked. Unlock time: {unlock_time}")]
    StillLocked { unlock_time: Timestamp },

    #[error("Must send funds with the expected denomination: {expected_denom}")]
    InvalidFunds { expected_denom: String },

    #[error("Cannot claim rewards: to claim {} {}, available {} {}, please contact administrators.",
        claim_amount, claim_denom,
        available_amount, available_denom)]

    CannotClaimRewards {
        claim_amount:     Uint128,
        claim_denom:      String,
        available_amount: Uint128,
        available_denom:  String,
    },

    #[error("Insufficient reward funds: available {} {}, required {} {}",
        available_amount, available_denom,
        required_amount, required_denom)]

    InsufficientRewardFunds {
        available_amount: u128,
        available_denom:  String,
        required_amount:  u128,
        required_denom:   String,
    },
}
