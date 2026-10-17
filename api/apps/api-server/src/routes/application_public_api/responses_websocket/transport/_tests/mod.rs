use crate::transport::WebSocketUpgrade;
use axum::{extract::ws::Message as ServerMessage, routing::get, Router};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
use tokio_tungstenite_deflate::{
    client_async_with_config,
    tungstenite::{
        client::IntoClientRequest,
        extensions::compression::deflate::DeflateConfig,
        protocol::{frame::coding::CloseCode, CloseFrame, WebSocketConfig},
        Message,
    },
};

#[derive(Clone, Default)]
struct Trace {
    read: Arc<Mutex<Vec<u8>>>,
    written: Arc<Mutex<Vec<u8>>>,
}

struct TracedStream {
    stream: TcpStream,
    trace: Trace,
}
impl AsyncRead for TracedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) {
            self.trace
                .read
                .lock()
                .unwrap()
                .extend_from_slice(&buf.filled()[before..]);
        }
        result
    }
}
impl AsyncWrite for TracedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_write(cx, bytes);
        if let Poll::Ready(Ok(size)) = result {
            self.trace
                .written
                .lock()
                .unwrap()
                .extend_from_slice(&bytes[..size]);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

// Abort the fixture even if a failed assertion unwinds its test.
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn image_request() -> String {
    let mut state = 1_u32;
    let bytes = (0..2 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    serde_json::json!({"type":"response.create","input":[{"role":"user","content":[
        {"type":"input_text","text":"原样保留\n\u{0} 🦀"},
        {"type":"input_image","image_url":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}
    ]}]}).to_string()
}

async fn roundtrip(compression: bool, declined_offer: bool) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let text = image_request();
    let expected = text.clone();
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel();
    let observed_tx = Arc::new(Mutex::new(Some(observed_tx)));
    let router = Router::new().route("/ws", get(move |upgrade: WebSocketUpgrade| {
        let expected = expected.clone();
        let observed_tx = observed_tx.clone();
        async move { upgrade.on_upgrade(move |mut socket| async move {
            // Two complete messages exercise takeover state as well as the first frame.
            for _ in 0..2 {
                let message = socket.next().await.unwrap().unwrap();
                assert!(matches!(&message, ServerMessage::Text(value) if value.as_str()==expected));
                socket.send(message).await.unwrap();
            }
            assert!(matches!(socket.next().await.unwrap().unwrap(), ServerMessage::Ping(value) if value.as_ref()==b"probe"));
            socket.send(ServerMessage::Pong(b"probe".as_slice().into())).await.unwrap();
            let message = socket.next().await.unwrap().unwrap();
            assert!(matches!(&message, ServerMessage::Close(Some(frame)) if frame.code==1000 && frame.reason.as_str()=="正常完成"));
            // The codec queues the peer Close acknowledgement; flush it before release.
            socket.flush().await.unwrap();
            observed_tx.lock().unwrap().take().unwrap().send(()).unwrap();
        }).await }
    }));
    let _server = Server(tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap()
    }));
    let trace = Trace::default();
    let io = TracedStream {
        stream: TcpStream::connect(address).await.unwrap(),
        trace: trace.clone(),
    };
    let mut config = WebSocketConfig::default();
    if compression {
        config.extensions.permessage_deflate = Some(DeflateConfig::default());
    }
    let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
    if declined_offer {
        request.headers_mut().insert(
            "sec-websocket-extensions",
            "permessage-deflate; unsupported_parameter".parse().unwrap(),
        );
    }
    let (mut client, response) = client_async_with_config(request, io, Some(config))
        .await
        .unwrap();
    assert_eq!(response.status(), 101);
    assert_eq!(
        response.headers().get("sec-websocket-extensions").is_some(),
        compression
    );
    let handshake = String::from_utf8(trace.read.lock().unwrap().clone()).unwrap();
    assert_eq!(
        handshake.matches("HTTP/1.1 101").count(),
        1,
        "Hyper writes exactly one upgrade response"
    );
    trace.read.lock().unwrap().clear();
    trace.written.lock().unwrap().clear();
    for _ in 0..2 {
        client
            .send(Message::Text(text.clone().into()))
            .await
            .unwrap();
        assert_eq!(
            client
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_text()
                .unwrap()
                .as_str(),
            text
        );
    }
    client
        .send(Message::Ping(b"probe".as_slice().into()))
        .await
        .unwrap();
    assert_eq!(
        client.next().await.unwrap().unwrap(),
        Message::Pong(b"probe".as_slice().into())
    );
    client
        .close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "正常完成".into(),
        }))
        .await
        .unwrap();
    assert!(
        matches!(client.next().await.unwrap().unwrap(), Message::Close(Some(frame)) if frame.code==CloseCode::Normal && frame.reason.as_str()=="正常完成")
    );
    observed_rx.await.unwrap();
    for bytes in [&trace.read, &trace.written] {
        let bytes = bytes.lock().unwrap();
        assert_eq!(
            bytes[0] & 0x40 != 0,
            compression,
            "actual RSV1 agrees with negotiated compression"
        );
        if compression {
            assert!(bytes.len() < text.len() * 2, "wire byte count is reduced");
        } else {
            assert!(bytes.len() >= text.len() * 2);
        }
    }
}

#[tokio::test]
async fn codex_compression_offer_has_lossless_bidirectional_wire_and_control_frames() {
    tokio::time::timeout(Duration::from_secs(15), roundtrip(true, false))
        .await
        .unwrap();
}

#[tokio::test]
async fn clients_without_compression_keep_the_existing_protocol() {
    tokio::time::timeout(Duration::from_secs(15), roundtrip(false, false))
        .await
        .unwrap();
}

#[tokio::test]
async fn unsupported_deflate_offer_is_declined_without_breaking_plain_messages() {
    tokio::time::timeout(Duration::from_secs(15), roundtrip(false, true))
        .await
        .unwrap();
}

#[tokio::test]
async fn responses_accepts_plain_and_compressed_messages_larger_than_sixteen_mebibytes() {
    tokio::time::timeout(Duration::from_secs(15), async {
        for compression in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let text = "x".repeat(16 * 1024 * 1024 + 1024);
            let expected = text.clone();
            let (observed_tx, observed_rx) = tokio::sync::oneshot::channel();
            let observed_tx = Arc::new(Mutex::new(Some(observed_tx)));
            let router = Router::new().route(
                "/ws",
                get(move |upgrade: WebSocketUpgrade| {
                    let observed_tx = observed_tx.clone();
                    let expected = expected.clone();
                    async move {
                        upgrade
                            .on_upgrade(move |mut socket| async move {
                                let result = socket.next().await.unwrap();
                                let intact = matches!(
                                    result,
                                    Ok(ServerMessage::Text(text)) if text.as_str() == expected
                                );
                                observed_tx
                                    .lock()
                                    .unwrap()
                                    .take()
                                    .unwrap()
                                    .send(intact)
                                    .unwrap();
                            })
                            .await
                    }
                }),
            );
            let _server = Server(tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap()
            }));
            let io = TcpStream::connect(address).await.unwrap();
            let mut config = WebSocketConfig::default();
            if compression {
                config.extensions.permessage_deflate = Some(DeflateConfig::default());
            }
            let (mut client, response) =
                client_async_with_config(format!("ws://{address}/ws"), io, Some(config))
                    .await
                    .unwrap();
            assert_eq!(
                response.headers().contains_key("sec-websocket-extensions"),
                compression
            );
            client.send(Message::Text(text.into())).await.unwrap();
            assert!(
                observed_rx.await.unwrap(),
                "the complete message must reach the consumer"
            );
        }
    })
    .await
    .unwrap();
}
