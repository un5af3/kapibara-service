//! Socks protocol client handshake

use std::net::{IpAddr, Ipv4Addr};

use bytes::BufMut;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use super::{
    SocksAddr, SocksAuth, SocksError, SocksReply, SocksRequest, SocksStatus, SocksVersion,
    NO_AUTHENTICATION, USERNAME_PASSWORD,
};

#[derive(Clone, Debug)]
pub struct SocksClientHandshake {
    request: SocksRequest,
    state: State,
}

#[derive(Clone, Debug)]
enum State {
    Initial,
    Socks4Wait,
    Socks5AuthWait,
    Socks5UsernameWait,
    Socks5Wait,
    Done,
    Failed,
}

impl SocksClientHandshake {
    pub fn new(request: SocksRequest) -> Self {
        SocksClientHandshake {
            request,
            state: State::Initial,
        }
    }

    pub async fn connect<S>(&mut self, stream: &mut S) -> Result<SocksReply, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        loop {
            if let Some(reply) = self.handshake(stream).await? {
                return Ok(reply);
            }
        }
    }

    pub async fn handshake<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        use State::*;

        let result = match self.state {
            Initial => match self.request.version() {
                SocksVersion::V4 => self.send_v4(stream).await,
                SocksVersion::V5 => self.send_v5_initial(stream).await,
            },
            Socks4Wait => self.handle_v4(stream).await,
            Socks5AuthWait => self.handle_v5_auth(stream).await,
            Socks5UsernameWait => self.handle_v5_username_ack(stream).await,
            Socks5Wait => self.handle_v5_final(stream).await,
            Done => Err(SocksError::HandshakeFinished("succeeded".to_string())),
            Failed => Err(SocksError::HandshakeFinished("failed".to_string())),
        };

        if result.is_err() {
            self.state = State::Failed;
        }

        result
    }

    async fn send_v4<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let mut msg = vec![];

        msg.put_u8(SocksVersion::V4.into());
        msg.put_u8(self.request.command().into());
        msg.put_u16(self.request.port());

        let use_v4a = match self.request.addr() {
            SocksAddr::Socket(IpAddr::V4(ipv4)) => {
                msg.put_u32((*ipv4).into());
                false
            }
            _ => {
                msg.put_u32(1);
                true
            }
        };

        match self.request.auth() {
            SocksAuth::NoAuth => msg.put_u8(0),
            SocksAuth::Socks4(s) => {
                msg.put_slice(s.as_slice());
                msg.put_u8(0);
            }
            SocksAuth::Username(_, _) => {
                return Err(SocksError::UnsupportAuthMethod);
            }
        }

        if use_v4a {
            msg.put_slice(self.request.addr().to_string().as_bytes());
            msg.put_u8(0);
        }

        let _ = stream.write_all(&msg).await?;
        let _ = stream.flush().await?;
        self.state = State::Socks4Wait;
        Ok(None)
    }

    async fn handle_v4<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ver = stream.read_u8().await?;
        if ver != 0 {
            return Err(SocksError::InvalidVersion(ver));
        }

        let status = stream.read_u8().await?;
        let port = stream.read_u16().await?;
        let ip: Ipv4Addr = stream.read_u32().await?.into();

        self.state = State::Done;

        Ok(SocksReply {
            port,
            addr: SocksAddr::Socket(ip.into()),
            status: SocksStatus::from_socks4_status(status),
        }
        .into())
    }

    async fn send_v5_initial<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let mut msg = vec![];

        msg.put_u8(5);
        match self.request.auth() {
            SocksAuth::NoAuth => {
                msg.put_u8(1); // 1 method
                msg.put_u8(NO_AUTHENTICATION);
            }
            SocksAuth::Socks4(_) => {
                return Err(SocksError::UnsupportAuthType);
            }
            SocksAuth::Username(_, _) => {
                msg.put_u8(2); // 2 methods
                msg.put_u8(USERNAME_PASSWORD);
                msg.put_u8(NO_AUTHENTICATION);
            }
        }

        let _ = stream.write_all(&msg).await;
        let _ = stream.flush().await?;
        self.state = State::Socks5AuthWait;

        Ok(None)
    }

    async fn handle_v5_auth<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ver = stream.read_u8().await?;
        if ver != 5 {
            return Err(SocksError::InvalidVersion(ver));
        }
        let auth = stream.read_u8().await?;
        let (msg, next_state) = match auth {
            NO_AUTHENTICATION => (self.generate_v5_command()?, State::Socks5Wait),
            USERNAME_PASSWORD => (self.generate_v5_username_auth()?, State::Socks5UsernameWait),
            other => return Err(SocksError::InvalidAuthMethod(other)),
        };

        let _ = stream.write_all(&msg).await?;
        let _ = stream.flush().await?;
        self.state = next_state;

        Ok(None)
    }

    async fn handle_v5_username_ack<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ver = stream.read_u8().await?;
        if ver != 1 {
            return Err(SocksError::InvalidVersion(ver));
        }

        let result = stream.read_u8().await?;
        if result != 0 {
            return Err(SocksError::UnknonwAuth);
        }

        let msg = self.generate_v5_command()?;

        let _ = stream.write_all(&msg).await?;
        let _ = stream.flush().await?;
        self.state = State::Socks5Wait;

        Ok(None)
    }

    fn generate_v5_username_auth(&self) -> Result<Vec<u8>, SocksError> {
        if let SocksAuth::Username(user, pass) = self.request.auth() {
            let mut msg = vec![];

            msg.put_u8(1); // version

            msg.put_u8(user.len() as u8);
            msg.put_slice(user.as_slice());

            msg.put_u8(pass.len() as u8);
            msg.put_slice(pass.as_slice());

            Ok(msg)
        } else {
            Err(SocksError::UnsupportAuthType)
        }
    }

    fn generate_v5_command(&self) -> Result<Vec<u8>, SocksError> {
        let mut msg = vec![];

        msg.put_u8(5); // version
        msg.put_u8(self.request.command().into());
        msg.put_u8(0); // reserved
        self.request.addr().put_to_buf(&mut msg)?;
        msg.put_u16(self.request.port());

        Ok(msg)
    }

    async fn handle_v5_final<S>(&mut self, stream: &mut S) -> Result<Option<SocksReply>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ver = stream.read_u8().await?;
        if ver != 5 {
            return Err(SocksError::InvalidVersion(ver));
        }

        let status: SocksStatus = stream
            .read_u8()
            .await?
            .try_into()
            .map_err(|n| SocksError::InvalidStatus(n))?;
        let _reserved = stream.read_u8().await?;
        let addr = SocksAddr::read_from(stream).await?;
        let port = stream.read_u16().await?;

        self.state = State::Done;

        Ok(Some(SocksReply::new(status, addr, port)))
    }
}
