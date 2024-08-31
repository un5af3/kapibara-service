//! Http Proxy oubound

use base64::{prelude::BASE64_URL_SAFE, Engine};
use http::{Method, Request, Uri};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream};

use crate::{
    address::NetworkType, error::ProtocolError, OutboundError, OutboundPacket, OutboundResult,
    OutboundServiceTrait,
};

use super::{
    read_response, write_request, HttpError, HttpOutboundOption, MAX_HEADER, MAX_HEADER_SIZE,
};

#[derive(Debug)]
pub struct HttpOutbound {
    auth: Option<String>,
}

impl HttpOutbound {
    pub fn init(option: HttpOutboundOption) -> OutboundResult<Self> {
        let auth = option.auth.map(|a| {
            let s = a.user + ":" + &a.pass;
            format!("Basic {}", BASE64_URL_SAFE.encode(s))
        });

        Ok(Self { auth })
    }
}

impl<S> OutboundServiceTrait<S> for HttpOutbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = BufStream<S>;

    async fn handshake(&self, stream: S, packet: OutboundPacket) -> OutboundResult<Self::Stream> {
        if packet.typ != NetworkType::Tcp {
            return Err(OutboundError::InvalidType(packet.typ));
        }

        let mut stream = BufStream::new(stream);

        let host = packet.dest.to_string();
        let uri = Uri::builder()
            .authority(host.as_str())
            .build()
            .map_err(|e| ProtocolError::Http(e.into()))?;
        let mut builder = Request::builder()
            .method(Method::CONNECT)
            .uri(uri)
            .header("Host", host)
            .header("Proxy-Connection", "Keep-Alive");

        if let Some(ref auth) = self.auth {
            builder = builder.header("Proxy-Authorization", auth);
        }

        let req = builder
            .body(())
            .map_err(|e| ProtocolError::Http(e.into()))?;

        let _ = write_request(&req, &mut stream)
            .await
            .map_err(|e| ProtocolError::Http(e));
        let _ = stream.flush().await?;

        let resp = read_response(&mut stream, MAX_HEADER, MAX_HEADER_SIZE)
            .await
            .map_err(|e| ProtocolError::Http(e))?;

        if !resp.status().is_success() {
            return Err(ProtocolError::Http(HttpError::InvalidStatusCode(resp.status())).into());
        }

        Ok(stream)
    }
}
