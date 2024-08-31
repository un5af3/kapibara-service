//! Stream utils

use std::{pin::Pin, task::Poll};

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct CachedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    cache: Option<Bytes>,
    inner: S,
}

impl<S> CachedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    pub fn new(inner: S, cache: Option<Bytes>) -> Self {
        Self { cache, inner }
    }
}

impl<S> AsyncRead for CachedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        if let Some(mut cache) = this.cache.take() {
            if buf.remaining() < cache.len() {
                buf.put_slice(&cache.split_to(buf.remaining())[..]);
                this.cache = Some(cache);
            } else {
                buf.put_slice(&cache[..]);
            }

            return Ok(()).into();
        }

        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for CachedStream<S>
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
