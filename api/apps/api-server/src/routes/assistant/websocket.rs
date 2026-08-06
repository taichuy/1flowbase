use std::sync::Arc;

use axum::{
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, State},
    http::{header, HeaderMap},
    response::Response,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use control_plane::{
    auth::hash_api_key_token,
    orchestration_runtime::{CancelFlowRunCommand, OrchestrationRuntimeService},
    ports::{CacheStore, OrchestrationRuntimeRepository},
};
use futures_util::{SinkExt, StreamExt};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    api_provider_runtime, assistant_preference_for_target, launch_assistant_execution,
    prepare_assistant_execution, ApiError, ApiState, RequestContext, StartAssistantRunBody,
};
use crate::{
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::{
        application_public_api::native_websocket::schema::sequence_from_event_id, debug_run_stream,
    },
};

const ASSISTANT_WEBSOCKET_PROTOCOL: &str = "1flowbase.assistant.v1";
const ASSISTANT_WEBSOCKET_TICKET_PREFIX: &str = "1flowbase.assistant.ticket.";
const ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS: i64 = 60;

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

#[derive(Debug, Serialize, Deserialize)]
struct AssistantWebSocketTicket {
    user_id: Uuid,
    workspace_id: Uuid,
    application_id: Uuid,
    origin: String,
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
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    assistant_preference_for_target(&state, &context, body.application_id).await?;
    let origin = required_origin(&headers)?;
    let ticket = AssistantWebSocketTicket {
        user_id: context.user.id,
        workspace_id: context.actor.current_workspace_id,
        application_id: body.application_id,
        origin,
    };
    let cache = state.infrastructure.cache_store();
    let token = store_ticket(cache.as_ref(), &ticket).await?;
    Ok(Json(ApiSuccess::new(AssistantWebSocketTicketResponse {
        ticket: token,
        protocol: ASSISTANT_WEBSOCKET_PROTOCOL,
        expires_in_seconds: ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS,
    })))
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

async fn store_ticket(
    cache: &dyn CacheStore,
    ticket: &AssistantWebSocketTicket,
) -> Result<String, ApiError> {
    for _ in 0..3 {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        if cache
            .set_if_absent_json(
                &ticket_key(&token),
                serde_json::to_value(ticket)?,
                Some(Duration::seconds(ASSISTANT_WEBSOCKET_TICKET_TTL_SECONDS)),
            )
            .await?
        {
            return Ok(token);
        }
    }
    Err(control_plane::errors::ControlPlaneError::Conflict("assistant_websocket_ticket").into())
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
    #[serde(rename = "run.create")]
    Create {
        request_id: String,
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
    },
}

impl AssistantWebSocketCommand {
    fn request_id(&self) -> &str {
        match self {
            Self::Create { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::Attach { request_id, .. } => request_id,
        }
    }
}

type ActiveTurn = (JoinHandle<Result<(), ApiError>>, mpsc::Receiver<String>);

async fn run_connection(
    socket: WebSocket,
    state: Arc<ApiState>,
    authorization: Arc<AssistantWebSocketAuthorization>,
) {
    let (mut sender, mut receiver) = socket.split();
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
                            Ok(AssistantWebSocketCommand::Cancel { request_id, run_id }) => {
                                match cancel_run(&state, &authorization, run_id).await {
                                    Ok(()) => { let _ = sender.send(Message::Text(json!({"type":"command.accepted","request_id":request_id,"command":"run.cancel","run_id":run_id}).to_string())).await; }
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
                    Ok(command) => {
                        let state = state.clone();
                        let authorization = authorization.clone();
                        let (frame_sender, frame_receiver) = mpsc::channel(32);
                        active = Some((
                            tokio::spawn(async move {
                                execute_command(state, authorization, command, frame_sender).await
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
}

async fn execute_command(
    state: Arc<ApiState>,
    authorization: Arc<AssistantWebSocketAuthorization>,
    command: AssistantWebSocketCommand,
    frames: mpsc::Sender<String>,
) -> Result<(), ApiError> {
    let (request_id, run_id, from_sequence) = match command {
        AssistantWebSocketCommand::Create {
            request_id,
            request,
        } => {
            if request.application_id != authorization.application_id {
                return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "assistant_application_id",
                )
                .into());
            }
            let execution = prepare_assistant_execution(
                &state,
                &authorization.request_headers,
                &authorization.context,
                request,
            )
            .await?;
            let run_id = launch_assistant_execution(state.clone(), execution).await?;
            (request_id, run_id, None)
        }
        AssistantWebSocketCommand::Attach {
            request_id,
            run_id,
            after_event_id,
        } => {
            let run = state
                .store
                .get_flow_run(authorization.application_id, run_id)
                .await?
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "flow_run",
                ))?;
            if run.created_by != authorization.context.user.id {
                return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "assistant_run",
                )
                .into());
            }
            let from_sequence = sequence_from_event_id(run_id, after_event_id.as_deref())
                .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("event_id"))?;
            (request_id, run_id, from_sequence)
        }
        AssistantWebSocketCommand::Cancel { .. } => {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("assistant_command").into(),
            )
        }
    };
    let (event_sender, mut events) = mpsc::channel::<Value>(32);
    tokio::spawn(debug_run_stream::send_runtime_event_websocket_stream(
        state.runtime_event_stream.clone(),
        run_id,
        from_sequence,
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

async fn cancel_run(
    state: &Arc<ApiState>,
    authorization: &AssistantWebSocketAuthorization,
    run_id: Uuid,
) -> Result<(), ApiError> {
    let runtime = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    runtime
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: authorization.context.user.id,
            application_id: authorization.application_id,
            flow_run_id: run_id,
        })
        .await?;
    Ok(())
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
    use super::*;
    use storage_ephemeral::MokaCacheStore;

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
