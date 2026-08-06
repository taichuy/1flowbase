use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::{sync::mpsc, task::JoinHandle};

use super::{
    schema::{decode_client_message, NativeWebSocketClientCommand},
    turn_bridge::{NativeTurnBridge, NativeTurnBridgeError},
    NativeWebSocketAuthorization,
};
use crate::{app_state::ApiState, runtime_activity::ApplicationActivityKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeConnectionState {
    Idle,
    Active,
    Closed,
}

pub(crate) struct NativeConnectionActor {
    state: NativeConnectionState,
    next_turn: u64,
}

impl NativeConnectionActor {
    pub(crate) fn new() -> Self {
        Self {
            state: NativeConnectionState::Idle,
            next_turn: 1,
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> NativeConnectionState {
        self.state
    }

    pub(crate) fn start_turn(&mut self) -> Result<u64, &'static str> {
        if self.state != NativeConnectionState::Idle {
            return Err("active_run_exists");
        }
        let turn = self.next_turn;
        self.next_turn = self.next_turn.saturating_add(1);
        self.state = NativeConnectionState::Active;
        Ok(turn)
    }

    pub(crate) fn complete_turn(&mut self, turn: u64, active_turn: u64) {
        if self.state == NativeConnectionState::Active && turn == active_turn {
            self.state = NativeConnectionState::Idle;
        }
    }

    pub(crate) fn close(&mut self) {
        self.state = NativeConnectionState::Closed;
    }
}

type ActiveTurn = (
    u64,
    JoinHandle<Result<(), NativeTurnBridgeError>>,
    mpsc::Receiver<String>,
);

pub(crate) async fn run_connection(
    socket: WebSocket,
    state: Arc<ApiState>,
    authorization: Arc<NativeWebSocketAuthorization>,
) {
    let (mut sender, mut receiver) = socket.split();
    let _activity = state.runtime_activity.start(
        authorization.actor.application_id,
        ApplicationActivityKind::WebSocketConnection,
    );
    let bridge = Arc::new(NativeTurnBridge::new(state, authorization));
    let mut actor = NativeConnectionActor::new();
    let mut active: Option<ActiveTurn> = None;

    loop {
        if let Some((turn, mut task, mut frames)) = active.take() {
            tokio::select! {
                biased;
                Some(frame) = frames.recv() => {
                    if sender.send(Message::Text(frame)).await.is_err() {
                        task.abort();
                        break;
                    }
                    active = Some((turn, task, frames));
                }
                result = &mut task => {
                    actor.complete_turn(turn, turn);
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) if error.code() != "writer_closed" => {
                            let _ = sender.send(error_frame(None, error.code(), error.to_string())).await;
                        }
                        Ok(Err(_)) => {}
                        Err(_) => {
                            let _ = sender.send(error_frame(None, "turn_task_failed", "Native turn task failed")).await;
                        }
                    }
                }
                message = receiver.next() => {
                    let Some(Ok(message)) = message else {
                        task.abort();
                        actor.close();
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
                            actor.close();
                            let _ = sender.send(Message::Close(frame)).await;
                            break;
                        }
                        other => match decode_client_message(other) {
                            Ok(Some(NativeWebSocketClientCommand::Cancel { request_id, run_id })) => {
                                match bridge.cancel(&request_id, run_id).await {
                                    Ok(frame) => { let _ = sender.send(Message::Text(frame)).await; }
                                    Err(error) => { let _ = sender.send(error_frame(Some(&request_id), error.code(), error.to_string())).await; }
                                }
                                active = Some((turn, task, frames));
                            }
                            Ok(Some(command)) => {
                                let request_id = command.request_id().to_string();
                                let _ = sender.send(error_frame(Some(&request_id), "active_run_exists", "only one run may be active on a connection")).await;
                                active = Some((turn, task, frames));
                            }
                            Ok(None) => active = Some((turn, task, frames)),
                            Err(error) => {
                                let _ = sender.send(error_frame(None, error.code(), error.to_string())).await;
                                active = Some((turn, task, frames));
                            }
                        }
                    }
                }
            }
            continue;
        }

        let Some(Ok(message)) = receiver.next().await else {
            actor.close();
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
                actor.close();
                let _ = sender.send(Message::Close(frame)).await;
                break;
            }
            other => match decode_client_message(other) {
                Ok(Some(NativeWebSocketClientCommand::Cancel { request_id, .. })) => {
                    let _ = sender
                        .send(error_frame(
                            Some(&request_id),
                            "no_active_run",
                            "run.cancel requires an active run",
                        ))
                        .await;
                }
                Ok(Some(command)) => {
                    let request_id = command.request_id().to_string();
                    match actor.start_turn() {
                        Ok(turn) => {
                            let bridge = bridge.clone();
                            let (frame_sender, frame_receiver) = mpsc::channel(32);
                            active = Some((
                                turn,
                                tokio::spawn(
                                    async move { bridge.execute(command, frame_sender).await },
                                ),
                                frame_receiver,
                            ));
                        }
                        Err(code) => {
                            let _ = sender
                                .send(error_frame(
                                    Some(&request_id),
                                    code,
                                    "only one run may be active on a connection",
                                ))
                                .await;
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = sender
                        .send(error_frame(None, error.code(), error.to_string()))
                        .await;
                }
            },
        }
    }
}

fn error_frame(request_id: Option<&str>, code: &str, message: impl Into<String>) -> Message {
    Message::Text(
        json!({
            "type": "error",
            "request_id": request_id,
            "error": { "code": code, "message": message.into() }
        })
        .to_string(),
    )
}
