use crate::state::Listing;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint256;
use cw20::Cw20ReceiveMsg;
use cw721::Cw721ReceiveMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub cw404_address: String,
    pub cw20_address: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    CancelListing { id: String },
    Receive(Cw20ReceiveMsg),
    ReceiveNft(Cw721ReceiveMsg),
}

#[cw_serde]
pub enum ReceiveMsg {
    Buy { id: String },
}

#[cw_serde]
pub enum ReceiveNftMsg {
    NewListing { price: Uint256 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Listing)]
    GetListing { id: String },
    #[returns(Vec<Listing>)]
    GetListingsBySeller {
        seller: String,
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    #[returns(Vec<Listing>)]
    GetAllListings {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    #[returns(u128)]
    GetListingCount {},
}

#[cw_serde]
pub struct MigrateMsg {}
