//! Socks service for outbound

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    address::NetworkType, Address, OutboundError, OutboundPacket, OutboundResult,
    OutboundServiceTrait,
};

use super::{
    protocol::{
        SocksAddr, SocksAuth, SocksClientHandshake, SocksCommand, SocksRequest, SocksStatus,
        SocksVersion,
    },
    SocksError, SocksOutboundOption,
};

#[derive(Debug)]
pub struct SocksOutbound {
    version: SocksVersion,
    auth: SocksAuth,
}

impl SocksOutbound {
    pub fn init(option: SocksOutboundOption) -> OutboundResult<Self> {
        let version = option.version.try_into().map_err(|n| {
            OutboundError::Option(format!("unsupport service socks version: {0:x}", n))
        })?;

        let auth: SocksAuth = option.auth.into();

        if !auth.validate(version) {
            return Err(OutboundError::Option(
                "authentication method dismatch socks version".to_string(),
            )
            .into());
        }

        Ok(Self { auth, version })
    }
}

impl<S> OutboundServiceTrait<S> for SocksOutbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = S;

    async fn handshake(
        &self,
        mut stream: S,
        packet: OutboundPacket,
    ) -> OutboundResult<Self::Stream> {
        let addr = match packet.dest.addr {
            Address::Domain(domain) => SocksAddr::Domain(domain),
            Address::Socket(ip) => SocksAddr::Socket(ip),
        };

        let port = packet.dest.port;

        let command = match packet.typ {
            NetworkType::Tcp => SocksCommand::CONNECT,
            NetworkType::Udp => SocksCommand::UDP_ASSOCIATE,
        };

        let req = SocksRequest::new(self.version, command, addr, port, self.auth.clone())
            .map_err(|e| OutboundError::Handshake(e.into()))?;

        let mut cli = SocksClientHandshake::new(req);

        let reply = cli
            .connect(&mut stream)
            .await
            .map_err(|e| OutboundError::Handshake(e.into()))?;

        if reply.status() != SocksStatus::SUCCEEDED {
            return Err(OutboundError::Handshake(
                SocksError::InvalidStatus(reply.status().into()).into(),
            ));
        }

        Ok(stream)
    }
}
