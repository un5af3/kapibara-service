//! Direct Outbound Service

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, UdpSocket},
};

use crate::{
    address::NetworkType, Address, OutboundError, OutboundPacket, OutboundResult,
    OutboundServiceStream, OutboundServiceTrait,
};

#[derive(Debug, Clone, Copy)]
pub struct DirectOutbound;

impl<S> OutboundServiceTrait<S> for DirectOutbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = OutboundServiceStream<S>;

    async fn handshake(&self, _stream: S, packet: OutboundPacket) -> OutboundResult<Self::Stream> {
        let addr = match packet.dest.addr {
            Address::Domain(_) => return Err(OutboundError::Unresolved),
            Address::Socket(ip) => SocketAddr::new(ip, packet.dest.port),
        };

        match packet.typ {
            NetworkType::Tcp => {
                let stream = TcpStream::connect(addr).await?;
                Ok(OutboundServiceStream::Direct(DirectStream::Tcp(stream)))
            }
            NetworkType::Udp => {
                let stream = UdpStream::connect(addr).await?;
                Ok(OutboundServiceStream::Direct(DirectStream::Udp(stream)))
            }
        }
    }
}

#[derive(Debug)]
pub enum DirectStream {
    Tcp(TcpStream),
    Udp(UdpStream),
}

impl AsyncRead for DirectStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            Self::Udp(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for DirectStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            Self::Udp(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_flush(cx),
            Self::Udp(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            Self::Udp(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[derive(Debug)]
pub struct UdpStream {
    socket: UdpSocket,
}

impl UdpStream {
    pub async fn connect(addr: SocketAddr) -> std::io::Result<Self> {
        let local_addr = if addr.is_ipv4() {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
        };

        let socket = UdpSocket::bind(local_addr).await?;
        socket.connect(addr).await?;

        Ok(Self { socket })
    }
}

impl AsyncRead for UdpStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_mut().socket.poll_recv(cx, buf)
    }
}

impl AsyncWrite for UdpStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.get_mut().socket.poll_send(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Ok(()).into()
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Ok(()).into()
    }
}
