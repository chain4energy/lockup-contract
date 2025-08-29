use crate::msg::InstantiateMsg;
use crate::state::TierConfig;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cosmwasm_std::{Coin, Empty};
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

pub const ADMIN: &str = "admin";
pub const USER_1: &str = "user1";
pub const DENOM: &str = "uc4e";

pub fn lockup_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn proper_instantiate() -> (App, Addr) {
    let mut app = AppBuilder::new().build(|router, api, storage| {
        let user_addr = api.addr_make(USER_1);
        let admin_addr = api.addr_make(ADMIN);

        router
            .bank
            .init_balance(
                storage,
                &user_addr,
                vec![Coin::new(1_000_000_000_000_000u128, DENOM)],
            )
            .unwrap();

        // give admin some funds too for deposits
        router
            .bank
            .init_balance(
                storage,
                &admin_addr,
                vec![Coin::new(1_000_000_000_000u128, DENOM)],
            )
            .unwrap();
    });
    let contract_code_id = app.store_code(lockup_contract());

    let admin_addr = app.api().addr_make(ADMIN);

    // create tier config
    let tier_config = TierConfig {
        tier_1_min: Uint128::new(10_000 * 1_000_000),                                   // 10K C4E
        tier_2_min: Uint128::new(50_000 * 1_000_000),                                   // 50K C4E
        tier_3_min: Uint128::new(100_000 * 1_000_000),                                  // 100K C4E
        tier_4_min: Uint128::new(500_000 * 1_000_000),                                  // 500K C4E
        tier_4_limit: Uint128::new(1_000_000 * 1_000_000),                              // 1M C4E
        tier_1_apr: Decimal::percent(2),                                                // 2%
        tier_2_apr: Decimal::from_atomics(35u32, 3).unwrap(),   // 3.5%
        tier_3_apr: Decimal::percent(5),                                                // 5%
        tier_4_apr: Decimal::percent(8),                                                // 8%
        percentage_increase_per_year: Decimal::percent(1),                              // 1% increase per year past lockup
        max_percentage_increase: Decimal::percent(10),                                   // 10% max increase
    };

    let instantiate_msg = InstantiateMsg {
        admins: None,                            // use sender as admin to avoid validation issues
        denom: DENOM.to_string(),
        lockup_duration_seconds: 31_536_000,    // 1 year
        tier_config,
    };
    let contract_addr = app
        .instantiate_contract(
            contract_code_id,
            admin_addr,
            &instantiate_msg,
            &[],
            "tiered-lockup",
            None,
        )
        .unwrap();

    (app, contract_addr)
}
