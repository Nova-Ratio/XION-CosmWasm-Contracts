use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint256};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub cw404_address: Addr,
    pub cw20_address: Addr,
}

#[cw_serde]
pub struct Listing {
    pub nft_id: String,
    pub price: Uint256,
    pub owner: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LISTINGS: Map<String, Listing> = Map::new("listings");
pub const LISTING_COUNTER: Item<u128> = Item::new("listing_counter");
