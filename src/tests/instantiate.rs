use super::common::*;
use crate::msg::QueryMsg;
use crate::state::Config;

#[test]
fn test_proper_initialization() {
    let (app, contract_addr) = proper_instantiate();

    let config: Config = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetConfig {})
        .unwrap();

    let expected_admin = app.api().addr_make(ADMIN);
    assert_eq!(config.admins, vec![expected_admin]);
    assert_eq!(config.denom, DENOM.to_string());
}

#[test]
fn test_multiple_admins_initialization() {
    use crate::msg::InstantiateMsg;
    use crate::state::TierConfig;
    use cosmwasm_std::{Decimal, Uint128};
    use cw_multi_test::{AppBuilder, Executor};

    let mut app = AppBuilder::new().build(|router, api, storage| {
        let admin_addr = api.addr_make(ADMIN);
        router
            .bank
            .init_balance(storage, &admin_addr, vec![])
            .unwrap();
    });

    let contract_code_id = app.store_code(lockup_contract());
    let admin_addr = app.api().addr_make(ADMIN);

    // define multiple admin addresses using app.api().addr_make to ensure valid addresses
    let admin1_addr = app.api().addr_make("admin1");
    let admin2_addr = app.api().addr_make("admin2");
    let admin3_addr = app.api().addr_make("admin3");

    let tier_config = TierConfig {
        tier_1_min: Uint128::new(10_000 * 1_000_000),
        tier_2_min: Uint128::new(50_000 * 1_000_000),
        tier_3_min: Uint128::new(100_000 * 1_000_000),
        tier_4_min: Uint128::new(500_000 * 1_000_000),
        tier_4_limit: Uint128::new(1_000_000 * 1_000_000),
        tier_1_apr: Decimal::percent(5),
        tier_2_apr: Decimal::percent(7),
        tier_3_apr: Decimal::percent(10),
        tier_4_apr: Decimal::percent(15),
        percentage_increase_per_year: Decimal::percent(1),
        max_percentage_increase: Decimal::percent(10),
    };

    let instantiate_msg = InstantiateMsg {
        admins: Some(vec![
            admin1_addr.to_string(),
            admin2_addr.to_string(),
            admin3_addr.to_string()
        ]),
        denom: DENOM.to_string(),
        lockup_duration_seconds: 31_536_000,
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

    let config: Config = app
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::GetConfig {})
        .unwrap();

    // verify all three admins are present
    assert_eq!(config.admins.len(), 3);
    assert!(config.admins.contains(&admin1_addr));
    assert!(config.admins.contains(&admin2_addr));
    assert!(config.admins.contains(&admin3_addr));
    assert_eq!(config.denom, DENOM.to_string());
}
