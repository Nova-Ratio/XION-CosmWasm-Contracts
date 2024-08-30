use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Empty};

use cw20::{BalanceResponse, Cw20ExecuteMsg, MinterResponse, TokenInfoResponse};
use cw721::{Cw721ExecuteMsg, NftInfoResponse, NumTokensResponse, OwnerOfResponse, TokensResponse};

#[cw_serde]
pub struct InitialBalance {
    pub target: String,
    pub amount: u128,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_native_supply: u128,
    pub token_uri: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    SetWhitelist {
        target: String,
        state: bool,
    },
    Approve {
        spender: String,
        amount_or_id: u128,
    },
    SetApprovalForAll {
        operator: String,
        approved: bool,
    },
    IncreaseAllowance {
        spender: String,
        amount: u128,
    },
    TransferFrom {
        owner: String,
        recipient: String,
        amount: u128,
    },
    Transfer {
        recipient: String,
        amount: u128,
    },
    TransferNft {
        recipient: String,
        token_id: String,
    },
    Send {
        contract: String,
        amount: u128,
        msg: Binary,
    },
    SendNft {
        contract: String,
        token_id: String,
        msg: Binary,
    },
    // Wrapper code
    Mint {},
    AirdropNft {
        recipient: String,
        nft_amount: u128,
    },
}

impl From<Cw20ExecuteMsg> for ExecuteMsg {
    fn from(msg: Cw20ExecuteMsg) -> Self {
        match msg {
            // correct from statement
            Cw20ExecuteMsg::TransferFrom {
                owner,
                recipient,
                amount,
            } => ExecuteMsg::TransferFrom {
                owner,
                recipient,
                amount: amount.u128(),
            },
            Cw20ExecuteMsg::Transfer { recipient, amount } => ExecuteMsg::Transfer {
                recipient,
                amount: amount.u128(),
            },
            Cw20ExecuteMsg::Send {
                contract,
                amount,
                msg,
            } => ExecuteMsg::Send {
                contract,
                amount: amount.u128(),
                msg,
            },
            _ => panic!("Unsupported message"),
        }
    }
}

impl From<Cw721ExecuteMsg> for ExecuteMsg {
    fn from(msg: Cw721ExecuteMsg) -> Self {
        match msg {
            Cw721ExecuteMsg::TransferNft {
                recipient,
                token_id,
            } => ExecuteMsg::TransferNft {
                recipient,
                token_id,
            },
            Cw721ExecuteMsg::SendNft {
                contract,
                token_id,
                msg,
            } => ExecuteMsg::SendNft {
                contract,
                token_id,
                msg,
            },
            _ => panic!("Unsupported message"),
        }
    }
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    // cw20
    #[returns(BalanceResponse)]
    Balance { address: String },
    #[returns(TokenInfoResponse)]
    TokenInfo {},
    #[returns(MinterResponse)]
    Minter {},
    // cw721
    #[returns(OwnerOfResponse)]
    OwnerOf { token_id: String },
    #[returns(TokensResponse)]
    Tokens { owner: String },
    #[returns(NumTokensResponse)]
    NumTokens {},
    #[returns(NftInfoResponse<Empty>)]
    NftInfo { token_id: String },
}

#[cw_serde]
pub struct MigrateMsg {}
