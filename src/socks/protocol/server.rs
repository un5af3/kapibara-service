//! Socks protocol server handshake

use core::str;
use std::net::{IpAddr, Ipv4Addr};

use bytes::BufMut;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use super::{
    SocksAddr, SocksAuth, SocksCommand, SocksError, SocksRequest, SocksStatus, SocksVersion,
    NO_AUTHENTICATION, USERNAME_PASSWORD,
};

const UNSPECIFIED_ADDR: SocksAddr = SocksAddr::Socket(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

#[derive(Debug, Clone)]
pub struct SocksServerHandshake {
    state: State,
    auth: Option<SocksAuth>,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
enum State {
    /// Starting state: no messages have been handled yet.
    Initial,
    /// SOCKS5: we've negotiated Username/Password authentication, and
    /// are waiting for the client to send it.
    Socks5Username,
    /// SOCKS5: we've finished the authentication (if any), and
    /// we're waiting for the actual request.
    Socks5Wait,
    /// Ending (successful) state: the client has sent all its messages.
    ///
    /// (Note that we still need to send a reply.)
    Done,
    /// Ending (failed) state: the handshake has failed and cannot continue.
    Failed,
}

impl SocksServerHandshake {
    pub fn new() -> Self {
        Self {
            auth: None,
            state: State::Initial,
        }
    }

    pub async fn accept<S>(&mut self, stream: &mut S) -> Result<SocksRequest, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        loop {
            if let Some(request) = self.handshake(stream).await? {
                return Ok(request);
            }
        }
    }

    pub async fn handshake<S>(&mut self, stream: &mut S) -> Result<Option<SocksRequest>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ver = stream.read_u8().await?;

        let result = match (self.state, ver) {
            (State::Initial, 4) => self.s4(stream).await,
            (State::Initial, 5) => self.s5_initial(stream).await,
            (State::Initial, v) => Err(SocksError::InvalidVersion(v)),
            (State::Socks5Username, 1) => self.s5_uname(stream).await,
            (State::Socks5Wait, 5) => self.s5(stream).await,
            (State::Done, _) => Err(SocksError::HandshakeFinished("done".to_string())),
            (State::Failed, _) => Err(SocksError::HandshakeFinished("failed".to_string())),
            _ => Err(SocksError::UnsupportFrame),
        };

        if result.is_err() {
            self.state = State::Failed;
        }

        result
    }

    pub async fn s4<S>(&mut self, stream: &mut S) -> Result<Option<SocksRequest>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let command: SocksCommand = stream
            .read_u8()
            .await?
            .try_into()
            .map_err(|n| SocksError::InvalidCommand(n))?;
        let port = stream.read_u16().await?;
        let ip = stream.read_u32().await?;

        let mut buf = Vec::with_capacity(256);
        buf.clear();
        let n = stream.read_until(0, &mut buf).await?;
        let auth = if n == 0 {
            SocksAuth::NoAuth
        } else {
            SocksAuth::Socks4(buf[..n - 1].to_vec())
        };

        let addr = if ip != 0 && (ip >> 8) == 0 {
            // Socks4a; a hostname is given.
            buf.clear();
            let n = stream.read_until(0, &mut buf).await?;
            if n == 0 {
                return Err(SocksError::InvalidAddress);
            }

            let hostname = str::from_utf8(&buf[..n - 1])?;

            SocksAddr::Domain(hostname.to_owned())
        } else {
            let ip4: std::net::Ipv4Addr = ip.into();
            SocksAddr::Socket(ip4.into())
        };

        let request = SocksRequest::new(SocksVersion::V4, command, addr, port, auth)?;

        self.state = State::Done;

        Ok(Some(request))
    }

    pub async fn s5_initial<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<Option<SocksRequest>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let nmethods = stream.read_u8().await?;
        let mut methods = vec![0u8; nmethods as usize];
        let _ = stream.read_exact(&mut methods).await?;
        let (next, reply) = if methods.contains(&USERNAME_PASSWORD) {
            (State::Socks5Username, [5, USERNAME_PASSWORD])
        } else if methods.contains(&NO_AUTHENTICATION) {
            self.auth = Some(SocksAuth::NoAuth);
            (State::Socks5Wait, [5, NO_AUTHENTICATION])
        } else {
            return Err(SocksError::UnsupportAuthMethod);
        };

        let _ = stream.write_all(&reply).await?;
        let _ = stream.flush().await?;

        self.state = next;

        Ok(None)
    }

    pub async fn s5_uname<S>(&mut self, stream: &mut S) -> Result<Option<SocksRequest>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let ulen = stream.read_u8().await?;
        let mut username = vec![0u8; ulen as usize];
        let _ = stream.read_exact(&mut username).await?;

        let plen = stream.read_u8().await?;
        let mut password = vec![0u8; plen as usize];
        let _ = stream.read_exact(&mut password).await?;

        let _ = stream.write_all(&[1, 0]).await?;
        let _ = stream.flush().await?;

        self.auth = Some(SocksAuth::Username(username, password));
        self.state = State::Socks5Wait;

        Ok(None)
    }

    pub async fn s5<S>(&mut self, stream: &mut S) -> Result<Option<SocksRequest>, SocksError>
    where
        S: AsyncBufReadExt + AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let command = stream
            .read_u8()
            .await?
            .try_into()
            .map_err(|n| SocksError::InvalidCommand(n))?;
        let _ignore = stream.read_u8().await?;
        let addr = SocksAddr::read_from(stream).await?;
        let port = stream.read_u16().await?;

        let auth = self
            .auth
            .take()
            .ok_or_else(|| SocksError::UnsupportAuthType)?;

        let request = SocksRequest::new(SocksVersion::V5, command, addr, port, auth)?;

        self.state = State::Done;

        Ok(Some(request))
    }
}

impl SocksRequest {
    pub fn reply(
        &self,
        status: SocksStatus,
        addr: Option<&SocksAddr>,
    ) -> Result<Vec<u8>, SocksError> {
        match self.version() {
            SocksVersion::V4 => self.s4(status, addr),
            SocksVersion::V5 => self.s5(status, addr),
        }
    }

    fn s4(&self, status: SocksStatus, addr: Option<&SocksAddr>) -> Result<Vec<u8>, SocksError> {
        let mut w = vec![];
        w.put_u8(0);
        w.put_u8(status.into_socks4_status());
        match addr {
            Some(SocksAddr::Socket(IpAddr::V4(ip))) => {
                w.put_u16(self.port());
                w.put_slice(ip.octets().as_slice());
            }
            _ => {
                w.put_u16(0);
                w.put_u32(0);
            }
        }
        Ok(w)
    }

    fn s5(&self, status: SocksStatus, addr: Option<&SocksAddr>) -> Result<Vec<u8>, SocksError> {
        let mut w = vec![];
        w.put_u8(5);
        w.put_u8(status.into());
        w.put_u8(0); // reserved.
        if let Some(a) = addr {
            a.put_to_buf(&mut w)?;
            w.put_u16(self.port());
        } else {
            // TODO: sometimes I think we want to answer with ::, not 0.0.0.0
            UNSPECIFIED_ADDR.put_to_buf(&mut w)?;
            w.put_u16(0);
        }
        Ok(w)
    }
}
