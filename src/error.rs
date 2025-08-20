use cosmwasm_std::{StdError, Timestamp};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Funds are still locked. Unlock time: {unlock_time}")]
    StillLocked { unlock_time: Timestamp },

    #[error("Must send funds with the expected denomination: {expected_denom}")]
    InvalidFunds { expected_denom: String },

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
}
