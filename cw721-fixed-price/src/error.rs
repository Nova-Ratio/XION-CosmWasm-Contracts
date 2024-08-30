use cosmwasm_std::StdError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("PaymentError")]
    PaymentError(PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InvalidUnitPrice")]
    InvalidUnitPrice {},

    #[error("InvalidMaxTokens")]
    InvalidMaxTokens {},

    #[error("SoldOut")]
    SoldOut {},

    #[error("UnauthorizedTokenContract")]
    UnauthorizedTokenContract {},

    #[error("Uninitialized")]
    Uninitialized {},

    #[error("WrongPaymentAmount")]
    WrongPaymentAmount {},

    #[error("InvalidTokenReplyId")]
    InvalidTokenReplyId {},

    #[error("Cw721NotLinked")]
    Cw721NotLinked {},

    #[error("Cw721AlreadyLinked")]
    Cw721AlreadyLinked {},
}

impl From<PaymentError> for ContractError {
    fn from(err: PaymentError) -> Self {
        ContractError::PaymentError(err)
    }
}
