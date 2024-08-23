//! Address

use std::{fmt::Display, net::IpAddr, str::FromStr};

use bytes::BufMut;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::AddressError;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NetworkType {
    Tcp,
    Udp,
}

impl Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp => write!(f, "tcp"),
            Self::Udp => write!(f, "udp"),
        }
    }
}

pub trait AddrTypeConvert {
    fn into_u8(af: AddrType) -> u8;
    fn from_u8(val: u8) -> AddrType;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AddrType {
    Ipv4,
    Fqdn,
    Ipv6,
    Unknown,
}

#[macro_export]
macro_rules! impl_addr_type {
    {
        $(#[$meta:meta])*
        $v:vis enum $name:ident {
            $(#[$ipv4_meta:meta])*
            Ipv4 = $ipv4:literal,
            $(#[$ipv6_meta:meta])*
            Ipv6 = $ipv6:literal,
            $(#[$fqdn_meta:meta])*
            Fqdn = $fqdn:literal,
            $(#[$unknown_meta:meta])*
            Unknown = $unknown:literal,
        }
    } => {
        $(#[$meta])*
        $v enum $name {
            $(#[$ipv4_meta])*
            Ipv4 = $ipv4,
            $(#[$ipv6_meta])*
            Ipv6 = $ipv6,
            $(#[$fqdn_meta])*
            Fqdn = $fqdn,
            $(#[$unknown_meta])*
            Unknown = $unknown,
        }

        impl AddrTypeConvert for $name {
            fn from_u8(value: u8) -> AddrType {
                match value {
                    $ipv4 => AddrType::Ipv4,
                    $ipv6 => AddrType::Ipv6,
                    $fqdn => AddrType::Fqdn,
                    _ => AddrType::Unknown,
                }
            }

            fn into_u8(af: AddrType) -> u8 {
                match af {
                    AddrType::Ipv4 => $ipv4,
                    AddrType::Ipv6 => $ipv6,
                    AddrType::Fqdn => $fqdn,
                    AddrType::Unknown => $unknown,
                }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAddress {
    pub addr: Address,
    pub port: u16,
}

impl ServiceAddress {
    pub fn new(addr: Address, port: u16) -> Self {
        Self { addr, port }
    }
}

impl Display for ServiceAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Socket(IpAddr),
    Domain(String),
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Address::Domain(s) => write!(f, "{}", s),
            Address::Socket(s) => write!(f, "{}", s),
        }
    }
}

impl Address {
    pub fn is_ip(&self) -> bool {
        matches!(self, Self::Socket(_))
    }

    pub async fn read<R, C>(reader: &mut R) -> Result<Address, AddressError>
    where
        R: AsyncRead + Unpin,
        C: AddrTypeConvert,
    {
        let af = C::from_u8(reader.read_u8().await?);
        match af {
            AddrType::Ipv4 => {
                let mut addr = [0u8; 4];
                let _ = reader.read_exact(&mut addr).await?;
                let ip = IpAddr::from(addr);
                Ok(Address::Socket(ip.into()))
            }
            AddrType::Ipv6 => {
                let mut addr = [0u8; 16];
                let _ = reader.read_exact(&mut addr).await?;
                let ip = IpAddr::from(addr);
                Ok(Address::Socket(ip.into()))
            }
            AddrType::Fqdn => {
                let str_len = reader.read_u8().await?;
                let mut addr = vec![0u8; str_len as usize];
                let _ = reader.read_exact(&mut addr).await?;
                let addr = String::from_utf8(addr)?;
                Ok(Address::Domain(addr))
            }
            AddrType::Unknown => return Err(AddressError::InvalidAddrType),
        }
    }

    pub fn put_to_buf<B, C>(&self, buf: &mut B) -> Result<(), AddressError>
    where
        B: BufMut,
        C: AddrTypeConvert,
    {
        match self {
            Address::Domain(s) => {
                if s.len() > u8::MAX as usize {
                    return Err(AddressError::InvalidAddress(s.to_owned()));
                }

                buf.put_u8(C::into_u8(AddrType::Fqdn));
                buf.put_u8(s.len() as u8);
                buf.put(s.as_bytes());
            }
            Address::Socket(IpAddr::V4(ip)) => {
                buf.put_u8(C::into_u8(AddrType::Ipv4));
                buf.put(ip.octets().as_ref());
            }
            Address::Socket(IpAddr::V6(ip)) => {
                buf.put_u8(C::into_u8(AddrType::Ipv6));
                buf.put(ip.octets().as_ref());
            }
        }

        Ok(())
    }
}

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match IpAddr::from_str(s) {
            Ok(ip) => Ok(Self::Socket(ip)),
            Err(_) => Ok(Self::Domain(s.to_string())),
        }
    }
}

impl<T: AsRef<str> + ToString> From<T> for Address {
    fn from(s: T) -> Self {
        match IpAddr::from_str(s.as_ref()) {
            Ok(ip) => Self::Socket(ip),
            Err(_) => Self::Domain(s.to_string()),
        }
    }
}
