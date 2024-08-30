use cosmwasm_schema::cw_serde;

use cosmwasm_std::{Addr, Uint128};
use cw20::Denom;
use cw_storage_plus::Item;

use bonding_types::curves::DecimalPlaces;
use bonding_types::msg::{CurveType, MarketingInfoResponse};

/// Supply is dynamic and tracks the current supply of staked and ERC20 tokens.
#[cw_serde]
pub struct CurveState {
    /// reserve is how many native tokens exist bonded to the validator
    pub reserve: Uint128,
    /// supply is how many tokens this contract has issued
    pub supply: Uint128,

    // cw-20 token denom of the reserve
    pub reserve_denom: Denom,

    // how to normalize reserve and supply
    pub decimals: DecimalPlaces,
}

#[cw_serde]
pub struct CW20Balance {
    pub denom: Denom,
    pub amount: Uint128,
    pub sender: Addr,
}

impl CurveState {
    pub fn new(reserve_denom: Denom, decimals: DecimalPlaces) -> Self {
        CurveState {
            reserve: Uint128::zero(),
            supply: Uint128::zero(),
            reserve_denom,
            decimals,
        }
    }
}

pub const CURVE_STATE: Item<CurveState> = Item::new("curve_state");

pub const CURVE_TYPE: Item<CurveType> = Item::new("curve_type");

pub const MARKETING_INFO: Item<MarketingInfoResponse> = Item::new("marketing_info");
