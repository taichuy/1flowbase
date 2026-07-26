use std::{borrow::Cow, sync::Arc};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Map, Value};
use thiserror::Error;
use tokio::task::JoinHandle;

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
    Prewarming,
    Active,
    Cancelling,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TurnId(u64);

#[derive(Debug, PartialEq)]
pub(crate) enum ConnectionAction {
    Prewarmed,
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
            Self::ActiveTurnExists => "a response is already active",
            Self::ConnectionClosing => "connection is closing",
        }
    }
}

/// Pure EFSM for one socket. Transport IO and AgentFlow execution stay outside
/// this type so none of them can mutate the connection lifecycle directly.
pub(crate) struct ResponsesConnectionActor {
    state: ConnectionState,
    prewarmed_response: Option<Map<String, Value>>,
    active_turn: Option<TurnId>,
    next_turn: u64,
}

impl ResponsesConnectionActor {
    pub(crate) fn new() -> Self {
        Self {
            state: ConnectionState::Idle,
            prewarmed_response: None,
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

        let mut response = response
            .as_object()
            .cloned()
            .ok_or(ConnectionTransitionError::InvalidResponse)?;
        let generate = match response.remove("generate") {
            None => true,
            Some(Value::Bool(generate)) => generate,
            Some(_) => return Err(ConnectionTransitionError::InvalidGenerate),
        };

        if !generate {
            self.prewarmed_response = Some(response);
            self.state = ConnectionState::Prewarming;
            return Ok(ConnectionAction::Prewarmed);
        }

        let response = match self.prewarmed_response.take() {
            Some(mut prewarmed) => {
                prewarmed.extend(response);
                Value::Object(prewarmed)
            }
            None => Value::Object(response),
        };
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
                self.prewarmed_response = None;
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
            ConnectionState::Idle | ConnectionState::Prewarming => TurnCompletion::IgnoredStaleTurn,
        }
    }
}

pub(crate) async fn run_connection(
    socket: WebSocket,
    state: Arc<ApiState>,
    authorization: Arc<ResponsesWebSocketAuthorization>,
) {
    let (mut sender, mut receiver) = socket.split();
    let _connection_activity = state.runtime_activity.start(
        authorization.actor.application_id,
        ApplicationActivityKind::WebSocketConnection,
    );
    let bridge = Arc::new(ResponsesTurnBridge::new(state, authorization));
    let mut actor = ResponsesConnectionActor::new();
    let mut active: Option<(
        TurnId,
        JoinHandle<Result<(), super::turn_bridge::ResponsesTurnBridgeError>>,
    )> = None;

    loop {
        if let Some((turn, mut task)) = active.take() {
            tokio::select! {
                result = &mut task => {
                    let completion = actor.complete_turn(turn);
                    if result.is_err() || matches!(result, Ok(Err(_))) {
                        actor.begin_close();
                        let _ = sender.send(Message::Close(Some(CloseFrame {
                            code: 1011,
                            reason: Cow::Borrowed("Responses turn failed"),
                        }))).await;
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
                            active = Some((turn, task));
                        }
                        Message::Pong(_) => active = Some((turn, task)),
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
                            match decode_client_message(message) {
                                Ok(Some(ResponsesWebSocketClientRequest::Create { response })) => {
                                    match actor.accept_response(response) {
                                        Ok(_) => active = Some((turn, task)),
                                        Err(error) => {
                                            task.abort();
                                            let action = actor.begin_close();
                                            if matches!(action, ConnectionAction::CancelTurn { .. }) {
                                                let _ = actor.complete_turn(turn);
                                            }
                                            let _ = sender.send(transition_close(error)).await;
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => active = Some((turn, task)),
                                Err(error) => {
                                    task.abort();
                                    let action = actor.begin_close();
                                    if matches!(action, ConnectionAction::CancelTurn { .. }) {
                                        let _ = actor.complete_turn(turn);
                                    }
                                    let _ = sender.send(Message::Close(Some(CloseFrame {
                                        code: error.close_code(),
                                        reason: Cow::Borrowed(error.close_reason()),
                                    }))).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }

        let Some(message) = receiver.next().await else {
            actor.begin_close();
            break;
        };
        let Ok(message) = message else {
            actor.begin_close();
            break;
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
                        Ok(ConnectionAction::Prewarmed) => {}
                        Ok(ConnectionAction::StartTurn { turn, response }) => {
                            let bridge = bridge.clone();
                            active = Some((
                                turn,
                                tokio::spawn(async move { bridge.execute(response).await }),
                            ));
                        }
                        Ok(ConnectionAction::CancelTurn { .. } | ConnectionAction::Close) => {}
                        Err(error) => {
                            actor.begin_close();
                            let _ = sender.send(transition_close(error)).await;
                            break;
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    actor.begin_close();
                    let _ = sender
                        .send(Message::Close(Some(CloseFrame {
                            code: error.close_code(),
                            reason: Cow::Borrowed(error.close_reason()),
                        })))
                        .await;
                    break;
                }
            },
        }
    }
}

fn transition_close(error: ConnectionTransitionError) -> Message {
    Message::Close(Some(CloseFrame {
        code: error.close_code(),
        reason: Cow::Borrowed(error.close_reason()),
    }))
}
