//! Socks service for inbound

use std::borrow::Cow;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream};

use crate::{
    address::NetworkType, Address, InboundError, InboundPacket, InboundResult, InboundServiceTrait,
    ServiceAddress,
};

use super::{
    option::SocksAuthOption,
    protocol::{SocksAddr, SocksAuth, SocksCommand, SocksError, SocksServerHandshake, SocksStatus},
    SocksInboundOption,
};

#[derive(Debug)]
pub struct SocksInbound {
    users: Vec<SocksAuth>,
}

impl SocksInbound {
    pub fn init(option: SocksInboundOption) -> InboundResult<Self> {
        let mut users = vec![];
        if !option.auth.is_empty() {
            for user in option.auth.into_iter() {
                if user != SocksAuthOption::NoAuth {
                    users.push(user.into())
                }
            }
        }

        Ok(Self { users })
    }

    pub fn auth(&self, other: &SocksAuth) -> bool {
        if self.users.is_empty() && other == &SocksAuth::NoAuth {
            return true;
        }

        self.users.contains(other)
    }
}

impl<S> InboundServiceTrait<S> for SocksInbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = S;

    async fn handshake(&self, stream: S) -> InboundResult<(Self::Stream, crate::InboundPacket)> {
        let mut srv_hand = SocksServerHandshake::new();

        let mut stream = BufStream::with_capacity(1024, 1024, stream);
        let request = srv_hand
            .accept(&mut stream)
            .await
            .map_err(|e| InboundError::Handshake(e.into()))?;
        let mut stream = stream.into_inner();

        if !self.auth(request.auth()) {
            if let Ok(msg) = request.reply(SocksStatus::NOT_ALLOWED, None) {
                let _ = stream.write_all(&msg).await;
                let _ = stream.flush().await;
            }

            return Err(InboundError::Handshake(
                SocksError::InvalidAuth(request.auth().to_string()).into(),
            ));
        }

        let typ = match request.command() {
            SocksCommand::CONNECT => NetworkType::Tcp,
            SocksCommand::UDP_ASSOCIATE => NetworkType::Udp,
            other => {
                if let Ok(msg) = request.reply(SocksStatus::COMMAND_NOT_SUPPORTED, None) {
                    let _ = stream.write_all(&msg).await;
                    let _ = stream.flush().await;
                }

                return Err(InboundError::Handshake(
                    SocksError::InvalidCommand(other.into()).into(),
                ));
            }
        };

        if let Ok(msg) = request.reply(SocksStatus::SUCCEEDED, None) {
            let _ = stream.write_all(&msg).await?;
            let _ = stream.flush().await;
        }

        let port = request.port();
        let addr = match request.get_addr() {
            SocksAddr::Domain(d) => Address::Domain(d),
            SocksAddr::Socket(ip) => Address::Socket(ip),
        };

        Ok((
            stream,
            InboundPacket {
                typ,
                dest: ServiceAddress { addr, port },
                detail: Cow::Borrowed(""),
            },
        ))
    }
}
