use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

// expose to all others using contract, so others dont need to import cw721
pub use cw721::state::*;

use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub cw721_address: Option<Addr>,
    pub max_tokens: u32,
    pub unit_price: Uint128,
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub extension: DefaultOptionMetadataExtension,
    pub unused_token_id: u32,
}

pub const CONFIG: Item<Config> = Item::new("config");
