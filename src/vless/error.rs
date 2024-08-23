//! Vless Error

use thiserror::Error;

use crate::error::AddressError;

#[derive(Debug, Error)]
pub enum VlessError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("no destination")]
    NoDestination,
    #[error("invalid version: {0}")]
    InvalidVersion(u8),
    #[error("unknown version")]
    UnknownVersion,
    #[error("{0}")]
    InvalidAddress(#[from] AddressError),
    #[error("invalid command: {0}")]
    InvalidCommand(u8),
    #[error("invalid uuid: {0}")]
    InvalidUuid(String),
    #[error("invalid header: {0}")]
    InvalidHeader(u8),
}
