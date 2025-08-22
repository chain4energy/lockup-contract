use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct TierConfig {
    // === minimum amounts for each tier ===
    pub tier_1_min: Uint128,
    pub tier_2_min: Uint128,
    pub tier_3_min: Uint128,
    pub tier_4_min: Uint128,
    //

    // === maximum amount allowed to deposit ===
    pub tier_4_limit: Uint128,
    //

    // === annual percentage rates for each tier ===
    pub tier_1_apr: Decimal,
    pub tier_2_apr: Decimal,
    pub tier_3_apr: Decimal,
    pub tier_4_apr: Decimal,
    //

    // === percentage increases and limits past lockup period ===
    pub percentage_increase_per_year: Decimal,
    pub max_percentage_increase: Decimal,
    //
}

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub denom: String,
    pub lockup_duration_seconds: u64,
    pub tier_config: TierConfig,
}

#[cw_serde]
pub struct Lockup {
    pub owner: Addr,
    pub principal_amount: Coin,             // original amount of tokens locked
    pub unlock_time: Timestamp,             // time when the original amount can be withdrawn
    pub annual_percentage_rate: Decimal,    // annual percentage rate for this lockup tier
    pub last_claim_time: Timestamp,         // last time rewards were claimed or the lockup was created
    pub start_time: Timestamp,              // exact time when the lockup was first created
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const LOCKUPS: Map<Addr, Lockup> = Map::new("lockups"); // the key for the map will be the users address
