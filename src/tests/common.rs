use crate::msg::InstantiateMsg;
use cosmwasm_std::{Addr, Coin, Empty};
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
            .init_balance(storage, &user_addr, vec![Coin::new(1_000_000_000_000_000u128, DENOM)])
            .unwrap();

        // give admin some funds too for deposits
        router
            .bank
            .init_balance(storage, &admin_addr, vec![Coin::new(1_000_000_000_000u128, DENOM)])
            .unwrap();
    });
    let contract_code_id = app.store_code(lockup_contract());

    let admin_addr = app.api().addr_make(ADMIN);
    let msg = InstantiateMsg {
        admin: None, // use sender as admin to avoid validation issues
        denom: DENOM.to_string(),
    };
    let contract_addr = app
        .instantiate_contract(
            contract_code_id,
            admin_addr,
            &msg,
            &[],
            "tiered-lockup",
            None,
        )
        .unwrap();

    (app, contract_addr)
}
