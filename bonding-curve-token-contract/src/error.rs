use cosmwasm_std::StdError;
use cw_asset::AssetError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Base(#[from] cw20_base::ContractError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Wrong cw20 denom")]
    WrongCw20Denom {},

    #[error("Wrong native denom")]
    WrongNativeDenom {},

    #[error("Can not send sero amount")]
    ZeroPayment {},

    #[error("Asset error")]
    AssetError {},
}

impl From<AssetError> for ContractError {
    fn from(_err: AssetError) -> Self {
        ContractError::AssetError {}
    }
}
