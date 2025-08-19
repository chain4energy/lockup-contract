use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;
use crate::state::{Config, Lockup};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    // native token denomination:
    pub denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    // locks the funds sent with the message for a specified duration
    Lock { duration: u64 },
    // unlocks the original amount after the lockup period has passed
    // any pending rewards are claimed automatically
    UnlockPrincipal {},
    // claims the accured rewards without touching the original amount
    ClaimRewards {},
    // (Admin only) Deposit funds into the contract to be used for reward payouts
    DepositRewards {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // returns the contracts configuration
    #[returns(Config)]
    GetConfig {},
    // GetLockup returns the lockup details for a specific address
    #[returns(Lockup)]
    GetLockup { address: String },
    // calculates and returns the currently claimable rewards for an address
    #[returns(Coin)]
    GetClaimableRewards { address: String },
}
