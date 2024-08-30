use std::num::ParseIntError;

use cosmwasm_std::{DivideByZeroError, OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),

    #[error("{0}")]
    ParseIntError(#[from] ParseIntError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Already exists")]
    AlreadyExists {},

    #[error("Invalid sender")]
    InvalidSender {},

    #[error("Invalid input")]
    InvalidInput {},

    #[error("Invalid funds")]
    InvalidFunds {},

    #[error("Already minted today")]
    AlreadyMintedToday {},

    #[error("Out of supply")]
    OutOfSupply {},
}
