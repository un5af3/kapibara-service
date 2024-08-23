//! Inbound Service

use std::borrow::Cow;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    address::NetworkType, option::InboundServiceOption, socks::SocksInbound,
    svc_stream_traits_enum, vless::VlessInbound, InboundResult, InboundServiceTrait,
    ServiceAddress,
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

inbound_service_enum! {
    #[derive(Debug)]
    pub enum InboundService {
        Vless(VlessInbound),
        Socks(SocksInbound),
    }
}

svc_stream_traits_enum! {
    #[derive(Debug)]
    pub enum InboundServiceStream<S>
    where
     S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    {
        Raw(S),
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

impl InboundService {
    pub fn init(opt: InboundServiceOption) -> InboundResult<InboundService> {
        match opt {
            InboundServiceOption::Vless(o) => Ok(VlessInbound::init(o)?.into()),
            InboundServiceOption::Socks(o) => Ok(SocksInbound::init(o)?.into()),
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
