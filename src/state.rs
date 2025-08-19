use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, Timestamp};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub denom: String,
}

#[cw_serde]
pub struct Lockup {
    pub owner: Addr,
    // original amount of tokens locked
    pub principal_amount: Coin,
    // time when the original amount can be withdrawn
    pub unlock_time: Timestamp,
    // anual percentage rate for this lockup tier
    pub annual_percentage_rate: Decimal,
    // late time rewards were claimed or the lockup was created
    pub last_claim_time: Timestamp,
}

pub const CONFIG: Item<Config> = Item::new("config");
// the key for the map will be the users address
pub const LOCKUPS: Map<Addr, Lockup> = Map::new("lockups");
