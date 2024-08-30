use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
}
pub const CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Metadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u128,
    pub token_uri: String,
}
pub const METADATA: Item<Metadata> = Item::new("metadata");

pub const MINTED: Item<u128> = Item::new("minted");

pub const BALANCE_OF: Map<Addr, Uint128> = Map::new("balance_of");

pub const ALLOWANCE: Map<(Addr, Addr), Uint128> = Map::new("allowance");

pub const GET_APPROVED: Map<u128, Addr> = Map::new("get_approved");

pub const IS_APPROVED_FOR_ALL: Map<(Addr, Addr), bool> = Map::new("is_approved_for_all");

pub const OWNER_OF: Map<u128, Addr> = Map::new("owner_of");

pub const OWNED: Map<Addr, Vec<u128>> = Map::new("owned");

pub const OWNED_INDEX: Map<u128, u128> = Map::new("owned_index");

pub const WHITELIST: Map<Addr, bool> = Map::new("whitelist");

pub const LAST_MINT_SECONDS: Map<Addr, u64> = Map::new("last_mint_seconds");
