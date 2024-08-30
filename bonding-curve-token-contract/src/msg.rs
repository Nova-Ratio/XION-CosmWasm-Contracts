use bonding_types::msg::{CurveType, MarketingInfoResponse};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Decimal, Uint128};
use cw20::AllowanceResponse as Cw20AllowanceResponse;
use cw20::BalanceResponse as Cw20BalanceResponse;
use cw20::Cw20ReceiveMsg;
use cw20::Denom;
use cw20::Expiration;
use cw20::TokenInfoResponse as Cw20TokenInfoResponse;

#[cw_serde]
pub enum ReceiveMsg {
    Buy {},
}
#[cw_serde]
pub enum ExecuteMsg {
    /// Implements CW20. Transfer is a base message to move tokens to another account without triggering actions
    Transfer {
        recipient: String,
        amount: Uint128,
    },
    /// Implements CW20. Burn is a base message to destroy tokens forever
    Burn {
        amount: Uint128,
    },
    /// Implements CW20.  Send is a base message to transfer tokens to a contract and trigger an action
    /// on the receiving contract.
    Send {
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Implements CW20 "approval" extension. Allows spender to access an additional amount tokens
    /// from the owner's (env.sender) account. If expires is Some(), overwrites current allowance
    /// expiration with this one.
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Lowers the spender's access of tokens
    /// from the owner's (env.sender) account by amount. If expires is Some(), overwrites current
    /// allowance expiration with this one.
    DecreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Transfers amount tokens from owner -> recipient
    /// if `env.sender` has sufficient pre-approval.
    TransferFrom {
        owner: String,
        recipient: String,
        amount: Uint128,
    },
    /// Implements CW20 "approval" extension. Sends amount tokens from owner -> contract
    /// if `env.sender` has sufficient pre-approval.
    SendFrom {
        owner: String,
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Implements CW20 "approval" extension. Destroys tokens forever
    BurnFrom {
        owner: String,
        amount: Uint128,
    },

    Receive(Cw20ReceiveMsg),
    Buy {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the reserve and supply quantities, as well as the spot price to buy 1 token
    #[returns(CurveInfoResponse)]
    CurveInfo {},
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    #[returns(Cw20BalanceResponse)]
    Balance { address: String },
    /// Implements CW20. Returns metadata on the contract - name, decimals, supply, etc.
    #[returns(Cw20TokenInfoResponse)]
    TokenInfo {},
    /// Implements CW20 "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    #[returns(Cw20AllowanceResponse)]
    Allowance { owner: String, spender: String },
    // Marketing info
    #[returns(MarketingInfoResponse)]
    MarketingInfo {},
}

#[cw_serde]
pub struct CurveInfoResponse {
    // how many reserve tokens have been received
    pub reserve: Uint128,
    // how many supply tokens have been issued
    pub supply: Uint128,
    pub spot_price: Decimal,
    pub reserve_denom: Denom,
    pub curve_type: CurveType,
}
