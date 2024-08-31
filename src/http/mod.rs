//! Http Proxy Service

use http::{status::InvalidStatusCode, StatusCode};
use thiserror::Error;

pub mod option;
pub use option::{HttpInboundOption, HttpOutboundOption};

pub mod inbound;
pub use inbound::{HttpInbound, HttpInboundStream};

pub mod outbound;
pub use outbound::HttpOutbound;

pub mod protocol;
pub use protocol::{
    format_request, format_response, read_request, read_response, write_request, write_response,
};

const MAX_HEADER: usize = 64;
const MAX_HEADER_SIZE: usize = 65535;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Http(#[from] http::Error),
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid response")]
    InvalidResponse,
    #[error("invalid host")]
    InvalidHost,
    #[error("invalid authentication")]
    InvalidAuth,
    #[error("invalid line {0}")]
    InvalidLine(String),
    #[error("invalid version")]
    InvalidVersion,
    #[error("{0}")]
    InvalidMethod(#[from] http::method::InvalidMethod),
    #[error("{0}")]
    InvalidUri(#[from] http::uri::InvalidUri),
    #[error("{0}")]
    InvalidStatus(#[from] InvalidStatusCode),
    #[error("{0}")]
    InvalidStatusCode(StatusCode),
    #[error("header too large")]
    HeaderTooLarge,
}

#[derive(Debug, Clone)]
pub struct HttpAuth {
    pub user: Vec<u8>,
    pub pass: Vec<u8>,
}
