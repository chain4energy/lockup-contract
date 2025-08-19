use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_json_binary, Addr, Coin, CosmosMsg, CustomQuery, Querier, QuerierWrapper, StdResult, WasmMsg,
    WasmQuery,
};

use crate::msg::{ExecuteMsg, QueryMsg};
use crate::state::{Config, Lockup};

// LockupContract is a wrapper around Addr that provides helpers
// for working with the lockup contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LockupContract(pub Addr);

impl LockupContract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_json_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

    // Get Config
    pub fn config<Q, T, CQ>(&self, querier: &Q) -> StdResult<Config>
    where
        Q: Querier,
        T: Into<String>,
        CQ: CustomQuery,
    {
        let msg = QueryMsg::GetConfig {};
        let query = WasmQuery::Smart {
            contract_addr: self.addr().into(),
            msg: to_json_binary(&msg)?,
        }
        .into();
        let res: Config = QuerierWrapper::<CQ>::new(querier).query(&query)?;
        Ok(res)
    }

    // Get Lockup
    pub fn lockup<Q, T, CQ>(&self, querier: &Q, address: String) -> StdResult<Lockup>
    where
        Q: Querier,
        T: Into<String>,
        CQ: CustomQuery,
    {
        let msg = QueryMsg::GetLockup { address };
        let query = WasmQuery::Smart {
            contract_addr: self.addr().into(),
            msg: to_json_binary(&msg)?,
        }
        .into();
        let res: Lockup = QuerierWrapper::<CQ>::new(querier).query(&query)?;
        Ok(res)
    }

    // Get Claimable Rewards
    pub fn claimable_rewards<Q, T, CQ>(&self, querier: &Q, address: String) -> StdResult<Coin>
    where
        Q: Querier,
        T: Into<String>,
        CQ: CustomQuery,
    {
        let msg = QueryMsg::GetClaimableRewards { address };
        let query = WasmQuery::Smart {
            contract_addr: self.addr().into(),
            msg: to_json_binary(&msg)?,
        }
        .into();
        let res: Coin = QuerierWrapper::<CQ>::new(querier).query(&query)?;
        Ok(res)
    }
}
