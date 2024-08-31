//! Outbound Service

use tokio::io::{AsyncRead, AsyncWrite, BufStream};

use crate::{
    address::NetworkType,
    direct::{DirectOutbound, DirectStream},
    http::HttpOutbound,
    option::OutboundServiceOption,
    socks::SocksOutbound,
    vless::{VlessOutbound, VlessOutboundStream},
    OutboundResult, OutboundServiceTrait, ServiceAddress,
};

#[derive(Debug, Clone)]
pub struct OutboundPacket {
    pub typ: NetworkType,
    pub dest: ServiceAddress,
}

macro_rules! outbound_service_enum {
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

        impl<S> OutboundServiceTrait<S> for $name
        where
            S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
        {
            type Stream = OutboundServiceStream<S>;

            async fn handshake(&self, stream: S, packet: OutboundPacket) -> OutboundResult<Self::Stream> {
                match self {
                    $(
                        $name::$id(svc) => Ok(svc.handshake(stream, packet).await?.into()),
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

macro_rules! out_stream_traits_enum {
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

outbound_service_enum! {
    #[derive(Debug)]
    pub enum OutboundService {
        Direct(DirectOutbound),
        Vless(VlessOutbound),
        Socks(SocksOutbound),
        Http(HttpOutbound),
    }
}

out_stream_traits_enum! {
    #[derive(Debug)]
    pub enum OutboundServiceStream<S>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    {
        Raw(S),
        Buf(BufStream<S>),
        Direct(DirectStream),
        Vless(VlessOutboundStream<S>),
    }
}

impl<S> From<S> for OutboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: S) -> Self {
        Self::Raw(value)
    }
}

impl<S> From<BufStream<S>> for OutboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: BufStream<S>) -> Self {
        Self::Buf(value)
    }
}

impl OutboundService {
    pub fn init(opt: OutboundServiceOption) -> OutboundResult<OutboundService> {
        match opt {
            OutboundServiceOption::Direct => Ok(DirectOutbound.into()),
            OutboundServiceOption::Vless(o) => Ok(VlessOutbound::init(o)?.into()),
            OutboundServiceOption::Socks(o) => Ok(SocksOutbound::init(o)?.into()),
            OutboundServiceOption::Http(o) => Ok(HttpOutbound::init(o)?.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{vless::VlessOutboundOption, ServiceAddress};

    use super::*;

    #[tokio::test]
    async fn test_service_outbound() {
        let buf: Vec<u8> = vec![];
        let stream = Cursor::new(buf);

        let opt = OutboundServiceOption::Vless(VlessOutboundOption {
            uuid: "fc42fe34-e267-4c69-8861-2bc419057519".into(),
            flow: None,
        });

        let svc = OutboundService::init(opt).unwrap();

        let packet = OutboundPacket {
            typ: NetworkType::Tcp,
            dest: ServiceAddress {
                addr: "127.0.0.1".into(),
                port: 1234,
            },
        };

        let result = svc.handshake(stream, packet).await.unwrap();

        println!("{} {:?}", svc.name(), result);
    }
}
