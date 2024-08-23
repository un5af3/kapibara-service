//! Kapibara Service Error

use thiserror::Error;

use crate::{socks::SocksError, vless::VlessError};

#[derive(Debug, Error)]
pub enum InboundError {
    #[error("io error ({0})")]
    Io(#[from] std::io::Error),
    #[error("option error ({0})")]
    Option(String),
    #[error("handshake error ({0})")]
    Handshake(#[from] ProtocolError),
}

#[derive(Debug, Error)]
pub enum OutboundError {
    #[error("io error ({0})")]
    Io(#[from] std::io::Error),
    #[error("option error ({0})")]
    Option(String),
    #[error("handshake error ({0})")]
    Handshake(#[from] ProtocolError),
    #[error("unresolved address")]
    Unresolved,
}

#[derive(Debug, Error)]
pub enum AddressError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("invalid address type")]
    InvalidAddrType,
    #[error("invalid address {0}")]
    InvalidAddress(String),
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("[vless] {0}")]
    Vless(#[from] VlessError),
    #[error("[socks] {0}")]
    Socks(#[from] SocksError),
}
