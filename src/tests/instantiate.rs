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
    assert_eq!(config.admin, expected_admin);
    assert_eq!(config.denom, DENOM.to_string());
}
