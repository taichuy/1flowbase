//! Managed duplex admission and socket projection. Runtime frames never touch durable storage.
use super::{
    interface::{Adapter, ManagedServiceInput, ManagedServiceOutput},
    ManagedServiceRegistration,
};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_interface::{
        self, ConsoleInterfaceTargetError, ConsoleServerStreamFuture, ConsoleServerStreamPort,
    },
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::HeaderMap,
    response::Response,
    Json,
};
use control_plane::errors::ControlPlaneError as Error;
use interface_runtime::{InterfaceContract, InterfaceStreamTerminal, UserPrincipal};
use runtime_core::runtime_backend::{RuntimeManagedDuplex, RuntimeManagedDuplexEvent};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::{mpsc, oneshot, watch};

pub(super) const FRAME_BYTES: usize = 1024 * 1024;

pub(crate) enum StreamInput {
    Ticket {
        origin: String,
    },
    Run {
        duplex: RuntimeManagedDuplex,
        cancel: watch::Receiver<bool>,
    },
}
pub(crate) struct StreamEvent(pub RuntimeManagedDuplexEvent);
impl InterfaceContract for StreamEvent {
    const CONTRACT_ID: &'static str = "managed-service-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        ManagedServiceInput::managed_projection_schema()
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(json!({"body_bytes":serde_json::to_vec(&self.0.event).ok()?.len()}))
    }
}
#[derive(Default)]
struct InstallationStreams {
    epoch: u64,
    sockets: BTreeMap<uuid::Uuid, watch::Sender<bool>>,
}
#[derive(Default)]
struct StreamRegistry {
    generation: Option<String>,
    installations: BTreeMap<uuid::Uuid, InstallationStreams>,
}
fn streams() -> &'static Mutex<StreamRegistry> {
    static STREAMS: OnceLock<Mutex<StreamRegistry>> = OnceLock::new();
    STREAMS.get_or_init(Mutex::default)
}
pub(crate) fn publish_generation(generation: String) {
    let mut all = streams().lock().unwrap_or_else(|e| e.into_inner());
    all.generation = Some(generation);
    for entry in all.installations.values_mut() {
        entry.epoch += 1;
        for (_, cancel) in std::mem::take(&mut entry.sockets) {
            let _ = cancel.send(true);
        }
    }
}
pub(crate) fn revoke_installation(id: uuid::Uuid) {
    let mut all = streams().lock().unwrap_or_else(|e| e.into_inner());
    let entry = all.installations.entry(id).or_default();
    entry.epoch += 1;
    for (_, cancel) in std::mem::take(&mut entry.sockets) {
        let _ = cancel.send(true);
    }
}
pub(crate) fn revoke_all() {
    let mut all = streams().lock().unwrap_or_else(|e| e.into_inner());
    for entry in all.installations.values_mut() {
        entry.epoch += 1;
        for (_, cancel) in std::mem::take(&mut entry.sockets) {
            let _ = cancel.send(true);
        }
    }
}
pub(super) fn epoch(id: uuid::Uuid) -> u64 {
    streams()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .installations
        .entry(id)
        .or_default()
        .epoch
}
struct SocketLease {
    installation: uuid::Uuid,
    id: uuid::Uuid,
    cancel: watch::Sender<bool>,
}
impl Drop for SocketLease {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
        if let Some(entry) = streams()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .installations
            .get_mut(&self.installation)
        {
            entry.sockets.remove(&self.id);
        }
    }
}
fn register(
    installation: uuid::Uuid,
    expected_epoch: u64,
    generation: &str,
) -> Result<(SocketLease, watch::Receiver<bool>), ApiError> {
    let mut all = streams().lock().unwrap_or_else(|e| e.into_inner());
    if all
        .generation
        .as_ref()
        .is_some_and(|current| current != generation)
    {
        return Err(Error::Conflict("managed_service_generation_mismatch").into());
    }
    let entry = all.installations.entry(installation).or_default();
    if entry.epoch != expected_epoch {
        return Err(Error::Conflict("managed_service_generation_mismatch").into());
    }
    let (cancel, receiver) = watch::channel(false);
    let id = uuid::Uuid::now_v7();
    entry.sockets.insert(id, cancel.clone());
    Ok((
        SocketLease {
            installation,
            id,
            cancel,
        },
        receiver,
    ))
}

impl ConsoleServerStreamPort<ManagedServiceInput, StreamEvent, ManagedServiceOutput> for Adapter {
    fn execute_stream(
        &self,
        principal: &UserPrincipal,
        input: ManagedServiceInput,
    ) -> ConsoleServerStreamFuture<StreamEvent, ManagedServiceOutput> {
        let state = self.state.upgrade();
        let principal = principal.clone();
        let registration = self.registration.clone();
        let op = self
            .operations
            .get(&input.interface_id)
            .map(|op| op.declaration.clone());
        Box::pin(async move {
            let prepare = async {
                let state = state.ok_or(Error::UpstreamUnavailable("managed_service_host"))?;
                let op = op.ok_or(Error::NotFound("managed_service_operation"))?;
                let validator = jsonschema::validator_for(&op.input_schema)?;
                if !validator.is_valid(&input.payload) { return Err(ApiError::from(Error::InvalidInput("managed_service_input"))); }
                let (publisher, stream) = interface_runtime::interface_stream_channel(16);
                match input.stream.ok_or(Error::InvalidInput("managed_service_stream"))? {
                    StreamInput::Ticket { origin } => {
                        let session = principal.authenticated_session().ok_or(Error::PermissionDenied("cookie_session_required"))?;
                        let ticket = super::stream_ticket::Ticket {
                            session: session.expose_to_trusted_handler().to_owned(), actor: principal.actor().user_id,
                            origin, installation: registration.installation_id,
                            generation: super::stream_ticket::generation(&state)?, epoch: epoch(registration.installation_id),
                            operation: op.interface_id, payload: input.payload,
                        };
                        let token = super::stream_ticket::store(state.infrastructure.cache_store().as_ref(), &ticket).await?;
                        publisher.finish(InterfaceStreamTerminal::Completed(ManagedServiceOutput(json!({"ticket":token})))).await.map_err(|_| Error::UpstreamUnavailable("managed_service_stream"))?;
                    }
                    StreamInput::Run { duplex, mut cancel } => {
                        let specification = op.stream.clone().ok_or(Error::InvalidInput("managed_service_stream"))?;
                        let event_validator = jsonschema::validator_for(&specification.event_schema)?;
                        // The receiver is separately forwarded through the canonical kernel event projection.
                        let runtime = state.provider_runtime.managed_composition()?;
                        let snapshot = match state.extension_boot_snapshot.as_ref().filter(|boot| boot.has_pinned_managed_snapshots()) {
                            Some(boot) => boot.managed_snapshot(domain::SYSTEM_SCOPE_ID),
                            None => runtime.snapshot(domain::SYSTEM_SCOPE_ID).await,
                        }.ok_or(Error::Conflict("managed_service_inactive"))?;
                        let contribution = plugin_framework::extension_bus::ContributionId::new(&op.contribution_id)?;
                        let binding = snapshot.bindings.get(&contribution).ok_or(Error::Conflict("managed_service_inactive"))?;
                        if binding.handle.identity().installation_id().as_str() != registration.installation_id.to_string() { return Err(Error::Conflict("managed_service_generation_mismatch").into()); }
                        // A duplex session has no one-shot business duration. Explicit transport
                        // cancellation, current authority/session checks and plugin idle own its life.
                        let deadline = i64::MAX;
                        let (sender, mut events) = mpsc::channel(16);
                        let duplex = RuntimeManagedDuplex { input: duplex.input, events: sender };
                        let needs_call_context = registration.declaration.worker_lifetime == plugin_framework::ManagedServiceWorkerLifetime::ActorWorkspaceScope;
                        let caller = if needs_call_context || binding.descriptor.required_permissions.iter().any(|permission| matches!(permission.as_str(), "file.import.resource" | "file.read.content" | "application.logs.read" | "application.logs.import" | "network.http.request")) {
                            crate::resource_import::capture_caller(&state, &principal, &op.interface_id).await?
                        } else { None };
                        if needs_call_context && caller.is_none() { return Err(Error::NotAuthenticated.into()); }
                        tokio::spawn(async move {
                            let execution = runtime.execute_snapshot_duplex(snapshot, domain::SYSTEM_SCOPE_ID, &contribution,
                                crate::extension_bus::ManagedExecutionInput {
                                    principal: runtime_core::runtime_backend::RuntimeExecutionPrincipal { caller, workspace_id: domain::SYSTEM_SCOPE_ID.to_string(), actor_id: Some(principal.actor().user_id.to_string()), deadline_unix_ms: deadline },
                                    config_payload: json!({}), input_payload: input.payload, duplex: Some(duplex),
                                });
                            tokio::pin!(execution);
                            let outcome = loop {
                                tokio::select! {
                                    biased;
                                    _ = cancel.changed() => break Err(Error::Conflict("managed_service_stream_cancelled").into()),
                                    result = &mut execution => break result,
                                    Some(event) = events.recv() => {
                                        if !event_validator.is_valid(&event.event) || serde_json::to_vec(&event.event).map_or(true, |bytes| bytes.len() > FRAME_BYTES - 128) { break Err(Error::UpstreamUnavailable("managed_service_event_contract").into()); }
                                        // Do not await subscriber ACK here. Its oneshot travels with the event.
                                        tokio::select! {
                                            _ = cancel.changed() => break Err(Error::Conflict("managed_service_stream_cancelled").into()),
                                            result = publisher.emit(StreamEvent(event)) => if result.is_err() { break Err(Error::Conflict("managed_service_stream_closed").into()); }
                                        }
                                    }
                                }
                            };
                            // Dropping the in-flight ephemeral runtime future explicitly cancels its call.
                            // Preserve output already emitted before the execution terminal.
                            while !*cancel.borrow() {
                                let Ok(event) = events.try_recv() else { break; };
                                if !event_validator.is_valid(&event.event) { break; }
                                tokio::select! {
                                    _ = cancel.changed() => break,
                                    result = publisher.emit(StreamEvent(event)) => if result.is_err() { break; }
                                }
                            }
                            let terminal = match outcome {
                                Ok(output) if jsonschema::validator_for(&op.output_schema).is_ok_and(|schema| schema.is_valid(&output)) => InterfaceStreamTerminal::Completed(ManagedServiceOutput(output)),
                                Err(error) => InterfaceStreamTerminal::Failed(interface_runtime::InterfaceTargetFailure::new("managed_service_stream", ConsoleInterfaceTargetError(super::ManagedServiceFailure::from_runtime(&error).map(ApiError::from).unwrap_or_else(|| Error::UpstreamUnavailable("managed_service_stream_execution").into())))),
                            _ => InterfaceStreamTerminal::Failed(interface_runtime::InterfaceTargetFailure::new("managed_service_stream", ConsoleInterfaceTargetError(Error::UpstreamUnavailable("managed_service_stream_execution").into()))),
                            };
                            let _ = publisher.finish(terminal).await;
                        });
                    }
                }
                Ok(stream)
            }.await;
            prepare.map_err(|error| {
                interface_runtime::InterfaceTargetFailure::new(
                    "managed_service_stream",
                    ConsoleInterfaceTargetError(error),
                )
            })
        })
    }
}

pub(super) fn route_assembly(
    service: &ManagedServiceRegistration,
    operation: &plugin_framework::ManagedServiceOperation,
) -> crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>> {
    use crate::routes::console_route_assembly::*;
    let path = super::routes::axum_path(&operation.path);
    let owner =
        access_control::ConsoleRouteOwnership::ConsoleOperation(operation.interface_id.clone());
    let operation = Arc::new(operation.clone());
    let post_op = operation.clone();
    let installation = service.installation_id;
    ConsoleRouteAssembly::new().route(&path, console_get(move |State(state): State<Arc<ApiState>>, Path(path): Path<BTreeMap<String,String>>, Query(query): Query<BTreeMap<String,String>>, headers: HeaderMap, ws: WebSocketUpgrade| {
        let operation = operation.clone();
        async move { upgrade(state, installation, operation, path, query, headers, ws).await }
    }, access_control::ConsoleRouteOwnership::Authenticated).post(move |State(state): State<Arc<ApiState>>, Path(path): Path<BTreeMap<String,String>>, headers: HeaderMap, Json(body): Json<Value>| {
        let operation = post_op.clone();
        async move {
            let origin = super::stream_ticket::origin(&headers)?;
            let stream = console_interface::invoke_server_stream::<ManagedServiceInput,StreamEvent,ManagedServiceOutput>(state.clone(), &format!("http.{}.v1",operation.interface_id), crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {state,headers}, ManagedServiceInput {interface_id:operation.interface_id.clone(),payload:json!({"path":path,"query":{},"body":body}),stream:Some(StreamInput::Ticket {origin})}).await?;
            let (_,completion) = stream.into_parts();
            match completion.complete().await.map_err(|_| Error::UpstreamUnavailable("managed_service_ticket"))?.into_parts().0 {
                InterfaceStreamTerminal::Completed(ManagedServiceOutput(value)) => Ok::<_,ApiError>(Json(ApiSuccess::new(value))),
                _ => Err(Error::UpstreamUnavailable("managed_service_ticket").into())
            }
        }
    }, owner))
}

async fn revalidate(
    state: &ApiState,
    headers: &HeaderMap,
    session: &str,
    operation: &plugin_framework::ManagedServiceOperation,
) -> Result<(), ApiError> {
    let context = crate::middleware::require_session::require_session(state, headers).await?;
    if context.cookie_session()?.session_id != session {
        return Err(Error::PermissionDenied("managed_service_session").into());
    }
    let access =
        crate::middleware::require_settings_feature_permission::compiled_console_route_access(
            &state.console_operation_registry,
            &operation.method,
            &operation.path,
        )
        .map_err(Error::PermissionDenied)?;
    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(&context.actor)
        .await?;
    if !crate::middleware::require_settings_feature_permission::authorize_compiled_console_access(
        &access,
        &context.actor,
        &policies,
    ) {
        return Err(Error::PermissionDenied("console_operation_permission_denied").into());
    }
    Ok(())
}
async fn upgrade(
    state: Arc<ApiState>,
    installation: uuid::Uuid,
    operation: Arc<plugin_framework::ManagedServiceOperation>,
    path: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let context = crate::middleware::require_session::require_session(&state, &headers).await?;
    let session = context.cookie_session()?.session_id.clone();
    let origin = super::stream_ticket::origin(&headers)?;
    let token = query
        .get("ticket")
        .ok_or(Error::PermissionDenied("managed_service_ticket"))?;
    let ticket = super::stream_ticket::claim(
        state.infrastructure.cache_store().as_ref(),
        token,
        super::stream_ticket::TicketBinding {
            session: &session,
            actor: context.actor.user_id,
            origin: &origin,
            installation,
            generation: &super::stream_ticket::generation(&state)?,
            operation: &operation.interface_id,
        },
    )
    .await?;
    if ticket.payload.get("path") != Some(&json!(path)) {
        return Err(Error::PermissionDenied("managed_service_ticket_route").into());
    }
    revalidate(&state, &headers, &session, &operation).await?;
    let (lease, cancel) = register(installation, ticket.epoch, &ticket.generation)?;
    let (input, input_receiver) = mpsc::channel(64);
    let (unused_events, _) = mpsc::channel(1);
    let stream = console_interface::invoke_server_stream::<
        ManagedServiceInput,
        StreamEvent,
        ManagedServiceOutput,
    >(
        state.clone(),
        &format!("http.{}.v1", operation.interface_id),
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers: headers.clone(),
        },
        ManagedServiceInput {
            interface_id: operation.interface_id.clone(),
            payload: ticket.payload,
            stream: Some(StreamInput::Run {
                duplex: RuntimeManagedDuplex {
                    input: Arc::new(tokio::sync::Mutex::new(input_receiver)),
                    events: unused_events,
                },
                cancel: cancel.clone(),
            }),
        },
    )
    .await?;
    let (events, completion) = stream.into_parts();
    let (terminal_tx, terminal_rx) = oneshot::channel();
    // This owner is independent of upgrade success, socket task abort and event projection failure.
    tokio::spawn(async move {
        let _ = terminal_tx.send(completion.complete().await);
    });
    Ok(ws
        .max_message_size(FRAME_BYTES)
        .max_frame_size(FRAME_BYTES)
        .on_upgrade(move |socket| {
            run_socket(
                socket,
                SocketSession {
                    state,
                    operation,
                    headers,
                    session,
                    input,
                    events,
                    terminal: terminal_rx,
                    cancel,
                    lease,
                },
            )
        }))
}
struct SocketSession {
    state: Arc<ApiState>,
    operation: Arc<plugin_framework::ManagedServiceOperation>,
    headers: HeaderMap,
    session: String,
    input: mpsc::Sender<Value>,
    events: mpsc::Receiver<StreamEvent>,
    terminal: oneshot::Receiver<
        Result<
            interface_runtime::InterfaceStreamTerminalOutcome<
                ManagedServiceOutput,
                ConsoleInterfaceTargetError,
            >,
            interface_runtime::InterfaceInvocationFailure,
        >,
    >,
    cancel: watch::Receiver<bool>,
    lease: SocketLease,
}
async fn run_socket(socket: WebSocket, session: SocketSession) {
    let SocketSession {
        state,
        operation,
        headers,
        session,
        input,
        mut events,
        mut terminal,
        mut cancel,
        lease,
    } = session;
    use futures_util::{SinkExt, StreamExt};
    let specification = operation
        .stream
        .as_ref()
        .expect("compiled stream operation");
    let Ok(input_validator) = jsonschema::validator_for(&specification.input_schema) else {
        return;
    };
    let (mut writer, mut reader) = socket.split();
    let (outgoing, mut outgoing_frames) =
        mpsc::channel::<(Message, Option<oneshot::Sender<()>>)>(16);
    let acknowledgements = Arc::new(Mutex::new(BTreeMap::<u64, oneshot::Sender<()>>::new()));
    let mut writer_cancel = cancel.clone();
    let writer_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = writer_cancel.changed() => break,
                frame = outgoing_frames.recv() => match frame {
                    Some((frame, delivered)) => {
                        tokio::select! {
                            _ = writer_cancel.changed() => break,
                            result = writer.send(frame) => {
                                if result.is_err() { break; }
                                if let Some(delivered) = delivered { let _ = delivered.send(()); }
                            },
                        }
                    }
                    None => break,
                }
            }
        }
        let _ = writer.close().await;
    });
    let projection_ack = acknowledgements.clone();
    let projection_out = outgoing.clone();
    let mut projection_cancel = cancel.clone();
    let (projection_done, mut projected) = oneshot::channel();
    let projection_task = tokio::spawn(async move {
        let mut id = 0;
        loop {
            let event = tokio::select! {
                _ = projection_cancel.changed() => break,
                event = events.recv() => event,
            };
            let Some(StreamEvent(event)) = event else {
                break;
            };
            let frame = json!({"type":"event","id":id,"event":event.event}).to_string();
            if frame.len() > FRAME_BYTES {
                break;
            }
            projection_ack
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id, event.consumed);
            id += 1;
            // Multiple frames may remain in flight; only browser subscriber ACK releases credit.
            tokio::select! {
                _ = projection_cancel.changed() => break,
                result = projection_out.send((Message::Text(frame.into()),None)) => if result.is_err() { break; },
            }
        }
        let _ = projection_done.send(());
    });
    let mut auth_cancel = cancel.clone();
    let auth_stop = lease.cancel.clone();
    let auth_task = tokio::spawn(async move {
        let mut validation = tokio::time::interval(std::time::Duration::from_secs(30));
        validation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        validation.tick().await;
        loop {
            tokio::select! {
                _ = auth_cancel.changed() => break,
                _ = validation.tick() => {
                    let valid = tokio::select! {
                        _ = auth_cancel.changed() => break,
                        result = revalidate(&state,&headers,&session,&operation) => result.is_ok(),
                    };
                    if !valid { let _ = auth_stop.send(true); break; }
                }
            }
        }
    });
    let (pending_input, mut pending_inputs) =
        mpsc::unbounded_channel::<(Value, tokio::sync::OwnedSemaphorePermit)>();
    let input_bytes = Arc::new(tokio::sync::Semaphore::new(FRAME_BYTES));
    let mut input_cancel = cancel.clone();
    let input_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = input_cancel.changed() => break,
                pending = pending_inputs.recv() => match pending {
                    Some((value,permit)) => {
                        tokio::select! {
                            _ = input_cancel.changed() => break,
                            result = input.send(value) => if result.is_err() { break; },
                        }
                        drop(permit);
                    }
                    None => break,
                }
            }
        }
    });
    let mut terminal_open = true;
    let mut projection_open = true;
    let mut completed = None;
    loop {
        if !terminal_open
            && !projection_open
            && acknowledgements
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        {
            if let Some(output) = completed.take() {
                let (delivered, received) = oneshot::channel();
                if outgoing
                    .send((
                        Message::Text(
                            json!({"type":"complete","output":output})
                                .to_string()
                                .into(),
                        ),
                        Some(delivered),
                    ))
                    .await
                    .is_ok()
                {
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), received).await;
                }
            }
            break;
        }
        tokio::select! {
            biased;
            _ = cancel.changed() => break,

            _ = &mut projected, if projection_open => { projection_open = false; },
            result = &mut terminal, if terminal_open => {
                terminal_open = false;
                let mut failure = "managed_service_stream_execution".to_owned();
                if let Ok(Ok(outcome)) = result {
                    match outcome.into_parts().0 {
                        InterfaceStreamTerminal::Completed(ManagedServiceOutput(output)) => { completed = Some(output); }
                        InterfaceStreamTerminal::Failed(error) => {
                            if let Some(safe) = error.error().0.0.downcast_ref::<super::ManagedServiceFailure>() { failure = safe.0.clone(); }
                        }
                        _ => {}
                    }
                }
                if completed.is_none() {
                    let (delivered, received) = oneshot::channel();
                    if outgoing.send((Message::Text(json!({"type":"error","message":failure}).to_string().into()),Some(delivered))).await.is_ok() {
                        let _ = tokio::time::timeout(std::time::Duration::from_secs(5),received).await;
                    }
                    break;
                }
            },
            frame = reader.next() => match frame {
                Some(Ok(Message::Text(text))) if text.len()<=FRAME_BYTES => {
                    match parse_client_frame(&text,&input_validator) {
                        Ok(ClientFrame::Input(value)) => {
                            let Ok(permit) = input_bytes.clone().try_acquire_many_owned(text.len().max(1) as u32) else { break; };
                            if pending_input.send((value,permit)).is_err() { break; }
                        }
                        Ok(ClientFrame::Ack(id)) => {
                            let Some(consumed) = acknowledgements.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) else { break; };
                            let _ = consumed.send(());
                        }
                        Err(()) => break,
                    }
                }
                Some(Ok(Message::Ping(bytes))) => { if outgoing.try_send((Message::Pong(bytes),None)).is_err() { break; } }
                Some(Ok(Message::Pong(_))) => {},
                _ => break,
            }
        }
    }
    drop(lease); // Explicit ephemeral-call cancellation on every exit, including invalid frames.
    auth_task.abort();
    input_task.abort();
    projection_task.abort();
    writer_task.abort();
    let _ = auth_task.await;
    let _ = input_task.await;
    let _ = projection_task.await;
    let _ = writer_task.await;
}

enum ClientFrame {
    Input(Value),
    Ack(u64),
}
fn parse_client_frame(text: &str, input: &jsonschema::Validator) -> Result<ClientFrame, ()> {
    if text.len() > FRAME_BYTES {
        return Err(());
    }
    let frame: Value = serde_json::from_str(text).map_err(|_| ())?;
    if !frame.as_object().is_some_and(|o| o.len() == 2) {
        return Err(());
    }
    match frame.get("type").and_then(Value::as_str) {
        Some("input") => frame
            .get("input")
            .filter(|v| input.is_valid(v))
            .cloned()
            .map(ClientFrame::Input)
            .ok_or(()),
        Some("ack") => frame
            .get("id")
            .and_then(Value::as_u64)
            .map(ClientFrame::Ack)
            .ok_or(()),
        _ => Err(()),
    }
}

#[cfg(test)]
#[path = "_tests/stream_lifecycle.rs"]
mod lifecycle_tests;
