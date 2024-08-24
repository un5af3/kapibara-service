//! Outbound Service

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    address::NetworkType,
    direct::{DirectOutbound, DirectStream},
    option::OutboundServiceOption,
    socks::SocksOutbound,
    svc_stream_traits_enum,
    vless::{outbound::VlessStream, VlessOutbound},
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

outbound_service_enum! {
    #[derive(Debug)]
    pub enum OutboundService {
        Direct(DirectOutbound),
        Vless(VlessOutbound),
        Socks(SocksOutbound),
    }
}

svc_stream_traits_enum! {
    #[derive(Debug)]
    pub enum OutboundServiceStream<S>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    {
        Raw(S),
        Vless(VlessStream<S>),
        Direct(DirectStream),
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

impl OutboundService {
    pub fn init(opt: OutboundServiceOption) -> OutboundResult<OutboundService> {
        match opt {
            OutboundServiceOption::Direct => Ok(DirectOutbound.into()),
            OutboundServiceOption::Vless(o) => Ok(VlessOutbound::init(o)?.into()),
            OutboundServiceOption::Socks(o) => Ok(SocksOutbound::init(o)?.into()),
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
