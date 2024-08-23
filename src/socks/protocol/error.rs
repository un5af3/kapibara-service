//! Socks Error

use std::{str::Utf8Error, string::FromUtf8Error};

#[derive(thiserror::Error, Debug)]
pub enum SocksError {
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Utf-8 error: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("Utf-8 error: {0}")]
    FromUtf8(#[from] FromUtf8Error),
    #[error("Invalid SOCKS version: {0:x}")]
    InvalidVersion(u8),
    #[error("Invalid command: {0:x}")]
    InvalidCommand(u8),
    #[error("Invalid address")]
    InvalidAddress,
    #[error("Invalid address type: {0:x}")]
    InvalidAddrType(u8),
    #[error("Invalid authentication method: {0:x}")]
    InvalidAuthMethod(u8),
    #[error("Invalid authentication: {0}")]
    InvalidAuth(String),
    #[error("Unknown authentication")]
    UnknonwAuth,
    #[error("Invalid status {0:x}")]
    InvalidStatus(u8),
    #[error("String more than 255 bytes `{0}`")]
    TooLongString(String),
    #[error("Unsupport frame")]
    UnsupportFrame,
    #[error("Unsupport address type")]
    UnsupportAddrtype,
    #[error("Unsupport authentication type")]
    UnsupportAuthType,
    #[error("Unsupport authentication method")]
    UnsupportAuthMethod,
    #[error("Handshake finished status: {0}")]
    HandshakeFinished(String),
}
