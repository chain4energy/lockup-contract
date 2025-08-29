use crate::state::{Config, Lockup, TierConfig};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Timestamp, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admins: Option<Vec<String>>,
    // native token denomination:
    pub denom: String,
    // lockup duration in seconds (e.g., 31_536_000 for 1 year)
    pub lockup_duration_seconds: u64,
    // tier configuration
    pub tier_config: TierConfig,
}

#[cw_serde]
pub enum ExecuteMsg {
    // locks the funds sent with the message for the configured duration
    Lock {},
    // unlocks the original amount after the lockup period has passed
    // any pending rewards are claimed automatically
    UnlockPrincipal {},
    // claims the accured rewards without touching the original amount
    ClaimRewards {},
    // (Admin only) Deposit funds into the contract to be used for reward payouts
    DepositRewards {},
}

#[cw_serde]
pub struct LockupInfo {
    pub address: String,
    pub start_time: Timestamp,
    pub principal_amount: Uint128,
    pub annual_percentage_rate: Decimal,
    pub tier: u8,
}

#[cw_serde]
pub struct AllLockupsResponse {
    pub lockups: Vec<LockupInfo>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // returns the contracts configuration
    #[returns(Config)]
    GetConfig {},

    // returns the lockup details for a specific address
    #[returns(Lockup)]
    GetLockup { address: String },

    // calculates and returns the currently claimable rewards for an address
    #[returns(Coin)]
    GetClaimableRewards { address: String },

    // returns all lockups with their details:
    //  - address
    //  - principal_amount
    //  - APR
    //  - tier
    //  - start_time
    #[returns(AllLockupsResponse)]
    GetAllLockups {},

    // returns the total amount of available rewards currently deposited in the contract
    #[returns(Coin)]
    GetDepositedRewards {},

    // returns the sum of lockups and sum of deposited amounts
    #[returns((Uint128, Coin))]
    GetSumLockupsAndDeposits {},

    // returns the total amount of all rewards (claimed and unclaimed) for all users
    #[returns(Coin)]
    GetAllRewards {},

    // checks if the the reward pool has enough funds to cover all rewards for 1 year
    // returns a boolean
    #[returns(bool)]
    CheckCoinAvailability {},
}
