//! socks protocol

pub mod client;
pub use client::SocksClientHandshake;

pub mod server;
pub use server::SocksServerHandshake;

pub mod error;
pub use error::SocksError;

use std::{fmt, net::IpAddr};

use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncReadExt};

macro_rules! enum_int {
    {
        $(#[$meta:meta])*
        $v:vis enum $name:ident ($numtype:ty) {
            $(
                $(#[$item_meta:meta])*
                $id:ident = $num:literal
            ),+
            $(,)?
        }
    } => {
        $(#[$meta])*
        $v enum $name {
            $(
                $(#[$item_meta])*
                $id = $num
            ),+
        }

        impl From<$name> for $numtype {
            fn from(val: $name) -> $numtype {
                val as $numtype
            }
        }

        impl TryFrom<$numtype> for $name {
            type Error = $numtype;

            fn try_from(val: $numtype) -> std::result::Result<Self, Self::Error> {
                match val {
                    $(
                        $num => Ok($name::$id),
                    )+
                    _ => Err(val),
                }
            }
        }

        impl $name {
            $v fn get_num(self) -> $numtype {
                self.into()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        $name::$id => write!(f, "{}", stringify!($id)),
                    )+
                }
            }
        }
    };
}

/// Constant for Username/Password-style authentication.
/// (See RFC 1929)
const USERNAME_PASSWORD: u8 = 0x02;
/// Constant for "no authentication".
const NO_AUTHENTICATION: u8 = 0x00;

#[derive(Debug, Clone)]
pub struct SocksReply {
    status: SocksStatus,
    addr: SocksAddr,
    port: u16,
}

impl SocksReply {
    pub fn new(status: SocksStatus, addr: SocksAddr, port: u16) -> Self {
        Self { status, addr, port }
    }

    pub fn status(&self) -> SocksStatus {
        self.status
    }

    pub fn addr(&self) -> &SocksAddr {
        &self.addr
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Clone)]
pub struct SocksRequest {
    version: SocksVersion,
    command: SocksCommand,
    addr: SocksAddr,
    port: u16,
    auth: SocksAuth,
}

impl SocksRequest {
    pub fn new(
        version: SocksVersion,
        command: SocksCommand,
        addr: SocksAddr,
        port: u16,
        auth: SocksAuth,
    ) -> Result<Self, SocksError> {
        if !command.is_support() {
            return Err(SocksError::InvalidCommand(command.into()));
        }

        if !auth.validate(version) {
            return Err(SocksError::UnsupportAuthType);
        }

        Ok(Self {
            version,
            command,
            addr,
            port,
            auth,
        })
    }

    pub fn version(&self) -> SocksVersion {
        self.version
    }

    pub fn command(&self) -> SocksCommand {
        self.command
    }

    pub fn auth(&self) -> &SocksAuth {
        &self.auth
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn addr(&self) -> &SocksAddr {
        &self.addr
    }

    pub fn get_addr(self) -> SocksAddr {
        self.addr
    }
}

enum_int! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SocksVersion(u8) {
        V4 = 4,
        V5 = 5,
    }
}

enum_int! {
    #[derive(Debug, Clone, Copy)]
    #[allow(non_camel_case_types)]
    pub enum SocksCommand(u8) {
        CONNECT = 1,
        BIND = 2,
        UDP_ASSOCIATE = 3,
    }
}

impl SocksCommand {
    pub fn is_support(&self) -> bool {
        matches!(self, &SocksCommand::CONNECT | &SocksCommand::UDP_ASSOCIATE)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SocksAddr {
    Socket(IpAddr),
    Domain(String),
}

impl fmt::Display for SocksAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocksAddr::Domain(h) => write!(f, "{}", h),
            SocksAddr::Socket(a) => write!(f, "{}", a),
        }
    }
}

/// Provided authentication from a SOCKS handshake
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocksAuth {
    /// No authentication was provided
    NoAuth,
    /// Socks4 authentication (a string) was provided.
    Socks4(Vec<u8>),
    /// Socks5 username/password authentication was provided.
    Username(Vec<u8>, Vec<u8>),
}

impl SocksAuth {
    pub fn validate(&self, version: SocksVersion) -> bool {
        match self {
            SocksAuth::NoAuth => true,
            SocksAuth::Socks4(d) => version == SocksVersion::V4 && !d.contains(&0),
            SocksAuth::Username(u, p) => {
                version == SocksVersion::V5
                    && u.len() <= u8::MAX as usize
                    && p.len() <= u8::MAX as usize
            }
        }
    }

    pub fn size(&self) -> usize {
        match self {
            SocksAuth::NoAuth => 0,
            SocksAuth::Socks4(d) => d.len(),
            SocksAuth::Username(u, p) => u.len() + p.len(),
        }
    }
}

impl std::fmt::Display for SocksAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAuth => write!(f, "noauth"),
            Self::Socks4(d) => write!(f, "socks4 auth {}", String::from_utf8_lossy(&d)),
            Self::Username(user, pass) => write!(
                f,
                "username: {} password: {}",
                String::from_utf8_lossy(&user),
                String::from_utf8_lossy(&pass)
            ),
        }
    }
}

enum_int! {
    #[allow(non_camel_case_types)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SocksStatus(u8) {
        /// RFC 1928: "succeeded"
        SUCCEEDED = 0x00,
        /// RFC 1928: "general SOCKS server failure"
        GENERAL_FAILURE = 0x01,
        /// RFC 1928: "connection not allowable by ruleset"
        ///
        /// (This is the only occurrence of 'ruleset' or even 'rule'
        /// in RFC 1928.)
        NOT_ALLOWED = 0x02,
        /// RFC 1928: "Network unreachable"
        NETWORK_UNREACHABLE = 0x03,
        /// RFC 1928: "Host unreachable"
        HOST_UNREACHABLE = 0x04,
        /// RFC 1928: "Connection refused"
        CONNECTION_REFUSED = 0x05,
        /// RFC 1928: "TTL expired"
        ///
        /// (This is the only occurrence of 'TTL' in RFC 1928.)
        TTL_EXPIRED = 0x06,
        /// RFC 1929: "Command not supported"
        COMMAND_NOT_SUPPORTED = 0x07,
        /// RFC 1929: "Address type not supported"
        ADDRTYPE_NOT_SUPPORTED = 0x08,
    }
}

impl SocksStatus {
    /// Convert this status into a value for use with SOCKS4 or SOCKS4a.
    pub fn into_socks4_status(self) -> u8 {
        match self {
            SocksStatus::SUCCEEDED => 0x5A,
            _ => 0x5B,
        }
    }
    /// Create a status from a SOCKS4 or SOCKS4a reply code.
    pub fn from_socks4_status(status: u8) -> Self {
        match status {
            0x5A => SocksStatus::SUCCEEDED,
            0x5B => SocksStatus::GENERAL_FAILURE,
            0x5C | 0x5D => SocksStatus::NOT_ALLOWED,
            _ => SocksStatus::GENERAL_FAILURE,
        }
    }
}

impl SocksAddr {
    pub async fn read_from<S>(r: &mut S) -> Result<SocksAddr, SocksError>
    where
        S: AsyncRead + Unpin,
    {
        let atype = r.read_u8().await?;
        match atype {
            1 => {
                let mut addr = [0u8; 4];
                let _ = r.read_exact(&mut addr).await?;
                let ip4 = IpAddr::from(addr);
                Ok(SocksAddr::Socket(ip4.into()))
            }
            3 => {
                let str_len = r.read_u8().await?;
                let mut addr = vec![0u8; str_len as usize];
                let _ = r.read_exact(&mut addr).await?;
                let addr = String::from_utf8(addr)?;
                Ok(SocksAddr::Domain(addr))
            }
            4 => {
                let mut addr = [0u8; 16];
                let _ = r.read_exact(&mut addr).await?;
                let ip6 = IpAddr::from(addr);
                Ok(SocksAddr::Socket(ip6.into()))
            }
            other => Err(SocksError::InvalidAddrType(other)),
        }
    }

    pub fn put_to_buf<B>(&self, buf: &mut B) -> Result<(), SocksError>
    where
        B: BufMut,
    {
        match self {
            SocksAddr::Socket(IpAddr::V4(ip)) => {
                buf.put_u8(1);
                buf.put_slice(ip.octets().as_slice());
            }
            SocksAddr::Socket(IpAddr::V6(ip)) => {
                buf.put_u8(4);
                buf.put_slice(ip.octets().as_slice());
            }
            SocksAddr::Domain(domain) => {
                if domain.len() > u8::MAX as usize {
                    return Err(SocksError::TooLongString(domain.to_owned()));
                }

                buf.put_u8(3);
                buf.put_u8(domain.len() as u8);
                buf.put(domain.as_bytes());
            }
        }

        Ok(())
    }
}
