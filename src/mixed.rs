//! Mixed for socks5 or http proxy

use std::pin::Pin;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufStream};

use crate::{
    http::{option::HttpAuthOption, HttpInbound, HttpInboundOption, HttpInboundStream},
    socks::{option::SocksAuthOption, SocksInbound, SocksInboundOption},
    CachedStream, InboundPacket, InboundResult, InboundServiceStream, InboundServiceTrait,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MixedInboundOption {
    #[serde(default)]
    auth: Vec<MixedAuthOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedAuthOption {
    user: String,
    pass: String,
}

#[derive(Debug)]
pub struct MixedInbound {
    http_in: HttpInbound,
    socks_in: SocksInbound,
}

impl MixedInbound {
    pub fn init(opt: MixedInboundOption) -> InboundResult<Self> {
        let socks_opt = SocksInboundOption {
            auth: opt
                .auth
                .iter()
                .map(|auth| SocksAuthOption::Username {
                    user: auth.user.clone(),
                    pass: auth.pass.clone(),
                })
                .collect(),
        };
        let socks_in = SocksInbound::init(socks_opt)?;

        let http_opt = HttpInboundOption {
            auth: opt
                .auth
                .into_iter()
                .map(|auth| HttpAuthOption {
                    user: auth.user,
                    pass: auth.pass,
                })
                .collect(),
        };
        let http_in = HttpInbound::init(http_opt)?;

        Ok(Self { http_in, socks_in })
    }
}

impl<S> InboundServiceTrait<S> for MixedInbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = MixedInboundStream<S>;

    async fn handshake(&self, mut stream: S) -> InboundResult<(Self::Stream, InboundPacket)> {
        let byte = stream.read_u8().await?;

        let stream = CachedStream::new(stream, Some(Bytes::from(vec![byte].into_boxed_slice())));
        match byte {
            4 | 5 => {
                let (stream, pac) = self.socks_in.handshake(stream).await?;
                let stream = MixedInboundStream::Socks(stream);
                Ok((stream, pac))
            }
            _ => {
                let (stream, pac) = self.http_in.handshake(stream).await?;
                let stream = MixedInboundStream::Http(stream);
                Ok((stream, pac))
            }
        }
    }
}

#[derive(Debug)]
pub enum MixedInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    Http(HttpInboundStream<CachedStream<S>>),
    Socks(BufStream<CachedStream<S>>),
}

impl<S> From<MixedInboundStream<S>> for InboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    fn from(value: MixedInboundStream<S>) -> Self {
        Self::Mixed(value)
    }
}

impl<S> AsyncRead for MixedInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Http(s) => Pin::new(s).poll_read(cx, buf),
            Self::Socks(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<S> AsyncWrite for MixedInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            Self::Http(s) => Pin::new(s).poll_write(cx, buf),
            Self::Socks(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Http(s) => Pin::new(s).poll_flush(cx),
            Self::Socks(s) => Pin::new(s).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Http(s) => Pin::new(s).poll_shutdown(cx),
            Self::Socks(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
