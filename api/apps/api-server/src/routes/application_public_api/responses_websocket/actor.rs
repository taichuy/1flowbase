use std::{borrow::Cow, sync::Arc};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};

use super::{
    schema::{decode_client_message, ResponsesWebSocketClientRequest},
    turn_bridge::ResponsesTurnBridge,
    ResponsesWebSocketAuthorization,
};
use crate::{app_state::ApiState, runtime_activity::ApplicationActivityKind};

/// Externally observable lifecycle of one Responses WebSocket connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionState {
    Idle,
    Active,
    Cancelling,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TurnId(u64);

#[derive(Debug, PartialEq)]
pub(crate) enum ConnectionAction {
    StartTurn { turn: TurnId, response: Value },
    CancelTurn { turn: TurnId },
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TurnCompletion {
    ReturnedToIdle,
    Closed,
    IgnoredStaleTurn,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ConnectionTransitionError {
    #[error("response.generate must be a boolean when present")]
    InvalidGenerate,
    #[error("response must be an object")]
    InvalidResponse,
    #[error("response.input must be text or an array when present")]
    InvalidInput,
    #[error("only one response may be active on a connection")]
    ActiveTurnExists,
    #[error("the Responses WebSocket connection is closing")]
    ConnectionClosing,
}

impl ConnectionTransitionError {
    pub(crate) fn close_code(&self) -> u16 {
        1008
    }

    pub(crate) fn close_reason(&self) -> &'static str {
        match self {
            Self::InvalidGenerate => "response.generate must be boolean",
            Self::InvalidResponse => "response must be an object",
            Self::InvalidInput => "response.input must be text or an array",
            Self::ActiveTurnExists => "a response is already active",
            Self::ConnectionClosing => "connection is closing",
        }
    }
}

/// Pure EFSM for one socket. Transport IO and AgentFlow execution stay outside
/// this type so none of them can mutate the connection lifecycle directly.
pub(crate) struct ResponsesConnectionActor {
    state: ConnectionState,
    active_turn: Option<TurnId>,
    next_turn: u64,
}

impl ResponsesConnectionActor {
    pub(crate) fn new() -> Self {
        Self {
            state: ConnectionState::Idle,
            active_turn: None,
            next_turn: 1,
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> ConnectionState {
        self.state
    }

    pub(crate) fn accept_response(
        &mut self,
        response: Value,
    ) -> Result<ConnectionAction, ConnectionTransitionError> {
        if matches!(self.state, ConnectionState::Active) {
            return Err(ConnectionTransitionError::ActiveTurnExists);
        }
        if matches!(
            self.state,
            ConnectionState::Cancelling | ConnectionState::Closed
        ) {
            return Err(ConnectionTransitionError::ConnectionClosing);
        }

        let response = response
            .as_object()
            .cloned()
            .ok_or(ConnectionTransitionError::InvalidResponse)?;
        match response.get("generate") {
            None | Some(Value::Bool(_)) => {}
            Some(_) => return Err(ConnectionTransitionError::InvalidGenerate),
        }

        if response
            .get("input")
            .is_some_and(|input| !input.is_string() && !input.is_array())
        {
            return Err(ConnectionTransitionError::InvalidInput);
        }

        let response = Value::Object(response);
        let turn = TurnId(self.next_turn);
        self.next_turn = self.next_turn.saturating_add(1);
        self.active_turn = Some(turn);
        self.state = ConnectionState::Active;
        Ok(ConnectionAction::StartTurn { turn, response })
    }

    pub(crate) fn begin_close(&mut self) -> ConnectionAction {
        match (self.state, self.active_turn) {
            (ConnectionState::Active, Some(turn)) => {
                self.state = ConnectionState::Cancelling;
                ConnectionAction::CancelTurn { turn }
            }
            (ConnectionState::Cancelling, Some(turn)) => ConnectionAction::CancelTurn { turn },
            _ => {
                self.active_turn = None;
                self.state = ConnectionState::Closed;
                ConnectionAction::Close
            }
        }
    }

    pub(crate) fn complete_turn(&mut self, turn: TurnId) -> TurnCompletion {
        if self.active_turn != Some(turn) {
            return TurnCompletion::IgnoredStaleTurn;
        }

        self.active_turn = None;
        match self.state {
            ConnectionState::Cancelling | ConnectionState::Closed => {
                self.state = ConnectionState::Closed;
                TurnCompletion::Closed
            }
            ConnectionState::Active => {
                self.state = ConnectionState::Idle;
                TurnCompletion::ReturnedToIdle
            }
            ConnectionState::Idle => TurnCompletion::IgnoredStaleTurn,
        }
    }
}

pub(crate) async fn run_connection(
    socket: WebSocket,
    state: Arc<ApiState>,
    authorization: Arc<ResponsesWebSocketAuthorization>,
) {
    let _connection_activity = state.runtime_activity.start(
        authorization.principal.application_id(),
        ApplicationActivityKind::WebSocketConnection,
    );
    let bridge = Arc::new(ResponsesTurnBridge::new(state, authorization));
    run_connection_loop(socket, move |response, frames| {
        let bridge = bridge.clone();
        async move { bridge.execute(response, frames).await }
    })
    .await;
}

pub(super) async fn run_connection_loop<F, Fut>(socket: WebSocket, execute: F)
where
    F: Fn(Value, mpsc::Sender<String>) -> Fut,
    Fut: std::future::Future<Output = Result<(), super::turn_bridge::ResponsesTurnBridgeError>>
        + Send
        + 'static,
{
    let (mut sender, mut receiver) = socket.split();
    let mut terminal_delivered = false;
    let mut queued_response: Option<Message> = None;
    let mut actor = ResponsesConnectionActor::new();
    type ActiveTurn = (
        TurnId,
        JoinHandle<Result<(), super::turn_bridge::ResponsesTurnBridgeError>>,
        mpsc::Receiver<String>,
    );
    let mut active: Option<ActiveTurn> = None;

    loop {
        if let Some((turn, mut task, mut frames)) = active.take() {
            tokio::select! {
                biased;
                Some(frame) = frames.recv() => {
                    let terminal = serde_json::from_str::<Value>(&frame).ok().is_some_and(|event| {
                        matches!(event.get("type").and_then(Value::as_str), Some("response.completed" | "response.failed" | "response.incomplete" | "response.cancelled" | "error"))
                    });
                    if sender.send(Message::Text(frame)).await.is_err() {
                        task.abort();
                        break;
                    }
                    terminal_delivered |= terminal;
                    active = Some((turn, task, frames));
                }
                result = &mut task => {
                    let completion = actor.complete_turn(turn);
                    if result.is_err() || matches!(result, Ok(Err(_))) {
                        actor.begin_close();
                        finish_server_close(&mut sender, &mut receiver, Some(CloseFrame {
                            code: 1011,
                            reason: Cow::Borrowed("Responses turn failed"),
                        })).await;
                        break;
                    }
                    if completion == TurnCompletion::Closed {
                        break;
                    }
                }
                message = receiver.next() => {
                    let Some(message) = message else {
                        let action = actor.begin_close();
                        if matches!(action, ConnectionAction::CancelTurn { .. }) {
                            task.abort();
                            let _ = actor.complete_turn(turn);
                        }
                        break;
                    };
                    let Ok(message) = message else {
                        task.abort();
                        let action = actor.begin_close();
                        if matches!(action, ConnectionAction::CancelTurn { .. }) {
                            let _ = actor.complete_turn(turn);
                        }
                        break;
                    };

                    match message {
                        Message::Ping(payload) => {
                            if sender.send(Message::Pong(payload)).await.is_err() {
                                task.abort();
                                break;
                            }
                            active = Some((turn, task, frames));
                        }
                        Message::Pong(_) => active = Some((turn, task, frames)),
                        Message::Close(frame) => {
                            task.abort();
                            let action = actor.begin_close();
                            if matches!(action, ConnectionAction::CancelTurn { .. }) {
                                let _ = actor.complete_turn(turn);
                            }
                            let _ = sender.send(Message::Close(frame)).await;
                            break;
                        }
                        message => {
                            match decode_client_message(message.clone()) {
                                Ok(Some(ResponsesWebSocketClientRequest::Create { response })) => {
                                    // A delivered protocol terminal permits one next request,
                                    // but its dispatch must wait for the independent Kernel receipt.
                                    if terminal_delivered && queued_response.is_none() {
                                        queued_response = Some(message);
                                        active = Some((turn, task, frames));
                                        continue;
                                    }
                                    match actor.accept_response(response) {
                                        Ok(_) => active = Some((turn, task, frames)),
                                        Err(error) => {
                                            task.abort();
                                            let action = actor.begin_close();
                                            if matches!(action, ConnectionAction::CancelTurn { .. }) {
                                                let _ = actor.complete_turn(turn);
                                            }
                                            finish_server_close(
                                                &mut sender,
                                                &mut receiver,
                                                transition_close_frame(error),
                                            )
                                            .await;
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => active = Some((turn, task, frames)),
                                Err(error) => {
                                    task.abort();
                                    let action = actor.begin_close();
                                    if matches!(action, ConnectionAction::CancelTurn { .. }) {
                                        let _ = actor.complete_turn(turn);
                                    }
                                    finish_server_close(&mut sender, &mut receiver, Some(CloseFrame {
                                        code: error.close_code(),
                                        reason: Cow::Borrowed(error.close_reason()),
                                    })).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }

        let message = if let Some(message) = queued_response.take() {
            message
        } else {
            let Some(Ok(message)) = receiver.next().await else {
                actor.begin_close();
                break;
            };
            message
        };
        match message {
            Message::Ping(payload) => {
                if sender.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Message::Pong(_) => {}
            Message::Close(frame) => {
                actor.begin_close();
                let _ = sender.send(Message::Close(frame)).await;
                break;
            }
            message => match decode_client_message(message) {
                Ok(Some(ResponsesWebSocketClientRequest::Create { response })) => {
                    match actor.accept_response(response) {
                        Ok(ConnectionAction::StartTurn { turn, response }) => {
                            let (frame_sender, frame_receiver) = mpsc::channel(1);
                            terminal_delivered = false;
                            active = Some((
                                turn,
                                tokio::spawn(execute(response, frame_sender)),
                                frame_receiver,
                            ));
                        }
                        Ok(ConnectionAction::CancelTurn { .. } | ConnectionAction::Close) => {}
                        Err(error) => {
                            actor.begin_close();
                            finish_server_close(
                                &mut sender,
                                &mut receiver,
                                transition_close_frame(error),
                            )
                            .await;
                            break;
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    actor.begin_close();
                    finish_server_close(
                        &mut sender,
                        &mut receiver,
                        Some(CloseFrame {
                            code: error.close_code(),
                            reason: Cow::Borrowed(error.close_reason()),
                        }),
                    )
                    .await;
                    break;
                }
            },
        }
    }
}

fn transition_close_frame(error: ConnectionTransitionError) -> Option<CloseFrame<'static>> {
    Some(CloseFrame {
        code: error.close_code(),
        reason: Cow::Borrowed(error.close_reason()),
    })
}

const CLOSE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

/// A server-initiated close is complete only after the peer acknowledges it or a bounded timeout
/// expires. `SinkExt::send` flushes the Close frame before we continue polling the read half.
async fn finish_server_close(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    frame: Option<CloseFrame<'static>>,
) {
    if sender.send(Message::Close(frame)).await.is_err() {
        return;
    }
    let _ = timeout(CLOSE_HANDSHAKE_TIMEOUT, async {
        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Close(_)) | Err(_) => break,
                // Once Close is sent, no new application or control frame is written. Keep
                // driving the peer side until its Close arrives; Tungstenite owns protocol I/O.
                Ok(Message::Ping(_) | Message::Pong(_)) => {}
                Ok(_) => {}
            }
        }
    })
    .await;
}
