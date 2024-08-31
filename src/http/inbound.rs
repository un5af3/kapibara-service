//! Http Proxy Inbound Service

use std::{borrow::Cow, pin::Pin, task::Poll};

use base64::{prelude::BASE64_URL_SAFE, Engine};
use bytes::Bytes;
use http::{HeaderMap, Method, Request, Response, StatusCode};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream};

use crate::{
    address::NetworkType, error::ProtocolError, Address, InboundError, InboundPacket,
    InboundResult, InboundServiceStream, InboundServiceTrait, ServiceAddress,
};

use super::{
    format_request, option::HttpInboundOption, read_request, write_response, HttpError, MAX_HEADER,
    MAX_HEADER_SIZE,
};

#[derive(Debug)]
pub struct HttpInbound {
    pub auth: Vec<Vec<u8>>,
}

impl HttpInbound {
    pub fn init(in_opt: HttpInboundOption) -> InboundResult<Self> {
        let auth: Vec<_> = in_opt
            .auth
            .into_iter()
            .map(|a| [a.user, a.pass].join(":").into_bytes())
            .collect();

        Ok(Self { auth })
    }

    fn verify_auth(&self, req: &Request<()>) -> InboundResult<Vec<u8>> {
        let auth_val = req
            .headers()
            .get("Proxy-Authorization")
            .ok_or(ProtocolError::Http(HttpError::InvalidAuth))?;

        if auth_val.as_bytes().starts_with(b"Basic ") {
            let auth = BASE64_URL_SAFE
                .decode(&auth_val.as_bytes()[6..])
                .map_err(|_| {
                    InboundError::Handshake(ProtocolError::Http(HttpError::InvalidAuth))
                })?;
            if self.auth.contains(&auth) {
                return Ok(auth);
            }
        }

        Err(InboundError::Handshake(ProtocolError::Http(
            HttpError::InvalidAuth,
        )))
    }
}

impl<S> InboundServiceTrait<S> for HttpInbound
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    type Stream = HttpInboundStream<S>;

    async fn handshake(&self, stream: S) -> InboundResult<(Self::Stream, InboundPacket)> {
        let mut stream = BufStream::new(stream);
        let mut req = read_request(&mut stream, MAX_HEADER, MAX_HEADER_SIZE)
            .await
            .map_err(|e| ProtocolError::Http(e))?;

        if !self.auth.is_empty() {
            match self.verify_auth(&req) {
                Ok(_auth) => {}
                Err(err) => {
                    let resp = Response::builder()
                        .version(req.version())
                        .status(StatusCode::PROXY_AUTHENTICATION_REQUIRED)
                        .body(())
                        .unwrap();
                    let _ = write_response(&resp, &mut stream, None).await;
                    let _ = stream.flush().await?;
                    return Err(err);
                }
            }
        }

        let port = req.uri().port_u16().unwrap_or(80);
        let addr = req
            .uri()
            .host()
            .ok_or(ProtocolError::Http(HttpError::InvalidRequest))?;

        let in_pac = InboundPacket {
            typ: NetworkType::Tcp,
            dest: ServiceAddress {
                addr: addr.parse::<Address>()?,
                port,
            },
            detail: Cow::Borrowed(""),
        };

        if req.method() == Method::CONNECT {
            let resp = Response::builder()
                .version(req.version())
                .status(StatusCode::OK)
                .body(())
                .unwrap();
            let _ = write_response(&resp, &mut stream, Some("Connection established"))
                .await
                .map_err(|e| ProtocolError::Http(e))?;
            let _ = stream.flush().await?;

            let stream = HttpInboundStream::Raw(stream);

            return Ok((stream, in_pac));
        } else {
            if req.uri().scheme().is_none() || req.uri().authority().is_none() {
                let resp = Response::builder()
                    .version(req.version())
                    .status(StatusCode::BAD_REQUEST)
                    .body(())
                    .unwrap();
                let _ = write_response(&resp, &mut stream, None).await;
                let _ = stream.flush().await?;

                return Err(ProtocolError::Http(HttpError::InvalidHost).into());
            }

            remove_hop_by_hop_headers(req.headers_mut());

            let req_data = Bytes::from(format_request(&req).map_err(|e| ProtocolError::Http(e))?);
            let stream = HttpPlainStream {
                inner: stream,
                data: Some(req_data),
            };

            let stream = HttpInboundStream::Plain(stream);

            Ok((stream, in_pac))
        }
    }
}

fn remove_hop_by_hop_headers(header: &mut HeaderMap) {
    // Strip hop-by-hop header based on RFC:
    // http://www.w3.org/Protocols/rfc2616/rfc2616-sec13.html#sec13.5.1
    // https://www.mnot.net/blog/2011/07/11/what_proxies_must_do

    header.remove("Proxy-Connection");
    header.remove("Proxy-Authenticate");
    header.remove("Proxy-Authorization");
    header.remove("TE");
    header.remove("Trailers");
    header.remove("Transfer-Encoding");
    header.remove("Upgrade");

    let connections = header.remove("Connection");
    if connections.is_none() {
        return;
    }

    connections
        .unwrap()
        .as_bytes()
        .split(|c| *c == b',')
        .for_each(|key| {
            let key_str = String::from_utf8_lossy(key);
            header.remove(key_str.trim());
        });
}

#[derive(Debug)]
pub enum HttpInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    Raw(BufStream<S>),
    Plain(HttpPlainStream<BufStream<S>>),
}

impl<S> From<HttpInboundStream<S>> for InboundServiceStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    fn from(value: HttpInboundStream<S>) -> Self {
        Self::Http(value)
    }
}

impl<S> AsyncRead for HttpInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Raw(s) => Pin::new(s).poll_read(cx, buf),
            Self::Plain(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<S> AsyncWrite for HttpInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            Self::Raw(s) => Pin::new(s).poll_write(cx, buf),
            Self::Plain(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Raw(s) => Pin::new(s).poll_flush(cx),
            Self::Plain(s) => Pin::new(s).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Raw(s) => Pin::new(s).poll_shutdown(cx),
            Self::Plain(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[derive(Debug)]
pub struct HttpPlainStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    inner: S,
    data: Option<Bytes>,
}

impl<S> AsyncRead for HttpPlainStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if let Some(mut data) = this.data.take() {
            if buf.remaining() < data.len() {
                buf.put_slice(&data.split_to(buf.remaining())[..]);
                this.data = Some(data);
            } else {
                buf.put_slice(&data[..]);
            }

            return Ok(()).into();
        }

        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for HttpPlainStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    #[inline]
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

    use super::*;

    use crate::http::{option::HttpAuthOption, HttpInboundOption};

    #[tokio::test]
    async fn test_http_proxy() {
        let opt = HttpInboundOption {
            auth: vec![HttpAuthOption {
                user: "test".into(),
                pass: "test".into(),
            }],
        };
        let inbound = HttpInbound::init(opt).unwrap();
        let mut data =
            b"CONNECT bing.com HTTP/1.1\r\nHost: bing.com\r\nContent-Type: json\r\n".to_vec();
        let test = format!("{}: {}\r\n", "t".repeat(50), "t".repeat(50)).repeat(60);
        data.extend(test.as_bytes());
        data.extend(
            format!(
                "Proxy-Authorization: Basic {}\r\n",
                BASE64_URL_SAFE.encode(b"test:test")
            )
            .as_bytes(),
        );
        data.extend(b"\r\ntest");

        let res = inbound.handshake(Cursor::new(data)).await;
        if let Err(err) = res {
            println!("{}", err);
        }
    }
}
