use std::str::FromStr;

use tokio::io::{AsyncRead, AsyncWrite, BufStream};
use uuid::Uuid;

use crate::{
    address::NetworkType, OutboundError, OutboundPacket, OutboundResult, OutboundServiceTrait,
};

use super::{
    protocol::{Response, COMMAND_TCP, COMMAND_UDP},
    Request, VlessOutboundOption,
};

#[derive(Debug)]
pub struct VlessOutbound {
    uuid: uuid::Uuid,
    flow: Option<String>,
}

impl VlessOutbound {
    pub fn init(option: VlessOutboundOption) -> OutboundResult<Self> {
        let uuid =
            Uuid::from_str(&option.uuid).map_err(|e| OutboundError::Option(e.to_string()))?;

        Ok(Self {
            uuid,
            flow: option.flow,
        })
    }
}

impl<S> OutboundServiceTrait<S> for VlessOutbound
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Stream = S;

    async fn handshake(&self, stream: S, packet: OutboundPacket) -> OutboundResult<Self::Stream> {
        let command = match packet.typ {
            NetworkType::Tcp => COMMAND_TCP,
            NetworkType::Udp => COMMAND_UDP,
        };

        let req = &Request {
            uuid: self.uuid,
            flow: self.flow.clone(),
            command,
            destination: Some(packet.dest),
        };

        let mut stream = BufStream::with_capacity(1024, 1024, stream);
        let _ = req
            .write(&mut stream, None)
            .await
            .map_err(|e| OutboundError::Handshake(e.into()))?;

        let _resp = Response::read(&mut stream)
            .await
            .map_err(|e| OutboundError::Handshake(e.into()))?;

        let stream = stream.into_inner();

        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::ServiceAddress;

    use super::*;

    #[tokio::test]
    async fn test_vless_outbound() {
        let buf: Vec<u8> = vec![];
        let stream = Cursor::new(buf);

        let opt = VlessOutboundOption {
            uuid: "fc42fe34-e267-4c69-8861-2bc419057519".into(),
            flow: None,
        };

        let vo = VlessOutbound::init(opt).unwrap();

        let packet = OutboundPacket {
            typ: NetworkType::Tcp,
            dest: ServiceAddress {
                addr: "127.0.0.1".into(),
                port: 1234,
            },
        };

        let result = vo.handshake(stream, packet).await.unwrap();

        println!("{:?}", result);
    }
}
