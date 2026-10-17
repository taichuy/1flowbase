//! Responses connection adapter: HTTP ownership stays in Axum/Hyper, RFC 7692 in
//! the same pinned Tungstenite codec used by Codex. No transport flags enter AI Native.
use axum::{
    body::Body,
    extract::{ws::WebSocketUpgrade as AxumUpgrade, FromRequestParts},
    http::{header, request::Parts, HeaderMap, StatusCode, Version},
    response::{IntoResponse, Response},
};
use std::future::Future;
use tokio_tungstenite_deflate::tungstenite::{
    extensions::compression::deflate::DeflateConfig,
    handshake::server::{Callback, ErrorResponse, Request, Response as HandshakeResponse},
    protocol::WebSocketConfig,
};

mod io;
mod socket;
pub(crate) use socket::Socket;

pub(crate) struct WebSocketUpgrade {
    validated: AxumUpgrade,
    upgrade: hyper::upgrade::OnUpgrade,
    headers: HeaderMap,
    http1: bool,
    config: WebSocketConfig,
}

impl<S: Send + Sync> FromRequestParts<S> for WebSocketUpgrade {
    type Rejection = <AxumUpgrade as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Axum remains the owner of GET/CONNECT, version, Upgrade and key validation.
        // Its successful extractor consumes the OnUpgrade extension; retain a handle
        // before validation, without starting another reader of that connection.
        let upgrade = parts.extensions.get::<hyper::upgrade::OnUpgrade>().cloned();
        let validated = AxumUpgrade::from_request_parts(parts, state).await?;
        let mut headers = HeaderMap::new();
        for name in [
            header::CONNECTION,
            header::UPGRADE,
            header::SEC_WEBSOCKET_VERSION,
            header::SEC_WEBSOCKET_KEY,
            header::SEC_WEBSOCKET_EXTENSIONS,
        ] {
            for value in parts.headers.get_all(&name) {
                headers.append(name.clone(), value.clone());
            }
        }
        let mut config = WebSocketConfig::default();
        config.extensions.permessage_deflate = Some(DeflateConfig::default());
        Ok(Self {
            validated,
            upgrade: upgrade.expect("Axum validated the connection upgrade extension"),
            headers,
            http1: parts.version <= Version::HTTP_11,
            config,
        })
    }
}

// The codec owns the callback's rejection type. Capture its negotiated response
// through that typed contract instead of inferring a large error on a closure.
struct CaptureHandshake<'a>(&'a mut Option<HandshakeResponse>);

impl Callback for CaptureHandshake<'_> {
    fn on_request(
        self,
        _request: &Request,
        response: HandshakeResponse,
    ) -> Result<HandshakeResponse, ErrorResponse> {
        *self.0 = Some(response.clone());
        Ok(response)
    }
}

impl WebSocketUpgrade {
    pub(crate) fn max_message_size(mut self, size: usize) -> Self {
        self.config.max_message_size = Some(size);
        self.validated = self.validated.max_message_size(size);
        self
    }

    pub(crate) fn max_frame_size(mut self, size: usize) -> Self {
        self.config.max_frame_size = Some(size);
        self.validated = self.validated.max_frame_size(size);
        self
    }

    pub(crate) async fn on_upgrade<C, Fut>(self, callback: C) -> Response
    where
        C: FnOnce(Socket) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if !self.http1 {
            // Preserve Axum's existing extended-CONNECT transport. The public
            // Responses route is GET; Codex's HTTP/1.1 path negotiates below.
            return self.validated.on_upgrade(|socket| callback(socket.into()));
        }

        let mut response = None;
        let socket = tokio_tungstenite_deflate::accept_hdr_async_with_config(
            io::UpgradeIo::handshake(&self.headers),
            CaptureHandshake(&mut response),
            Some(self.config),
        )
        .await;
        let mut socket = match socket {
            Ok(socket) => socket,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "invalid WebSocket upgrade").into_response()
            }
        };
        let response = response.expect("completed codec handshake produces its response");
        tokio::spawn(async move {
            match self.upgrade.await {
                Ok(upgraded) => {
                    socket.get_mut().attach(upgraded);
                    callback(Socket::from_codec(socket)).await;
                }
                Err(error) => tracing::debug!(%error, "Responses WebSocket upgrade failed"),
            }
        });
        response.map(|()| Body::empty())
    }
}
