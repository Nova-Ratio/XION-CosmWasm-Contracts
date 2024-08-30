use cosmwasm_std::{StdError, Uint256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Payment is not the same as the price {price}")]
    IncorrectPayment { price: Uint256 },

    #[error("The reply ID is unrecognized")]
    UnrecognizedReply {},
}
