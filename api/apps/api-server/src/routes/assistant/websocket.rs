use std::sync::Arc;

use axum::{
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, State},
    http::{header, HeaderMap},
    response::Response,
    Json,
};
use control_plane::{auth::hash_api_key_token, ports::CacheStore};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::Duration;
use tokio::{sync::broadcast, sync::mpsc, task::JoinHandle};
use utoipa::ToSchema;
use uuid::Uuid;

use super::websocket_interface::{
    AssistantConversationSubscription, AssistantWebSocketCommandInput,
    AssistantWebSocketCommandOutput, AssistantWebSocketRun, BINDING_ID,
};
use super::websocket_ticket_interface::{
    AssistantWebSocketTicket, ASSISTANT_WEBSOCKET_PROTOCOL, ASSISTANT_WEBSOCKET_TICKET_PREFIX,
};
use super::{
    assistant_preference_for_target, ApiError, ApiState, AssistantClientToolBridge,
    AssistantClientToolId, AssistantConversationPageResponse, RequestContext,
    StartAssistantRunBody,
};
use crate::{
    middleware::require_session::require_session, response::ApiSuccess, routes::debug_run_stream,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAssistantWebSocketTicketBody {
    pub application_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantWebSocketTicketResponse {
    pub ticket: String,
    pub protocol: &'static str,
    pub expires_in_seconds: i64,
}

#[derive(Clone)]
struct AssistantWebSocketAuthorization {
    context: RequestContext,
    application_id: Uuid,
    request_headers: HeaderMap,
}

struct ExpectedAssistantWebSocketTicket<'a> {
    user_id: Uuid,
    workspace_id: Uuid,
    origin: &'a str,
}

#[utoipa::path(
    post,
    path = "/api/console/assistant/runs/websocket-ticket",
    operation_id = "assistant_create_websocket_ticket",
    summary = "Create an embedded Assistant WebSocket ticket",
    description = "Creates a short-lived single-use ticket bound to the current Cookie session, workspace, application, CSRF-authorized request, and browser Origin.",
    request_body = CreateAssistantWebSocketTicketBody,
    responses(
        (status = 200, body = AssistantWebSocketTicketResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_ticket(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAssistantWebSocketTicketBody>,
) -> Result<Json<ApiSuccess<AssistantWebSocketTicketResponse>>, ApiError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.runs.websocket-ticket.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::websocket_ticket_interface::AssistantWebSocketTicketInput::Create { body, origin },
    )
    .await?;
    let super::websocket_ticket_interface::AssistantWebSocketTicketOutput::Ticket(ticket) = output;
    Ok(Json(ApiSuccess::new(ticket)))
}

#[utoipa::path(
    get,
    path = "/api/console/assistant/runs/websocket",
    operation_id = "assistant_runs_websocket",
    summary = "Open an embedded Assistant WebSocket",
    description = "Upgrades a Cookie session request using the ticket offered as the 1flowbase.assistant.ticket.<token> WebSocket subprotocol. The selected subprotocol is 1flowbase.assistant.v1.",
    responses(
        (status = 101, description = "WebSocket upgrade"),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn upgrade(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    let origin = required_origin(&headers)?;
    let token = ticket_from_protocols(&headers)?;
    let ticket = consume_ticket(
        state.infrastructure.cache_store().as_ref(),
        &token,
        ExpectedAssistantWebSocketTicket {
            user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            origin: &origin,
        },
    )
    .await?;
    assistant_preference_for_target(&state, &context, ticket.application_id).await?;
    let mut request_headers = headers;
    for name in [
        header::SEC_WEBSOCKET_PROTOCOL,
        header::SEC_WEBSOCKET_KEY,
        header::SEC_WEBSOCKET_VERSION,
        header::UPGRADE,
        header::CONNECTION,
    ] {
        request_headers.remove(name);
    }
    let authorization = AssistantWebSocketAuthorization {
        context,
        application_id: ticket.application_id,
        request_headers,
    };
    Ok(websocket
        .protocols([ASSISTANT_WEBSOCKET_PROTOCOL])
        .on_upgrade(move |socket| async move {
            run_connection(socket, state, Arc::new(authorization)).await;
        }))
}

async fn consume_ticket(
    cache: &dyn CacheStore,
    token: &str,
    expected: ExpectedAssistantWebSocketTicket<'_>,
) -> Result<AssistantWebSocketTicket, ApiError> {
    let value = cache.get_json(&ticket_key(token)).await?.ok_or(
        control_plane::errors::ControlPlaneError::PermissionDenied("assistant_websocket_ticket"),
    )?;
    let ticket: AssistantWebSocketTicket = serde_json::from_value(value)?;
    if ticket.user_id != expected.user_id
        || ticket.workspace_id != expected.workspace_id
        || ticket.origin != expected.origin
    {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "assistant_websocket_ticket",
        )
        .into());
    }
    let claimed = cache
        .set_if_absent_json(
            &ticket_claim_key(token),
            json!(true),
            Some(Duration::minutes(2)),
        )
        .await?;
    if !claimed {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "assistant_websocket_ticket",
        )
        .into());
    }
    cache.delete(&ticket_key(token)).await?;
    Ok(ticket)
}

fn ticket_key(token: &str) -> String {
    format!("assistant-websocket-ticket:{}", hash_api_key_token(token))
}

fn ticket_claim_key(token: &str) -> String {
    format!(
        "assistant-websocket-ticket-claim:{}",
        hash_api_key_token(token)
    )
}

fn required_origin(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| !origin.is_empty() && *origin != "null")
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            control_plane::errors::ControlPlaneError::PermissionDenied("assistant_websocket_origin")
                .into()
        })
}

fn ticket_from_protocols(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix(ASSISTANT_WEBSOCKET_TICKET_PREFIX))
        .filter(|ticket| !ticket.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            control_plane::errors::ControlPlaneError::PermissionDenied("assistant_websocket_ticket")
                .into()
        })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AssistantWebSocketCommand {
    #[serde(rename = "conversation.subscribe")]
    SubscribeConversations { request_id: String },
    #[serde(rename = "run.create")]
    Create {
        request_id: String,
        #[serde(default)]
        client_tool_ids: Vec<AssistantClientToolId>,
        request: StartAssistantRunBody,
    },
    #[serde(rename = "run.cancel")]
    Cancel { request_id: String, run_id: Uuid },
    #[serde(rename = "run.attach")]
    Attach {
        request_id: String,
        run_id: Uuid,
        #[serde(default)]
        after_event_id: Option<String>,
        #[serde(default)]
        client_tool_ids: Vec<AssistantClientToolId>,
    },
    #[serde(rename = "client_tool.result")]
    ClientToolResult {
        request_id: String,
        call_id: Uuid,
        result: Value,
        #[serde(default)]
        is_error: bool,
    },
}

impl AssistantWebSocketCommand {
    fn request_id(&self) -> &str {
        match self {
            Self::SubscribeConversations { request_id }
            | Self::Create { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::Attach { request_id, .. }
            | Self::ClientToolResult { request_id, .. } => request_id,
        }
    }
}

async fn invoke_command_interface(
    state: Arc<ApiState>,
    authorization: &AssistantWebSocketAuthorization,
    input: AssistantWebSocketCommandInput,
) -> Result<AssistantWebSocketCommandOutput, ApiError> {
    crate::routes::console_interface::invoke_with_principal(
        state,
        BINDING_ID,
        authorization.context.interface_principal(),
        input,
    )
    .await
}

pub(super) fn enabled_client_tools_for_connection(
    preference: &[AssistantClientToolId],
    declared: &[AssistantClientToolId],
) -> Vec<AssistantClientToolId> {
    preference
        .iter()
        .copied()
        .filter(|tool_id| declared.contains(tool_id))
        .collect()
}

type ActiveTurn = (JoinHandle<Result<(), ApiError>>, mpsc::Receiver<String>);

async fn run_connection(
    socket: WebSocket,
    state: Arc<ApiState>,
    authorization: Arc<AssistantWebSocketAuthorization>,
) {
    let (mut sender, mut receiver) = socket.split();
    let (client_tool_bridge, mut client_tool_frames) = AssistantClientToolBridge::new();
    if sender
        .send(Message::Text(
            json!({
                "type": "connection.ready",
                "application_id": authorization.application_id,
            })
            .to_string(),
        ))
        .await
        .is_err()
    {
        return;
    }
    let mut active: Option<ActiveTurn> = None;
    loop {
        if let Some((mut task, mut frames)) = active.take() {
            tokio::select! {
                biased;
                Some(frame) = frames.recv() => {
                    if sender.send(Message::Text(frame)).await.is_err() {
                        task.abort();
                        break;
                    }
                    active = Some((task, frames));
                }
                Some(frame) = client_tool_frames.recv() => {
                    if sender.send(Message::Text(frame)).await.is_err() {
                        task.abort();
                        break;
                    }
                    active = Some((task, frames));
                }
                result = &mut task => {
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            let _ = sender.send(command_error(None, "assistant_run_failed", error.0.to_string())).await;
                        }
                        Err(_) => {
                            let _ = sender.send(command_error(None, "assistant_turn_failed", "Assistant WebSocket turn failed")).await;
                        }
                    }
                }
                message = receiver.next() => {
                    let Some(Ok(message)) = message else {
                        task.abort();
                        break;
                    };
                    match message {
                        Message::Ping(payload) => {
                            if sender.send(Message::Pong(payload)).await.is_err() {
                                task.abort();
                                break;
                            }
                            active = Some((task, frames));
                        }
                        Message::Pong(_) => active = Some((task, frames)),
                        Message::Close(frame) => {
                            task.abort();
                            let _ = sender.send(Message::Close(frame)).await;
                            break;
                        }
                        Message::Text(text) => match serde_json::from_str::<AssistantWebSocketCommand>(text.as_str()) {
                            Ok(AssistantWebSocketCommand::ClientToolResult { request_id, call_id, result, is_error }) => {
                                match invoke_command_interface(
                                    state.clone(),
                                    authorization.as_ref(),
                                    AssistantWebSocketCommandInput::ClientToolResult {
                                        connection_id: client_tool_bridge.connection_id(),
                                        call_id,
                                        result,
                                        is_error,
                                    },
                                ).await {
                                    Ok(AssistantWebSocketCommandOutput::ClientToolResult { completed: true }) => {
                                        let _ = sender.send(Message::Text(json!({"type":"command.accepted","request_id":request_id,"command":"client_tool.result","call_id":call_id}).to_string())).await;
                                    }
                                    Ok(AssistantWebSocketCommandOutput::ClientToolResult { completed: false }) => {
                                        let _ = sender.send(command_error(Some(&request_id), "unknown_client_tool_call", "client tool call is not pending")).await;
                                    }
                                    Ok(_) => {
                                        let _ = sender.send(command_error(Some(&request_id), "assistant_command_failed", "client tool result returned an invalid output")).await;
                                    }
                                    Err(error) => {
                                        let _ = sender.send(command_error(Some(&request_id), "assistant_command_failed", error.0.to_string())).await;
                                    }
                                }
                                active = Some((task, frames));
                            }
                            Ok(AssistantWebSocketCommand::Cancel { request_id, run_id }) => {
                                match invoke_command_interface(
                                    state.clone(),
                                    authorization.as_ref(),
                                    AssistantWebSocketCommandInput::Cancel {
                                        application_id: authorization.application_id,
                                        run_id,
                                    },
                                ).await {
                                    Ok(AssistantWebSocketCommandOutput::Cancelled) => { let _ = sender.send(Message::Text(json!({"type":"command.accepted","request_id":request_id,"command":"run.cancel","run_id":run_id}).to_string())).await; }
                                    Ok(_) => { let _ = sender.send(command_error(Some(&request_id), "assistant_cancel_failed", "run.cancel returned an invalid output")).await; }
                                    Err(error) => { let _ = sender.send(command_error(Some(&request_id), "assistant_cancel_failed", error.0.to_string())).await; }
                                }
                                active = Some((task, frames));
                            }
                            Ok(command) => {
                                let _ = sender.send(command_error(Some(command.request_id()), "active_run_exists", "only one Assistant run may be active")).await;
                                active = Some((task, frames));
                            }
                            Err(_) => {
                                let _ = sender.send(command_error(None, "invalid_command", "invalid Assistant WebSocket command")).await;
                                active = Some((task, frames));
                            }
                        },
                        Message::Binary(_) => {
                            let _ = sender.send(command_error(None, "binary_not_supported", "binary messages are not supported")).await;
                            active = Some((task, frames));
                        }
                    }
                }
            }
            continue;
        }

        let Some(Ok(message)) = receiver.next().await else {
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
                let _ = sender.send(Message::Close(frame)).await;
                break;
            }
            Message::Binary(_) => {
                let _ = sender
                    .send(command_error(
                        None,
                        "binary_not_supported",
                        "binary messages are not supported",
                    ))
                    .await;
            }
            Message::Text(text) => {
                match serde_json::from_str::<AssistantWebSocketCommand>(text.as_str()) {
                    Ok(AssistantWebSocketCommand::Cancel { request_id, .. }) => {
                        let _ = sender
                            .send(command_error(
                                Some(&request_id),
                                "no_active_run",
                                "run.cancel requires an active run",
                            ))
                            .await;
                    }
                    Ok(command @ AssistantWebSocketCommand::ClientToolResult { .. }) => {
                        let state = state.clone();
                        let authorization = authorization.clone();
                        let command_client_tool_bridge = client_tool_bridge.clone();
                        let (frame_sender, frame_receiver) = mpsc::channel(32);
                        active = Some((
                            tokio::spawn(async move {
                                execute_command(
                                    state,
                                    authorization,
                                    command,
                                    frame_sender,
                                    command_client_tool_bridge,
                                )
                                .await
                            }),
                            frame_receiver,
                        ));
                    }
                    Ok(command) => {
                        let state = state.clone();
                        let authorization = authorization.clone();
                        let command_client_tool_bridge = client_tool_bridge.clone();
                        let (frame_sender, frame_receiver) = mpsc::channel(32);
                        active = Some((
                            tokio::spawn(async move {
                                execute_command(
                                    state,
                                    authorization,
                                    command,
                                    frame_sender,
                                    command_client_tool_bridge,
                                )
                                .await
                            }),
                            frame_receiver,
                        ));
                    }
                    Err(_) => {
                        let _ = sender
                            .send(command_error(
                                None,
                                "invalid_command",
                                "invalid Assistant WebSocket command",
                            ))
                            .await;
                    }
                }
            }
        }
    }
    let connection_id = client_tool_bridge.connection_id();
    let sessions = state
        .assistant_client_sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for session in sessions {
        session.close_connection(connection_id).await;
    }
}

async fn execute_command(
    state: Arc<ApiState>,
    authorization: Arc<AssistantWebSocketAuthorization>,
    command: AssistantWebSocketCommand,
    frames: mpsc::Sender<String>,
    client_tool_bridge: AssistantClientToolBridge,
) -> Result<(), ApiError> {
    match command {
        AssistantWebSocketCommand::SubscribeConversations { request_id } => {
            let output = invoke_command_interface(
                state.clone(),
                authorization.as_ref(),
                AssistantWebSocketCommandInput::SubscribeConversations {
                    application_id: authorization.application_id,
                },
            )
            .await?;
            let AssistantWebSocketCommandOutput::ConversationSubscription(subscription) = output
            else {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "assistant_command_output",
                )
                .into());
            };
            return project_conversation_subscription(
                state,
                authorization,
                request_id,
                frames,
                subscription,
            )
            .await;
        }
        AssistantWebSocketCommand::Create {
            request_id,
            request,
            client_tool_ids,
        } => {
            let output = invoke_command_interface(
                state.clone(),
                authorization.as_ref(),
                AssistantWebSocketCommandInput::Create {
                    application_id: authorization.application_id,
                    request,
                    request_headers: authorization.request_headers.clone(),
                    client_tool_ids,
                    client_tool_bridge,
                },
            )
            .await?;
            let AssistantWebSocketCommandOutput::Run(run) = output else {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "assistant_command_output",
                )
                .into());
            };
            project_run_stream(state, request_id, frames, run).await
        }
        AssistantWebSocketCommand::Attach {
            request_id,
            run_id,
            after_event_id,
            client_tool_ids,
        } => {
            let output = invoke_command_interface(
                state.clone(),
                authorization.as_ref(),
                AssistantWebSocketCommandInput::Attach {
                    application_id: authorization.application_id,
                    run_id,
                    after_event_id,
                    client_tool_ids,
                    client_tool_bridge,
                },
            )
            .await?;
            let AssistantWebSocketCommandOutput::Run(run) = output else {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "assistant_command_output",
                )
                .into());
            };
            project_run_stream(state, request_id, frames, run).await
        }
        AssistantWebSocketCommand::ClientToolResult {
            request_id,
            call_id,
            result,
            is_error,
        } => {
            let output = invoke_command_interface(
                state,
                authorization.as_ref(),
                AssistantWebSocketCommandInput::ClientToolResult {
                    connection_id: client_tool_bridge.connection_id(),
                    call_id,
                    result,
                    is_error,
                },
            )
            .await?;
            let AssistantWebSocketCommandOutput::ClientToolResult { completed } = output else {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "assistant_command_output",
                )
                .into());
            };
            if completed {
                frames
                    .send(
                        json!({"type":"command.accepted","request_id":request_id,"command":"client_tool.result","call_id":call_id}).to_string(),
                    )
                    .await
                    .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("assistant_websocket"))?;
            } else {
                frames
                    .send(
                        command_error(
                            Some(&request_id),
                            "unknown_client_tool_call",
                            "client tool call is not pending",
                        )
                        .into_text()
                        .unwrap_or_default(),
                    )
                    .await
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::Conflict("assistant_websocket")
                    })?;
            }
            Ok(())
        }
        AssistantWebSocketCommand::Cancel { .. } => {
            Err(control_plane::errors::ControlPlaneError::InvalidInput("assistant_command").into())
        }
    }
}

async fn project_run_stream(
    state: Arc<ApiState>,
    request_id: String,
    frames: mpsc::Sender<String>,
    run: AssistantWebSocketRun,
) -> Result<(), ApiError> {
    let (event_sender, mut events) = mpsc::channel::<Value>(32);
    tokio::spawn(debug_run_stream::send_runtime_event_websocket_stream(
        state.runtime_event_stream.clone(),
        run.run_id,
        run.from_sequence,
        event_sender,
    ));
    while let Some(mut event) = events.recv().await {
        event["request_id"] = Value::String(request_id.clone());
        frames.send(event.to_string()).await.map_err(|_| {
            control_plane::errors::ControlPlaneError::Conflict("assistant_websocket")
        })?;
        if matches!(
            event.get("type").and_then(Value::as_str),
            Some(
                "flow_finished"
                    | "flow_incomplete"
                    | "flow_failed"
                    | "flow_cancelled"
                    | "waiting_human"
                    | "replay_expired"
                    | "replay_gap"
            )
        ) {
            return Ok(());
        }
    }
    Ok(())
}

async fn project_conversation_subscription(
    state: Arc<ApiState>,
    authorization: Arc<AssistantWebSocketAuthorization>,
    request_id: String,
    frames: mpsc::Sender<String>,
    subscription: AssistantConversationSubscription,
) -> Result<(), ApiError> {
    let mut events = subscription.events;
    send_conversation_snapshot(subscription.snapshot, &request_id, &frames).await?;

    loop {
        match events.recv().await {
            Ok(event) => {
                let mut frame = serde_json::to_value(event)?;
                frame["request_id"] = Value::String(request_id.clone());
                frames.send(frame.to_string()).await.map_err(|_| {
                    control_plane::errors::ControlPlaneError::Conflict("assistant_websocket")
                })?;
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                let output = invoke_command_interface(
                    state.clone(),
                    authorization.as_ref(),
                    AssistantWebSocketCommandInput::SubscribeConversations {
                        application_id: authorization.application_id,
                    },
                )
                .await?;
                let AssistantWebSocketCommandOutput::ConversationSubscription(subscription) =
                    output
                else {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "assistant_command_output",
                    )
                    .into());
                };
                events = subscription.events;
                send_conversation_snapshot(subscription.snapshot, &request_id, &frames).await?;
            }
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

async fn send_conversation_snapshot(
    snapshot: AssistantConversationPageResponse,
    request_id: &str,
    frames: &mpsc::Sender<String>,
) -> Result<(), ApiError> {
    frames
        .send(
            json!({
                "type": "conversation.snapshot",
                "request_id": request_id,
                "data": snapshot,
            })
            .to_string(),
        )
        .await
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::Conflict("assistant_websocket").into()
        })
}

fn command_error(request_id: Option<&str>, code: &str, message: impl Into<String>) -> Message {
    Message::Text(
        json!({
            "type": "error",
            "request_id": request_id,
            "error": { "code": code, "message": message.into() }
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::websocket_ticket_interface::store_ticket;
    use super::*;
    use storage_ephemeral::MokaCacheStore;

    #[test]
    fn ac_001_parses_application_conversation_subscription() {
        let command: AssistantWebSocketCommand = serde_json::from_value(json!({
            "type": "conversation.subscribe",
            "request_id": "conversation-subscribe-1"
        }))
        .unwrap();

        let AssistantWebSocketCommand::SubscribeConversations { request_id } = command else {
            panic!("expected conversation.subscribe");
        };
        assert_eq!(request_id, "conversation-subscribe-1");
    }

    #[test]
    fn ac_002_run_create_declares_only_supported_client_tool_ids() {
        let command: AssistantWebSocketCommand = serde_json::from_value(json!({
            "type": "run.create",
            "request_id": "create-1",
            "client_tool_ids": ["get_client_context", "refresh_client_view"],
            "request": {
                "application_id": Uuid::from_u128(1),
                "query": "refresh the page",
                "history": []
            }
        }))
        .unwrap();

        let AssistantWebSocketCommand::Create {
            client_tool_ids, ..
        } = command
        else {
            panic!("expected run.create");
        };
        assert_eq!(
            client_tool_ids,
            vec![
                AssistantClientToolId::GetClientContext,
                AssistantClientToolId::RefreshClientView,
            ]
        );
    }

    #[test]
    fn ac_001_disabled_client_tools_are_not_registered_for_the_connection() {
        assert_eq!(
            enabled_client_tools_for_connection(
                &[AssistantClientToolId::GetClientContext],
                &[
                    AssistantClientToolId::GetClientContext,
                    AssistantClientToolId::RefreshClientView,
                ],
            ),
            vec![AssistantClientToolId::GetClientContext]
        );
        assert!(enabled_client_tools_for_connection(
            &[AssistantClientToolId::RefreshClientView],
            &[AssistantClientToolId::GetClientContext],
        )
        .is_empty());
    }

    #[test]
    fn issue_1601_ticket_protocol_is_not_a_url_parameter() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "1flowbase.assistant.v1, 1flowbase.assistant.ticket.secret"
                .parse()
                .unwrap(),
        );
        assert_eq!(ticket_from_protocols(&headers).unwrap(), "secret");
    }

    #[test]
    fn issue_1601_ticket_cache_keys_do_not_expose_the_bearer_secret() {
        let token = "raw-browser-ticket";
        assert!(!ticket_key(token).contains(token));
        assert!(!ticket_claim_key(token).contains(token));
        assert_eq!(ticket_key(token), ticket_key(token));
    }

    #[test]
    fn issue_1601_origin_is_required_and_null_is_rejected() {
        assert!(required_origin(&HeaderMap::new()).is_err());
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, "null".parse().unwrap());
        assert!(required_origin(&headers).is_err());
        headers.insert(header::ORIGIN, "https://console.example".parse().unwrap());
        assert_eq!(
            required_origin(&headers).unwrap(),
            "https://console.example"
        );
    }

    #[tokio::test]
    async fn issue_1601_wrong_context_does_not_consume_single_use_ticket() {
        let cache = MokaCacheStore::new("assistant-ticket-test", 32);
        let ticket = AssistantWebSocketTicket {
            user_id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            application_id: Uuid::now_v7(),
            origin: "https://console.example".to_string(),
        };
        let token = store_ticket(&cache, &ticket).await.unwrap();

        assert!(consume_ticket(
            &cache,
            &token,
            ExpectedAssistantWebSocketTicket {
                user_id: ticket.user_id,
                workspace_id: ticket.workspace_id,
                origin: "https://wrong.example",
            },
        )
        .await
        .is_err());

        let consumed = consume_ticket(
            &cache,
            &token,
            ExpectedAssistantWebSocketTicket {
                user_id: ticket.user_id,
                workspace_id: ticket.workspace_id,
                origin: &ticket.origin,
            },
        )
        .await
        .unwrap();
        assert_eq!(consumed.application_id, ticket.application_id);
        assert!(consume_ticket(
            &cache,
            &token,
            ExpectedAssistantWebSocketTicket {
                user_id: ticket.user_id,
                workspace_id: ticket.workspace_id,
                origin: &ticket.origin,
            },
        )
        .await
        .is_err());
    }
}
