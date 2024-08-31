//! Inbound Service

use std::borrow::Cow;

use tokio::io::{AsyncRead, AsyncWrite, BufStream};

use crate::{
    address::NetworkType,
    http::{HttpInbound, HttpInboundStream},
    mixed::{MixedInbound, MixedInboundStream},
    option::InboundServiceOption,
    socks::SocksInbound,
    vless::VlessInbound,
    CachedStream, InboundResult, InboundServiceTrait, ServiceAddress,
};

#[derive(Debug, Clone)]
pub struct InboundPacket<'a> {
    pub typ: NetworkType,
    pub dest: ServiceAddress,
    pub detail: Cow<'a, str>,
}

macro_rules! inbound_service_enum {
    {
        $(#[$meta:meta])*
        $v:vis enum $name:ident
        {
            $(
                $(#[$item_meta:meta])*
                $id:ident($id_ty:ty),
            )+
        }
    } => {
        $(#[$meta])*
        $v enum $name {
            $(
                $(#[$item_meta])*
                $id($id_ty),
            )+
        }

        impl $name {
            pub fn name(&self) -> &str {
                match self {
                    $(
                        $name::$id(_) => stringify!($id),
                    )+
                }
            }
        }

        impl<S> InboundServiceTrait<S> for $name
        where
            S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
        {
            type Stream = InboundServiceStream<S>;

            async fn handshake(&self, stream: S) -> InboundResult<(Self::Stream, InboundPacket)> {
                match self {
                    $(
                        $name::$id(svc) => {
                            let (s, p) = svc.handshake(stream).await?;
                            Ok((s.into(), p))
                        }
                    )+
                }
            }
        }

        $(
            impl From<$id_ty> for $name {
                fn from(s: $id_ty) -> $name {
                    $name::$id(s)
                }
            }
        )+
    };
}

macro_rules! in_stream_traits_enum {
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

inbound_service_enum! {
    #[derive(Debug)]
    pub enum InboundService {
        Http(HttpInbound),
        Socks(SocksInbound),
        Miexd(MixedInbound),
        Vless(VlessInbound),
    }
}

in_stream_traits_enum! {
    #[derive(Debug)]
    pub enum InboundServiceStream<S>
    where
     S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    {
        Raw(S),
        Buf(BufStream<S>),
        Cached(CachedStream<S>),
        Http(HttpInboundStream<S>),
        Mixed(MixedInboundStream<S>),
    }
}

impl<S> From<S> for InboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: S) -> Self {
        Self::Raw(value)
    }
}

impl<S> From<BufStream<S>> for InboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: BufStream<S>) -> Self {
        Self::Buf(value)
    }
}

impl<S> From<CachedStream<S>> for InboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: CachedStream<S>) -> Self {
        Self::Cached(value)
    }
}

impl InboundService {
    pub fn init(opt: InboundServiceOption) -> InboundResult<InboundService> {
        match opt {
            InboundServiceOption::Http(o) => Ok(HttpInbound::init(o)?.into()),
            InboundServiceOption::Socks(o) => Ok(SocksInbound::init(o)?.into()),
            InboundServiceOption::Mixed(o) => Ok(MixedInbound::init(o)?.into()),
            InboundServiceOption::Vless(o) => Ok(VlessInbound::init(o)?.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::vless::{option::VlessUserOption, VlessInboundOption};

    use super::*;

    #[tokio::test]
    async fn test_service_inbound() {
        let buf: Vec<u8> = vec![
            0, 252, 66, 254, 52, 226, 103, 76, 105, 136, 97, 43, 196, 25, 5, 117, 25, 0, 1, 34,
            184, 1, 127, 0, 0, 1, 116, 101, 115, 116,
        ];

        let s = Cursor::new(buf);

        let opt = InboundServiceOption::Vless(VlessInboundOption {
            users: vec![VlessUserOption {
                user: "test".into(),
                uuid: "fc42fe34-e267-4c69-8861-2bc419057519".into(),
            }],
        });

        let svc = InboundService::init(opt).unwrap();

        let result = svc.handshake(s).await.unwrap();

        println!("{} {:?}", svc.name(), result)
    }
}
