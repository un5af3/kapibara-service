//! vless protocol

use std::{
    io::{Cursor, Read},
    net::IpAddr,
};

use bytes::{Buf, BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    impl_addr_type, read_varint, variant_len, write_varint, AddrType, AddrTypeConvert, Address,
    ServiceAddress,
};

use super::VlessError;

const VERSION: u8 = 0;

pub const COMMAND_TCP: u8 = 1;
pub const COMMAND_UDP: u8 = 2;
pub const COMMAND_MUX: u8 = 3;

impl_addr_type! {
    pub enum VlessAddrType {
        Ipv4 = 1,
        Ipv6 = 3,
        Fqdn = 2,
        Unknown = 255,
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Request {
    pub uuid: uuid::Uuid,
    pub flow: Option<String>,
    pub command: u8,
    pub destination: Option<ServiceAddress>,
}

impl Request {
    pub fn len(&self) -> usize {
        let mut request_len: usize = 1 + 16 + 1; // version + uuid + addons length

        if let Some(ref flow) = self.flow {
            request_len += 1 + variant_len(flow.len() as u64) + flow.len(); // header + flow_len + flow
        }

        request_len += 1; // command
        if self.command != COMMAND_MUX {
            if let Some(ref dest) = self.destination {
                request_len += match &dest.addr {
                    Address::Domain(s) => 4 + s.len(), // af + str_len + str + port
                    Address::Socket(ip) => match ip {
                        IpAddr::V4(_) => 7,  // af + ipv4 + port
                        IpAddr::V6(_) => 19, // af + ipv6 + port
                    },
                };
            }
        }
        request_len
    }

    pub async fn read<R>(stream: &mut R) -> Result<Request, VlessError>
    where
        R: AsyncRead + Unpin,
    {
        let version = stream.read_u8().await?;
        if version != VERSION {
            return Err(VlessError::InvalidVersion(version).into());
        }

        let mut uuid = [0u8; 16];
        let _ = stream.read_exact(&mut uuid).await?;

        let mut flow = None;
        let addons_len = stream.read_u8().await?;
        if addons_len > 0 {
            let mut addons_bytes = vec![0u8; addons_len as usize];
            let _ = stream.read_exact(&mut addons_bytes).await?;
            let addons = Addons::parse(&addons_bytes)?;
            flow = addons.flow;
        }

        let mut destination = None;
        let command = stream.read_u8().await?;
        match command {
            COMMAND_TCP | COMMAND_UDP => {
                let port = stream.read_u16().await?;
                let addr = Address::read::<R, VlessAddrType>(stream).await?;
                destination = Some(ServiceAddress::new(addr, port));
            }
            COMMAND_MUX => {}
            other => return Err(VlessError::InvalidCommand(other)),
        }

        Ok(Request {
            uuid: uuid::Uuid::from_bytes(uuid),
            flow,
            command,
            destination,
        })
    }

    pub async fn write<W>(&self, writer: &mut W, payload: Option<&[u8]>) -> Result<(), VlessError>
    where
        W: AsyncWrite + Unpin,
    {
        let _ = writer.write_all(&self.into_buf(payload)?).await?;
        let _ = writer.flush().await?;

        Ok(())
    }

    pub fn into_buf(&self, payload: Option<&[u8]>) -> Result<Vec<u8>, VlessError> {
        let request_len = self.len() + payload.map_or(0, |p| p.len());

        let mut buf = BytesMut::with_capacity(request_len);
        buf.put_u8(VERSION);
        buf.put(self.uuid.as_ref());

        match self.flow {
            Some(ref flow) => {
                buf.put_u8(flow.len() as u8);
                buf.put_u8(10);
                write_varint(&mut buf, flow.len() as u64);
                buf.put(flow.as_bytes());
            }
            None => buf.put_u8(0),
        }

        buf.put_u8(self.command);

        match self.command {
            COMMAND_TCP | COMMAND_UDP => {
                if let Some(ref ap) = self.destination {
                    buf.put_u16(ap.port);
                    ap.addr.put_to_buf::<BytesMut, VlessAddrType>(&mut buf)?;
                }
            }
            COMMAND_MUX => {}
            other => return Err(VlessError::InvalidCommand(other)),
        }

        if let Some(p) = payload {
            buf.put(p);
        }

        Ok(buf.to_vec())
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub flow: Option<String>,
}

impl Response {
    pub fn len(&self) -> usize {
        let mut resp_len: usize = 2; // version + addons_header

        if let Some(ref flow) = self.flow {
            resp_len += 1 + variant_len(flow.len() as u64) + flow.len(); // header + flow_len + flow
        }

        resp_len
    }

    pub async fn read<R>(stream: &mut R) -> Result<Response, VlessError>
    where
        R: AsyncRead + Unpin,
    {
        let version = stream.read_u8().await?;
        if version != VERSION {
            return Err(VlessError::InvalidVersion(version).into());
        }

        let mut resp = Response { flow: None };
        let addons_len = stream.read_u8().await?;
        if addons_len > 0 {
            let mut addons_bytes = vec![0u8; addons_len as usize];
            let _ = stream.read_exact(&mut addons_bytes).await?;
            let addons = Addons::parse(&addons_bytes)?;
            resp.flow = addons.flow;
        }

        Ok(resp)
    }

    pub fn read_buf(buf: &[u8]) -> Result<Response, VlessError> {
        if buf.len() < 2 {
            return Err(VlessError::UnknownVersion);
        }

        let version = buf[0];
        if version != VERSION {
            return Err(VlessError::InvalidVersion(version).into());
        }

        let mut resp = Response { flow: None };
        let addons_len = buf[1];
        if addons_len > 0 {
            if buf.len() - 2 > addons_len as usize {
                return Err(VlessError::InvalidHeader(addons_len));
            }

            let addons = Addons::parse(&buf[..addons_len as usize])?;
            resp.flow = addons.flow;
        }

        Ok(resp)
    }

    pub async fn write<W>(&self, writer: &mut W, payload: Option<&[u8]>) -> Result<(), VlessError>
    where
        W: AsyncWrite + Unpin,
    {
        let _ = writer.write_all(&self.into_buf(payload)?).await?;
        let _ = writer.flush().await?;

        Ok(())
    }

    pub fn into_buf(&self, payload: Option<&[u8]>) -> Result<Vec<u8>, VlessError> {
        let resp_len = self.len() + payload.map_or(0, |p| p.len());

        let mut buf = BytesMut::with_capacity(resp_len);

        buf.put_u8(VERSION);

        match self.flow {
            Some(ref flow) => {
                buf.put_u8(flow.len() as u8);
                buf.put_u8(10);
                write_varint(&mut buf, flow.len() as u64);
                buf.put(flow.as_bytes());
            }
            None => buf.put_u8(0),
        }

        if let Some(p) = payload {
            buf.put(p);
        }

        Ok(buf.to_vec())
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct Addons {
    flow: Option<String>,
    seed: Option<String>,
}

impl Addons {
    pub fn parse<B>(b: B) -> Result<Addons, VlessError>
    where
        B: AsRef<[u8]>,
    {
        let mut buf = Cursor::new(b);

        let proto_header = buf.get_u8();
        if proto_header != 10 {
            return Err(VlessError::InvalidHeader(proto_header));
        }

        let flow_len = match read_varint(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(Addons::default());
                } else {
                    return Err(e.into());
                }
            }
        };

        let mut flow_bytes = vec![0u8; flow_len as usize];
        let _ = buf.read_exact(&mut flow_bytes)?;
        let flow = Some(String::from_utf8(flow_bytes)?);

        let seed_len = match read_varint(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(Addons { flow, seed: None });
                } else {
                    return Err(e.into());
                }
            }
        };

        let mut seed_bytes = vec![0u8; seed_len as usize];
        let _ = buf.read_exact(&mut seed_bytes)?;
        let seed = Some(String::from_utf8(seed_bytes)?);

        Ok(Addons { flow, seed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vless_protocol() -> Result<(), VlessError> {
        let req1 = Request {
            flow: None,
            uuid: uuid::Uuid::from_bytes([
                252, 66, 254, 52, 226, 103, 76, 105, 136, 97, 43, 196, 25, 5, 117, 25,
            ]),
            destination: Some(ServiceAddress::new(
                Address::Socket("127.0.0.1".parse().unwrap()),
                8888,
            )),
            command: COMMAND_TCP,
        };

        let mut buf1 = Cursor::new(vec![]);
        let _ = req1.write(&mut buf1, Some("test".as_bytes())).await?;
        buf1.set_position(0);
        println!("{:?}", buf1);

        let req2 = Request::read(&mut buf1).await?;

        assert_eq!(req1, req2);
        assert_eq!(buf1.chunk(), "test".as_bytes());

        Ok(())
    }
}
