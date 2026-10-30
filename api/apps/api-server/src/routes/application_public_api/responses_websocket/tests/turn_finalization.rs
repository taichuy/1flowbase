//! Root #1998 F2: real Kernel finalization survives the production bridge's
//! abort, writer failure and missing terminal; normal projection still completes.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use control_plane::{
    application_public_api::native::{NativeRunResult, NativeRunStatus},
    orchestration_runtime::debug_stream_events,
    ports::RuntimeEventEnvelope,
};
use domain::ActorContext;
use interface_runtime::*;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::super::{
    projector::ResponsesWebSocketProjector,
    turn_bridge::{project_turn, ResponsesTurnBridgeError},
};
use crate::routes::application_public_api::compatibility_interface::{
    CompatibilityBlockingOutput, CompatibilityBlockingTargetError, CompatibilityStreamEvent,
};

type Stream = InterfaceEventStream<
    CompatibilityStreamEvent,
    CompatibilityBlockingOutput,
    CompatibilityBlockingTargetError,
>;
type Publisher = InterfaceStreamPublisher<
    CompatibilityStreamEvent,
    CompatibilityBlockingOutput,
    CompatibilityBlockingTargetError,
>;
type Invocation = InterfaceStreamInvocation<
    CompatibilityStreamEvent,
    CompatibilityBlockingOutput,
    CompatibilityBlockingTargetError,
>;

struct Input;
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "ws-finalization-input";
    const CONTRACT_VERSION: &'static str = "1";
}

struct Target(Mutex<Option<Stream>>);
impl
    InterfaceStreamHandler<
        Input,
        CompatibilityStreamEvent,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    > for Target
{
    fn invoke_stream(
        &self,
        _: InterfaceHandlerContext,
        _: Input,
    ) -> InterfaceStreamHandlerFuture<
        CompatibilityStreamEvent,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    > {
        let stream = self
            .0
            .lock()
            .unwrap()
            .take()
            .expect("one invocation per fixture");
        Box::pin(async move { Ok(stream) })
    }
}

struct Authorization;
impl InterfaceAuthorizationPort for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("ws.finalization.authz").unwrap()
    }
    fn authorize(&self, _: InterfaceAuthorizationRequest) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct CompletionBarrier {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

struct CompletionObserver {
    observations: mpsc::UnboundedSender<InterfaceInvocationTerminal>,
    barrier: Option<Arc<CompletionBarrier>>,
}
impl InterfaceCompletionHook for CompletionObserver {
    fn completed(
        &self,
        _: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.observations
                .send(terminal)
                .expect("fixture retains observation receiver");
            if let Some(barrier) = &self.barrier {
                barrier.entered.notify_one();
                barrier.release.notified().await;
            }
        })
    }
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).unwrap()
}

async fn invocation() -> (
    Publisher,
    Invocation,
    mpsc::UnboundedReceiver<InterfaceInvocationTerminal>,
) {
    invocation_with_completion_barrier(None).await
}

async fn invocation_with_completion_barrier(
    barrier: Option<Arc<CompletionBarrier>>,
) -> (
    Publisher,
    Invocation,
    mpsc::UnboundedReceiver<InterfaceInvocationTerminal>,
) {
    let id = InterfaceId::new("ws.finalization").unwrap();
    let identity = InterfaceIdentity::new(id.clone(), InterfaceVersion::new("1").unwrap());
    let owner = InterfaceOwner::new("ws.finalization.owner").unwrap();
    let operation = AuthorizationOperation::new("ws.finalization.run").unwrap();
    let handler = HandlerReference::new("ws.finalization.handler").unwrap();
    let target = TargetReference::new("ws.finalization.target").unwrap();
    let graph = GraphFingerprint::new("graph:ws-finalization").unwrap();
    let binding = BindingId::new("http.ws.finalization.v1").unwrap();
    let authentication = AuthenticationAdapterReference::new("ws.finalization.authn").unwrap();
    let activation =
        AuthenticationActivationIdentity::new("ws.finalization.authn.activation").unwrap();
    let contracts = InterfaceContracts::server_stream(
        contract::<Input>(),
        contract::<CompatibilityStreamEvent>(),
        contract::<CompatibilityBlockingOutput>(),
        contract::<CompatibilityBlockingTargetError>(),
    );
    let mut compiler = RegistryCompiler::new(graph.clone(), [operation.clone()], [owner.clone()]);
    compiler
        .register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                PrincipalProfile::User,
                InterfaceAuthenticationPolicy::Authenticated,
                operation,
                InterfaceScope::System,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::ServerStream,
                handler.clone(),
                target.clone(),
            ),
            InterfaceAuditPolicy::ReadOnly,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner,
        ))
        .unwrap();
    let auth_plugin = PluginIdentity::new("ws.finalization.authentication").unwrap();
    compiler
        .register_authentication_adapter(
            &id,
            1,
            InterfaceExtensionRegistration::new(
                auth_plugin.clone(),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                InterfaceScope::System,
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .unwrap(),
            ActivatedAuthenticationAdapter::new(
                auth_plugin,
                InterfaceExtensionTier::BuiltIn,
                authentication.clone(),
                activation.clone(),
                PrincipalProfile::User,
            ),
        )
        .unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                binding.clone(),
                identity,
                contracts,
                ProtocolProjection::http(RouteIdentity::new("POST", "/ws-finalization").unwrap()),
            ),
            InvocationAdapterPlan::new(
                authentication.clone(),
                AuthorizationAdapterReference::new("ws.finalization.authz").unwrap(),
                None,
            ),
        )
        .unwrap();
    let (publisher, stream) = interface_stream_channel(4);
    compiler.bind_stream_handler::<Input, CompatibilityStreamEvent, CompatibilityBlockingOutput, CompatibilityBlockingTargetError, UserPrincipal>(&id, handler.clone(), Arc::new(Target(Mutex::new(Some(stream))))).unwrap();
    let (observations, observed) = mpsc::unbounded_channel();
    let plugin = PluginIdentity::new("ws.finalization.completion").unwrap();
    compiler
        .register_extension(
            &id,
            2,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Completion,
                InterfaceExtensionPermission::ObserveCompletion,
                InterfaceScope::System,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::Terminal],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_hook_plan(
            &id,
            Arc::new(
                TypedInterfaceHookPlan::<Input, CompatibilityBlockingOutput>::new(graph)
                    .bind_completion(
                        plugin,
                        Arc::new(CompletionObserver {
                            observations,
                            barrier,
                        }),
                    ),
            ),
        )
        .unwrap();
    let registry = compiler.compile().unwrap();
    let kernel = InterfaceInvocationKernel::new(Arc::new(Authorization));
    let invocation = kernel.invoke_server_stream_with_dispatch_target::<Input, CompatibilityStreamEvent, CompatibilityBlockingOutput, CompatibilityBlockingTargetError>(
        registry,
        InvocationEnvelope::new(InvocationLineage::root(InvocationId::now_v7()), binding, InterfaceProtocol::Http, authentication, activation, ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root"), None, Input),
        ExecutionTargetPin::BuiltIn { handler, target },
    ).await.expect("real Kernel must open the fixture stream");
    (publisher, invocation, observed)
}

fn run() -> NativeRunResult {
    NativeRunResult {
        id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        api_key_id: Uuid::now_v7(),
        publication_version_id: Uuid::now_v7(),
        status: NativeRunStatus::Running,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: time::OffsetDateTime::UNIX_EPOCH,
    }
}

async fn completed_once(
    mut observed: mpsc::UnboundedReceiver<InterfaceInvocationTerminal>,
    expected: InterfaceInvocationTerminal,
) {
    tokio::time::timeout(Duration::from_secs(2), async move {
        assert_eq!(observed.recv().await, Some(expected));
        // Closing the observer channel proves its finalization owner was released;
        // a second invocation of the hook would produce a second terminal here.
        assert_eq!(observed.recv().await, None);
    })
    .await
    .expect("Kernel must finish its sole finalization owner after transport exit");
}

#[tokio::test]
async fn root_1998_bridge_abort_preserves_kernel_completion() {
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, mut received) = mpsc::channel(4);
    let bridge = tokio::spawn(project_turn(
        events,
        completion,
        ResponsesWebSocketProjector::new("model".into(), None),
        frames,
    ));
    let run = run();
    publisher
        .emit(CompatibilityStreamEvent::new(
            run.clone(),
            RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
        ))
        .await
        .unwrap();
    let frame: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(2), received.recv())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(frame["type"], "response.created");
    // The first delivered frame is a causal barrier: the actual bridge owns the
    // runtime and is awaiting more events before the actor aborts it.
    bridge.abort();
    assert!(bridge.await.unwrap_err().is_cancelled());
    publisher
        .finish(InterfaceStreamTerminal::Completed(
            CompatibilityBlockingOutput(run),
        ))
        .await
        .expect("abort must retain Kernel terminal receiver");
    completed_once(observed, InterfaceInvocationTerminal::Completed).await;
}

#[tokio::test]
async fn root_1998_closed_writer_preserves_kernel_completion() {
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, received) = mpsc::channel(1);
    drop(received);
    let run = run();
    publisher
        .emit(CompatibilityStreamEvent::new(
            run.clone(),
            RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
        ))
        .await
        .unwrap();
    let result = project_turn(
        events,
        completion,
        ResponsesWebSocketProjector::new("model".into(), None),
        frames,
    )
    .await;
    assert!(matches!(
        result,
        Err(ResponsesTurnBridgeError::SocketWriterClosed)
    ));
    publisher
        .finish(InterfaceStreamTerminal::Completed(
            CompatibilityBlockingOutput(run),
        ))
        .await
        .expect("writer error must retain Kernel terminal receiver");
    completed_once(observed, InterfaceInvocationTerminal::Completed).await;
}

#[tokio::test]
async fn root_1998_missing_terminal_still_finalizes_kernel_failure() {
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, _received) = mpsc::channel(1);
    drop(publisher);
    let result = project_turn(
        events,
        completion,
        ResponsesWebSocketProjector::new("model".into(), None),
        frames,
    )
    .await;
    assert!(matches!(
        result,
        Err(ResponsesTurnBridgeError::MissingTerminal)
    ));
    completed_once(observed, InterfaceInvocationTerminal::Failed).await;
}

#[tokio::test]
async fn root_1998_normal_bridge_delivers_terminal_and_finalizes_once() {
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, mut received) = mpsc::channel(4);
    let mut run = run();
    run.status = NativeRunStatus::Succeeded;
    publisher
        .emit(CompatibilityStreamEvent::new(
            run.clone(),
            RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::flow_finished(run.id, json!({})),
            ),
        ))
        .await
        .unwrap();
    publisher
        .finish(InterfaceStreamTerminal::Completed(
            CompatibilityBlockingOutput(run),
        ))
        .await
        .unwrap();
    project_turn(
        events,
        completion,
        ResponsesWebSocketProjector::new("model".into(), None),
        frames,
    )
    .await
    .expect("terminal projection and Kernel completion must both succeed");
    let frame: Value = serde_json::from_str(&received.recv().await.unwrap()).unwrap();
    assert_eq!(frame["type"], "response.completed");
    assert_eq!(frame["response"]["status"], "completed");
    assert!(received.recv().await.is_none());
    completed_once(observed, InterfaceInvocationTerminal::Completed).await;
}

fn committed_tool_claim(run: &NativeRunResult) -> control_plane::ports::RuntimeEventDeliveryClaim {
    let mut event = debug_stream_events::provider_output_item_done(
        "llm",
        Uuid::now_v7(),
        0,
        json!({"type":"function_call","call_id":"call-ws","name":"shell","arguments":"{}"}),
    );
    event.payload["committed_delivery"] = json!(true);
    control_plane::ports::RuntimeEventDeliveryClaim {
        event: domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: run.id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            sequence: 1,
            event_type: event.event_type,
            layer: domain::RuntimeEventLayer::ProviderRaw,
            source: domain::RuntimeEventSource::ProviderPlugin,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload: event.payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
            created_at: time::OffsetDateTime::now_utc(),
        },
        claim_token: Uuid::now_v7(),
        generation: 1,
        lease_expires_at: time::OffsetDateTime::now_utc(),
    }
}

fn committed_tool_event(
    run: &NativeRunResult,
    claim: &control_plane::ports::RuntimeEventDeliveryClaim,
) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run.id,
        claim.event.sequence,
        control_plane::ports::RuntimeEventPayload {
            event_type: claim.event.event_type.clone(),
            source: control_plane::ports::RuntimeEventSource::Provider,
            durability: control_plane::ports::RuntimeEventDurability::DurableRequired,
            persist_required: false,
            trace_visible: true,
            payload: claim.event.payload.clone(),
        },
    )
}

fn transparent_ws_projector() -> ResponsesWebSocketProjector {
    ResponsesWebSocketProjector::with_mode(
        "model".into(),
        None,
        crate::routes::application_public_api::compat_sse::ResponsesProjectionMode::TransparentProviderResponses,
    )
}

// AC-002: the WebSocket writer settles the durable tool delivery from the
// real frame write, never from the internal enqueue.
#[tokio::test]
async fn issue_2079_websocket_writer_acks_tool_delivery_only_after_frames_are_written() {
    use crate::routes::application_public_api::delivery_receipt::{
        RuntimeEventDeliveryReceipt, RuntimeEventDeliveryVerdict,
    };
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, mut received) = mpsc::channel(8);
    let mut run = run();
    run.status = NativeRunStatus::Succeeded;
    let claim = committed_tool_claim(&run);
    let (receipt, probe) = RuntimeEventDeliveryReceipt::probe(claim.clone());
    publisher
        .emit(CompatibilityStreamEvent::with_delivery(
            run.clone(),
            committed_tool_event(&run, &claim),
            Some(receipt),
        ))
        .await
        .unwrap();
    publisher
        .emit(CompatibilityStreamEvent::new(
            run.clone(),
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::flow_finished(run.id, json!({})),
            ),
        ))
        .await
        .unwrap();
    publisher
        .finish(InterfaceStreamTerminal::Completed(
            CompatibilityBlockingOutput(run),
        ))
        .await
        .unwrap();
    project_turn(events, completion, transparent_ws_projector(), frames)
        .await
        .expect("tool item and terminal both project");
    let mut types = Vec::new();
    while let Some(frame) = received.recv().await {
        let frame: Value = serde_json::from_str(&frame).unwrap();
        types.push(frame["type"].as_str().unwrap().to_string());
    }
    assert!(
        types.iter().any(|kind| kind == "response.output_item.done"),
        "tool item must be written: {types:?}"
    );
    assert_eq!(
        probe.verdict().await,
        Some(RuntimeEventDeliveryVerdict::Projected)
    );
    completed_once(observed, InterfaceInvocationTerminal::Completed).await;
}

#[tokio::test]
async fn issue_2079_closed_websocket_writer_marks_tool_delivery_uncertain() {
    use crate::routes::application_public_api::delivery_receipt::{
        RuntimeEventDeliveryReceipt, RuntimeEventDeliveryVerdict,
    };
    let (publisher, invocation, observed) = invocation().await;
    let (events, completion) = invocation.into_parts();
    let (frames, received) = mpsc::channel(8);
    drop(received);
    let run = run();
    let claim = committed_tool_claim(&run);
    let (receipt, probe) = RuntimeEventDeliveryReceipt::probe(claim.clone());
    publisher
        .emit(CompatibilityStreamEvent::with_delivery(
            run.clone(),
            committed_tool_event(&run, &claim),
            Some(receipt),
        ))
        .await
        .unwrap();
    let result = project_turn(events, completion, transparent_ws_projector(), frames).await;
    assert!(matches!(
        result,
        Err(ResponsesTurnBridgeError::SocketWriterClosed)
    ));
    // The frame write was attempted against a dead socket: the client may or
    // may not hold the tool call, so the delivery must not replay blindly.
    assert_eq!(
        probe.verdict().await,
        Some(RuntimeEventDeliveryVerdict::Uncertain)
    );
    publisher
        .finish(InterfaceStreamTerminal::Completed(
            CompatibilityBlockingOutput(run),
        ))
        .await
        .unwrap();
    completed_once(observed, InterfaceInvocationTerminal::Completed).await;
}

#[tokio::test]
async fn issue_2028_socket_queues_next_turn_after_terminal_until_kernel_completion() {
    use super::super::actor::run_connection_loop;
    use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
    use futures_util::{SinkExt, StreamExt};
    use std::collections::VecDeque;
    use tokio_tungstenite::tungstenite::Message;

    tokio::time::timeout(Duration::from_secs(5), async {
        let (first, first_invocation, first_observed) = invocation().await;
        let (second, second_invocation, second_observed) = invocation().await;
        let invocations = Arc::new(Mutex::new(VecDeque::from([
            first_invocation,
            second_invocation,
        ])));
        let (dispatched, mut dispatches) = mpsc::unbounded_channel();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route(
            "/ws",
            get(move |upgrade: WebSocketUpgrade| {
                let invocations = invocations.clone();
                let dispatched = dispatched.clone();
                async move {
                    upgrade.on_upgrade(move |socket| {
                        run_connection_loop(socket, move |_, frames| {
                            let invocation = invocations
                                .lock()
                                .unwrap()
                                .pop_front()
                                .expect("exactly two turns");
                            dispatched.send(()).unwrap();
                            let (events, completion) = invocation.into_parts();
                            project_turn(
                                events,
                                completion,
                                ResponsesWebSocketProjector::new("model".into(), None),
                                frames,
                            )
                        })
                    })
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();
        let create = Message::Text(
            json!({"type":"response.create","model":"model","input":[]})
                .to_string()
                .into(),
        );
        socket.send(create.clone()).await.unwrap();
        dispatches.recv().await.unwrap();
        let mut first_run = run();
        first_run.status = NativeRunStatus::Succeeded;
        first
            .emit(CompatibilityStreamEvent::new(
                first_run.clone(),
                RuntimeEventEnvelope::new(
                    first_run.id,
                    1,
                    debug_stream_events::flow_finished(first_run.id, json!({})),
                ),
            ))
            .await
            .unwrap();
        let frame: Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(frame["type"], "response.completed");
        socket.send(create).await.unwrap();
        socket.send(Message::Ping(vec![42].into())).await.unwrap();
        // Pong is a causal barrier proving the queued create was consumed while
        // the real Kernel terminal remains held, without using sleeps.
        assert_eq!(
            socket.next().await.unwrap().unwrap(),
            Message::Pong(vec![42].into())
        );
        assert!(
            dispatches.try_recv().is_err(),
            "next invocation must await the prior receipt"
        );
        first
            .finish(InterfaceStreamTerminal::Completed(
                CompatibilityBlockingOutput(first_run),
            ))
            .await
            .unwrap();
        drop(first);
        dispatches.recv().await.unwrap();
        let mut second_run = run();
        second_run.status = NativeRunStatus::Succeeded;
        second
            .emit(CompatibilityStreamEvent::new(
                second_run.clone(),
                RuntimeEventEnvelope::new(
                    second_run.id,
                    1,
                    debug_stream_events::flow_finished(second_run.id, json!({})),
                ),
            ))
            .await
            .unwrap();
        second
            .finish(InterfaceStreamTerminal::Completed(
                CompatibilityBlockingOutput(second_run),
            ))
            .await
            .unwrap();
        drop(second);
        let frame: Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(frame["type"], "response.completed");
        socket.close(None).await.unwrap();
        completed_once(first_observed, InterfaceInvocationTerminal::Completed).await;
        completed_once(second_observed, InterfaceInvocationTerminal::Completed).await;
        server.abort();
    })
    .await
    .expect("two socket turns and their Kernel receipts must complete");
}

#[tokio::test]
async fn issue_2039_server_close_flushes_terminal_and_waits_for_peer_ack() {
    use super::super::actor::run_connection_loop;
    use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{protocol::frame::coding::CloseCode, Message};

    tokio::time::timeout(Duration::from_secs(5), async {
        let (closed, mut closed_rx) = mpsc::unbounded_channel();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route(
            "/ws",
            get(move |upgrade: WebSocketUpgrade| {
                let closed = closed.clone();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        run_connection_loop(socket, move |_, frames| async move {
                            frames
                                .send(
                                    json!({
                                        "type": "response.failed",
                                        "response": {"id": "resp_failed", "status": "failed"}
                                    })
                                    .to_string(),
                                )
                                .await
                                .unwrap();
                            Err(ResponsesTurnBridgeError::MissingTerminal)
                        })
                        .await;
                        closed.send(()).unwrap();
                    })
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":"model","input":[]})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();

        let terminal: Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(terminal["type"], "response.failed");
        let close = socket.next().await.unwrap().unwrap();
        let Message::Close(Some(frame)) = close else {
            panic!("server must close with an RFC 6455 Close frame: {close:?}");
        };
        assert_eq!(frame.code, CloseCode::Error);
        assert!(
            closed_rx.try_recv().is_err(),
            "server handler must remain alive until the peer Close ACK is flushed"
        );

        socket.flush().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), closed_rx.recv())
            .await
            .expect("server must finish promptly after the Close ACK")
            .expect("server close observer must remain connected");
        server.abort();
    })
    .await
    .expect("terminal, Close, and peer ACK must complete within the bounded handshake window");
}

#[tokio::test]
async fn issue_2028_sse_terminal_closes_delivery_without_waiting_for_producer_drop() {
    use crate::routes::application_public_api::{
        compat_sse::openai_responses_interface_projection,
        compatibility_interface::project_compatibility_stream,
    };
    for writer_closed in [false, true] {
        let (publisher, invocation, observed) = invocation().await;
        let (frames, mut received) = mpsc::channel(4);
        if writer_closed {
            received.close();
        }
        let mut run = run();
        run.status = NativeRunStatus::Succeeded;
        publisher
            .emit(CompatibilityStreamEvent::new(
                run.clone(),
                RuntimeEventEnvelope::new(
                    run.id,
                    1,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            ))
            .await
            .unwrap();
        publisher
            .finish(InterfaceStreamTerminal::Completed(
                CompatibilityBlockingOutput(run),
            ))
            .await
            .unwrap();
        let (events, completion) = invocation.into_parts();
        tokio::time::timeout(
            Duration::from_secs(2),
            project_compatibility_stream(
                events,
                completion,
                openai_responses_interface_projection("model".into(), None),
                frames,
                crate::generation_dispatch::GenerationDispatch::new(Uuid::now_v7()),
            ),
        )
        .await
        .expect("a producer retaining its sender must not hold SSE delivery open after terminal");
        if !writer_closed {
            assert!(received.recv().await.is_some());
        }
        assert!(
            received.recv().await.is_none(),
            "SSE sender must close after the independent receipt"
        );
        completed_once(observed, InterfaceInvocationTerminal::Completed).await;
        drop(publisher);
    }
}

#[tokio::test]
async fn sse_independent_completion_keeps_g2_through_hook_after_publish_body_drop_and_projection_abort(
) {
    use crate::{
        _tests::support::{default_test_config, test_api_state_with_database_url},
        generation_dispatch::{bind_response, spawn_with_lease},
        routes::application_public_api::{
            compat_sse::openai_responses_interface_projection,
            compatibility_interface::project_compatibility_stream,
        },
    };
    use axum::response::{sse::Sse, IntoResponse};

    // Compatibility uses the real SSE projector; Assistant uses the same detached
    // completion spawn as start_run_stream. Both complete a real Kernel invocation.
    for compatibility_projection in [true, false] {
        let (state, _) = test_api_state_with_database_url().await;
        let config = default_test_config();
        let initial = crate::app_with_state_and_config(state.clone(), &config);
        let generations = crate::managed_publication::ManagedGenerationPublisher::new(
            &state,
            config,
            vec![],
            initial,
        )
        .unwrap();
        generations.publish(
            generations
                .prepare(vec![], Default::default())
                .await
                .unwrap(),
        );
        let g2 = generations.current_generation().unwrap();
        let g2_id = g2.owner.generation_id();
        let probe = Arc::downgrade(&g2.owner);
        let retired = g2.owner.retirement_notification();
        let barrier = Arc::new(CompletionBarrier {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let (publisher, invocation, observed) =
            invocation_with_completion_barrier(Some(barrier.clone())).await;
        let (frames, received) =
            mpsc::channel::<Result<axum::response::sse::Event, std::convert::Infallible>>(4);
        let body = bind_response(
            Sse::new(tokio_stream::wrappers::ReceiverStream::new(received)).into_response(),
            g2.owner.clone(),
        );
        let (events, completion) = invocation.into_parts();
        let projection = if compatibility_projection {
            Some(tokio::spawn(project_compatibility_stream(
                events,
                completion,
                openai_responses_interface_projection("model".into(), None),
                frames,
                g2.owner.clone(),
            )))
        } else {
            drop(events);
            drop(frames);
            // Exact independent-completion owner pattern used by Assistant SSE.
            drop(spawn_with_lease(g2.owner.clone(), async move {
                let _ = completion.complete().await;
            }));
            None
        };
        publisher
            .finish(InterfaceStreamTerminal::Completed(
                CompatibilityBlockingOutput(run()),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), barrier.entered.notified())
            .await
            .unwrap();
        drop(g2);
        generations.publish(
            generations
                .prepare(vec![], Default::default())
                .await
                .unwrap(),
        );
        assert_ne!(
            generations
                .current_generation()
                .unwrap()
                .owner
                .generation_id(),
            g2_id
        );
        drop(body);
        if let Some(projection) = projection {
            projection.abort();
            assert!(projection.await.unwrap_err().is_cancelled());
        }
        assert!(
            probe.upgrade().is_some(),
            "only independent completion holds G2 during the hook"
        );
        barrier.release.notify_one();
        tokio::time::timeout(Duration::from_secs(5), retired.notified())
            .await
            .unwrap();
        assert!(
            probe.upgrade().is_none(),
            "completed hook must release its lease"
        );
        completed_once(observed, InterfaceInvocationTerminal::Completed).await;
    }
}

// AC-002 (SSE lane): the SSE writer ACKs a tool delivery only after the
// frames reached the response sender; a closed sender leaves it uncertain.
#[tokio::test]
async fn issue_2079_sse_writer_settles_tool_delivery_from_the_real_write() {
    use crate::routes::application_public_api::{
        compat_sse::{openai_responses_interface_projection_with_mode, ResponsesProjectionMode},
        compatibility_interface::project_compatibility_stream,
        delivery_receipt::{RuntimeEventDeliveryReceipt, RuntimeEventDeliveryVerdict},
    };
    for writer_closed in [false, true] {
        let (publisher, invocation, observed) = invocation().await;
        let (frames, mut received) = mpsc::channel(8);
        if writer_closed {
            received.close();
        }
        let mut run = run();
        run.status = NativeRunStatus::Succeeded;
        let claim = committed_tool_claim(&run);
        let (receipt, probe) = RuntimeEventDeliveryReceipt::probe(claim.clone());
        publisher
            .emit(CompatibilityStreamEvent::with_delivery(
                run.clone(),
                committed_tool_event(&run, &claim),
                Some(receipt),
            ))
            .await
            .unwrap();
        publisher
            .emit(CompatibilityStreamEvent::new(
                run.clone(),
                RuntimeEventEnvelope::new(
                    run.id,
                    2,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            ))
            .await
            .unwrap();
        publisher
            .finish(InterfaceStreamTerminal::Completed(
                CompatibilityBlockingOutput(run),
            ))
            .await
            .unwrap();
        let (events, completion) = invocation.into_parts();
        tokio::time::timeout(
            Duration::from_secs(2),
            project_compatibility_stream(
                events,
                completion,
                openai_responses_interface_projection_with_mode(
                    "model".into(),
                    None,
                    ResponsesProjectionMode::TransparentProviderResponses,
                ),
                frames,
                crate::generation_dispatch::GenerationDispatch::new(Uuid::now_v7()),
            ),
        )
        .await
        .expect("SSE projection completes");
        let expected = if writer_closed {
            RuntimeEventDeliveryVerdict::Uncertain
        } else {
            RuntimeEventDeliveryVerdict::Projected
        };
        assert_eq!(
            probe.verdict().await,
            Some(expected),
            "writer_closed={writer_closed}"
        );
        completed_once(observed, InterfaceInvocationTerminal::Completed).await;
        drop(publisher);
    }
}

async fn peer_close_ack_roundtrip(active: bool, codec: bool, compression: bool) {
    use super::super::{actor::run_connection_loop, transport};
    use axum::{routing::get, Router};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite_deflate::{
        client_async_with_config,
        tungstenite::{
            extensions::compression::deflate::DeflateConfig,
            protocol::{frame::coding::CloseCode, CloseFrame, WebSocketConfig},
            Message,
        },
    };

    // Reuse the existing actor handshake test's bounded fixture window. No new
    // production timer or close policy is involved.
    tokio::time::timeout(Duration::from_secs(5), async {
        let (started, mut started_rx) = mpsc::unbounded_channel();
        let (finished, mut finished_rx) = mpsc::unbounded_channel();
        let (closed, mut closed_rx) = mpsc::unbounded_channel();
        let release = Arc::new(tokio::sync::Notify::new());
        let task_release = release.clone();
        let run_socket = move |socket: transport::Socket| {
            let started = started.clone();
            let finished = finished.clone();
            let closed = closed.clone();
            let release = task_release.clone();
            async move {
                run_connection_loop(socket, move |_, _frames| {
                    let started = started.clone();
                    let finished = finished.clone();
                    let release = release.clone();
                    async move {
                        started.send(()).unwrap();
                        release.notified().await;
                        finished.send(()).unwrap();
                        Ok(())
                    }
                })
                .await;
                closed.send(()).unwrap();
            }
        };
        let router = if codec {
            Router::new().route(
                "/ws",
                get(move |upgrade: transport::WebSocketUpgrade| {
                    let run = run_socket.clone();
                    async move { upgrade.on_upgrade(run).await }
                }),
            )
        } else {
            Router::new().route(
                "/ws",
                get(move |upgrade: axum::extract::ws::WebSocketUpgrade| {
                    let run = run_socket.clone();
                    async move { upgrade.on_upgrade(move |socket| run(socket.into())) }
                }),
            )
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut config = WebSocketConfig::default();
        if compression {
            config.extensions.permessage_deflate = Some(DeflateConfig::default());
        }
        let (mut socket, response) = client_async_with_config(
            format!("ws://{address}/ws"),
            tokio::net::TcpStream::connect(address).await.unwrap(),
            Some(config),
        )
        .await
        .unwrap();
        assert_eq!(
            response.headers().contains_key("sec-websocket-extensions"),
            compression
        );
        if active {
            socket
                .send(Message::Text(
                    json!({"type":"response.create","model":"fixture","input":[]})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            started_rx
                .recv()
                .await
                .expect("the actor must have an active turn before Close");
        }
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "正常完成".into(),
            }))
            .await
            .unwrap();
        let reply = socket
            .next()
            .await
            .expect("Close ACK must arrive before EOF")
            .expect("Close ACK must not be a reset/1006");
        let Message::Close(Some(frame)) = reply else {
            panic!("expected peer Close ACK, got {reply:?}");
        };
        assert_eq!(frame.code, CloseCode::Normal);
        assert_eq!(frame.reason.as_str(), "正常完成");
        closed_rx
            .recv()
            .await
            .expect("the actor must finish after flushing the ACK");
        if active {
            // Peer Close must not turn the existing detached execution lifetime
            // into an abort. Release it explicitly and observe normal completion.
            assert!(finished_rx.try_recv().is_err());
            release.notify_one();
            finished_rx
                .recv()
                .await
                .expect("active execution remains alive until explicitly released");
        } else {
            assert!(
                started_rx.try_recv().is_err(),
                "idle close must not start a turn"
            );
        }
        server.abort();
    })
    .await
    .expect("peer Close ACK and actor cleanup must finish within the existing fixture window");
}

#[tokio::test]
async fn peer_close_flushes_queued_ack_while_idle() {
    for (codec, compression) in [(false, false), (true, false), (true, true)] {
        peer_close_ack_roundtrip(false, codec, compression).await;
    }
}

#[tokio::test]
async fn peer_close_flushes_queued_ack_while_active() {
    for (codec, compression) in [(false, false), (true, false), (true, true)] {
        peer_close_ack_roundtrip(true, codec, compression).await;
    }
}
