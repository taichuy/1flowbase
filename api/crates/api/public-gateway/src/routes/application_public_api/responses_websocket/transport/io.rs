//! Adapt an already-parsed HTTP upgrade to the codec's public server handshake.
//! The prelude contains only handshake headers. The codec's response is returned
//! by Hyper, so it is not written a second time to the upgraded connection.
use axum::http::HeaderMap;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(super) enum UpgradeIo {
    Handshake { prelude: Vec<u8>, offset: usize },
    Connected(TokioIo<Upgraded>),
}

impl UpgradeIo {
    pub(super) fn handshake(headers: &HeaderMap) -> Self {
        // The route and authentication have already been resolved by the host.
        // Only RFC 6455 headers are replayed locally for library negotiation.
        let mut prelude = b"GET / HTTP/1.1\r\n".to_vec();
        for (name, value) in headers {
            prelude.extend_from_slice(name.as_str().as_bytes());
            prelude.extend_from_slice(b": ");
            prelude.extend_from_slice(value.as_bytes());
            prelude.extend_from_slice(b"\r\n");
        }
        prelude.extend_from_slice(b"\r\n");
        Self::Handshake { prelude, offset: 0 }
    }

    pub(super) fn attach(&mut self, upgraded: Upgraded) {
        // Replacing the prelude frees its headers before application frames arrive.
        *self = Self::Connected(TokioIo::new(upgraded));
    }
}

impl AsyncRead for UpgradeIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Handshake { prelude, offset } => {
                let size = buf.remaining().min(prelude.len() - *offset);
                buf.put_slice(&prelude[*offset..*offset + size]);
                *offset += size;
                Poll::Ready(Ok(()))
            }
            Self::Connected(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for UpgradeIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Handshake { .. } => Poll::Ready(Ok(bytes.len())),
            Self::Connected(stream) => Pin::new(stream).poll_write(cx, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Handshake { .. } => Poll::Ready(Ok(())),
            Self::Connected(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Handshake { .. } => Poll::Ready(Ok(())),
            Self::Connected(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
