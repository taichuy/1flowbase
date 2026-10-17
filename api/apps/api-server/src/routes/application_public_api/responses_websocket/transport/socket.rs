//! Keep the actor's existing message contract across both transport engines.
use super::io::UpgradeIo;
use axum::{
    body::Bytes,
    extract::ws::{CloseFrame, Message, WebSocket},
    Error,
};
use futures_util::{Sink, Stream};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio_tungstenite_deflate::{
    tungstenite::{self as codec, Message as CodecMessage},
    WebSocketStream,
};

pub(crate) struct Socket(Inner);
enum Inner {
    Axum(Box<WebSocket>),
    Codec(Box<WebSocketStream<UpgradeIo>>),
}

impl Socket {
    pub(super) fn from_codec(socket: WebSocketStream<UpgradeIo>) -> Self {
        Self(Inner::Codec(Box::new(socket)))
    }
}

impl From<WebSocket> for Socket {
    fn from(socket: WebSocket) -> Self {
        Self(Inner::Axum(Box::new(socket)))
    }
}

impl Stream for Socket {
    type Item = Result<Message, Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.get_mut().0 {
            Inner::Axum(socket) => Pin::new(socket.as_mut()).poll_next(cx),
            Inner::Codec(socket) => loop {
                match std::task::ready!(Pin::new(socket.as_mut()).poll_next(cx)) {
                    Some(Ok(message)) => {
                        if let Some(message) = from_codec(message) {
                            return Poll::Ready(Some(Ok(message)));
                        }
                    }
                    Some(Err(error)) => return Poll::Ready(Some(Err(Error::new(error)))),
                    None => return Poll::Ready(None),
                }
            },
        }
    }
}

impl Sink<Message> for Socket {
    type Error = Error;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match &mut self.get_mut().0 {
            Inner::Axum(socket) => Pin::new(socket.as_mut()).poll_ready(cx),
            Inner::Codec(socket) => Pin::new(socket.as_mut()).poll_ready(cx).map_err(Error::new),
        }
    }
    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Error> {
        match &mut self.get_mut().0 {
            Inner::Axum(socket) => Pin::new(socket.as_mut()).start_send(message),
            Inner::Codec(socket) => Pin::new(socket.as_mut())
                .start_send(to_codec(message))
                .map_err(Error::new),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match &mut self.get_mut().0 {
            Inner::Axum(socket) => Pin::new(socket.as_mut()).poll_flush(cx),
            Inner::Codec(socket) => Pin::new(socket.as_mut()).poll_flush(cx).map_err(Error::new),
        }
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match &mut self.get_mut().0 {
            Inner::Axum(socket) => Pin::new(socket.as_mut()).poll_close(cx),
            Inner::Codec(socket) => Pin::new(socket.as_mut()).poll_close(cx).map_err(Error::new),
        }
    }
}

fn from_codec(message: CodecMessage) -> Option<Message> {
    Some(match message {
        CodecMessage::Text(text) => {
            Message::Text(Bytes::from(text).try_into().expect("codec validated UTF-8"))
        }
        CodecMessage::Binary(bytes) => Message::Binary(bytes),
        CodecMessage::Ping(bytes) => Message::Ping(bytes),
        CodecMessage::Pong(bytes) => Message::Pong(bytes),
        CodecMessage::Close(frame) => Message::Close(frame.map(|frame| {
            CloseFrame {
                code: frame.code.into(),
                reason: Bytes::from(frame.reason)
                    .try_into()
                    .expect("codec validated close UTF-8"),
            }
        })),
        CodecMessage::Frame(_) => return None,
    })
}

fn to_codec(message: Message) -> CodecMessage {
    match message {
        Message::Text(text) => CodecMessage::Text(
            Bytes::from(text)
                .try_into()
                .expect("Axum text is valid UTF-8"),
        ),
        Message::Binary(bytes) => CodecMessage::Binary(bytes),
        Message::Ping(bytes) => CodecMessage::Ping(bytes),
        Message::Pong(bytes) => CodecMessage::Pong(bytes),
        Message::Close(frame) => CodecMessage::Close(frame.map(|frame| {
            codec::protocol::CloseFrame {
                code: frame.code.into(),
                reason: Bytes::from(frame.reason)
                    .try_into()
                    .expect("Axum close text is valid UTF-8"),
            }
        })),
    }
}
