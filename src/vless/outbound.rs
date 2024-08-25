use std::{pin::Pin, str::FromStr, task::Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use crate::{
    address::NetworkType, OutboundError, OutboundPacket, OutboundResult, OutboundServiceStream,
    OutboundServiceTrait,
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
    type Stream = VlessOutboundStream<S>;

    async fn handshake(
        &self,
        mut stream: S,
        packet: OutboundPacket,
    ) -> OutboundResult<Self::Stream> {
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

        let _ = req
            .write(&mut stream, None)
            .await
            .map_err(|e| OutboundError::Handshake(e.into()))?;

        Ok(VlessOutboundStream::new(stream))
    }
}

#[derive(Debug)]
pub struct VlessOutboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    inner: S,
    check_resp: bool,
}

impl<S> VlessOutboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            check_resp: true,
        }
    }
}

impl<S> From<VlessOutboundStream<S>> for OutboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn from(value: VlessOutboundStream<S>) -> Self {
        OutboundServiceStream::Vless(value)
    }
}

impl<S> AsyncRead for VlessOutboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Ready(Ok(_)) => {
                if this.check_resp {
                    let resp =
                        Response::read_buf(buf.filled()).map_err(|e| std::io::Error::other(e))?;
                    let data = buf.filled()[resp.len()..].to_vec();
                    buf.clear();
                    buf.put_slice(&data);
                    this.check_resp = false;
                }
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl<S> AsyncWrite for VlessOutboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
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
