//! Kapibara service library

use tokio::io::{AsyncRead, AsyncWrite};

pub mod error;
pub use error::{InboundError, OutboundError};

pub mod option;
pub use option::{InboundServiceOption, OutboundServiceOption};

pub mod inbound;
pub use inbound::{InboundPacket, InboundService, InboundServiceStream};

pub mod outbound;
pub use outbound::{OutboundPacket, OutboundService, OutboundServiceStream};

pub mod address;
pub use address::{AddrType, AddrTypeConvert, Address, ServiceAddress};

pub mod varint;
pub use varint::{read_varint, variant_len, write_varint};

pub mod stream;
pub use stream::CachedStream;

pub mod direct;
pub mod http;
pub mod mixed;
pub mod socks;
pub mod vless;

pub type InboundResult<T> = std::result::Result<T, InboundError>;
pub type OutboundResult<T> = std::result::Result<T, OutboundError>;

#[trait_variant::make(InboundServiceTrait: Send + Sync)]
pub trait LocalInboundServiceTrait<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + Sync;

    async fn handshake(&self, stream: S) -> InboundResult<(Self::Stream, InboundPacket)>;
}

#[trait_variant::make(OutboundServiceTrait: Send + Sync)]
pub trait LocalOutboundServiceTrait<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + Sync;

    async fn handshake(&self, stream: S, packet: OutboundPacket) -> OutboundResult<Self::Stream>;
}
