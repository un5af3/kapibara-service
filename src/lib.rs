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

pub mod direct;
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

#[macro_export]
macro_rules! svc_stream_traits_enum {
    {
        $(#[$meta:meta])*
        $v:vis enum $name:ident<S>
        where
            S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
        {
            $(
                $(#[$item_meta:meta])*
                $id:ident($id_ty:ty),
            )+
        }
    } => {
        $(#[$meta])*
        $v enum $name<S>
        where
            S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
        {
            $(
                $(#[$item_meta])*
                $id($id_ty),
            )+
        }

        impl<S> tokio::io::AsyncRead for $name<S>
        where
            S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
        {
            #[inline]
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                match self.get_mut() {
                    $(
                        $name::$id(val) => std::pin::Pin::new(val).poll_read(cx, buf),
                    )+
                }
            }
        }

        impl<S> tokio::io::AsyncWrite for $name<S>
        where
            S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
        {
            #[inline]
            fn poll_write(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                match self.get_mut() {
                    $(
                        $name::$id(val) => std::pin::Pin::new(val).poll_write(cx, buf),
                    )+
                }
            }

            #[inline]
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                match self.get_mut() {
                    $(
                        $name::$id(val) => std::pin::Pin::new(val).poll_flush(cx),
                    )+
                }
            }

            #[inline]
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                match self.get_mut() {
                    $(
                        $name::$id(val) => std::pin::Pin::new(val).poll_shutdown(cx),
                    )+
                }
            }
        }
    };
}
