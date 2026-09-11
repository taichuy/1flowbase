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

struct CompletionObserver(mpsc::UnboundedSender<InterfaceInvocationTerminal>);
impl InterfaceCompletionHook for CompletionObserver {
    fn completed(
        &self,
        _: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.0
                .send(terminal)
                .expect("fixture retains observation receiver");
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
                    .bind_completion(plugin, Arc::new(CompletionObserver(observations))),
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

#[tokio::test]
async fn issue_2028_socket_queues_next_turn_after_terminal_until_kernel_completion() {
    use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
    use futures_util::{SinkExt, StreamExt};
    use std::collections::VecDeque;
    use tokio_tungstenite::tungstenite::Message;
    use super::super::actor::run_connection_loop;

    tokio::time::timeout(Duration::from_secs(5), async {
        let (first, first_invocation, first_observed) = invocation().await;
        let (second, second_invocation, second_observed) = invocation().await;
        let invocations = Arc::new(Mutex::new(VecDeque::from([first_invocation, second_invocation])));
        let (dispatched, mut dispatches) = mpsc::unbounded_channel();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route("/ws", get(move |upgrade: WebSocketUpgrade| {
            let invocations = invocations.clone();
            let dispatched = dispatched.clone();
            async move {
                upgrade.on_upgrade(move |socket| run_connection_loop(socket, move |_, frames| {
                    let invocation = invocations.lock().unwrap().pop_front().expect("exactly two turns");
                    dispatched.send(()).unwrap();
                    let (events, completion) = invocation.into_parts();
                    project_turn(events, completion, ResponsesWebSocketProjector::new("model".into(), None), frames)
                }))
            }
        }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws")).await.unwrap();
        let create = Message::Text(json!({"type":"response.create","model":"model","input":[]}).to_string());
        socket.send(create.clone()).await.unwrap();
        dispatches.recv().await.unwrap();
        let mut first_run = run();
        first_run.status = NativeRunStatus::Succeeded;
        first.emit(CompatibilityStreamEvent::new(first_run.clone(), RuntimeEventEnvelope::new(
            first_run.id, 1, debug_stream_events::flow_finished(first_run.id, json!({})),
        ))).await.unwrap();
        let frame: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(frame["type"], "response.completed");
        socket.send(create).await.unwrap();
        socket.send(Message::Ping(vec![42])).await.unwrap();
        // Pong is a causal barrier proving the queued create was consumed while
        // the real Kernel terminal remains held, without using sleeps.
        assert_eq!(socket.next().await.unwrap().unwrap(), Message::Pong(vec![42]));
        assert!(dispatches.try_recv().is_err(), "next invocation must await the prior receipt");
        first.finish(InterfaceStreamTerminal::Completed(CompatibilityBlockingOutput(first_run))).await.unwrap();
        drop(first);
        dispatches.recv().await.unwrap();
        let mut second_run = run();
        second_run.status = NativeRunStatus::Succeeded;
        second.emit(CompatibilityStreamEvent::new(second_run.clone(), RuntimeEventEnvelope::new(
            second_run.id, 1, debug_stream_events::flow_finished(second_run.id, json!({})),
        ))).await.unwrap();
        second.finish(InterfaceStreamTerminal::Completed(CompatibilityBlockingOutput(second_run))).await.unwrap();
        drop(second);
        let frame: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(frame["type"], "response.completed");
        socket.close(None).await.unwrap();
        completed_once(first_observed, InterfaceInvocationTerminal::Completed).await;
        completed_once(second_observed, InterfaceInvocationTerminal::Completed).await;
        server.abort();
    }).await.expect("two socket turns and their Kernel receipts must complete");
}

#[tokio::test]
async fn issue_2028_sse_terminal_closes_delivery_without_waiting_for_producer_drop() {
    use crate::routes::application_public_api::{
        compatibility_interface::project_compatibility_stream,
        compat_sse::openai_responses_interface_projection,
    };
    for writer_closed in [false, true] {
        let (publisher, invocation, observed) = invocation().await;
        let (frames, mut received) = mpsc::channel(4);
        if writer_closed { received.close(); }
        let mut run = run();
        run.status = NativeRunStatus::Succeeded;
        publisher.emit(CompatibilityStreamEvent::new(run.clone(), RuntimeEventEnvelope::new(
            run.id, 1, debug_stream_events::flow_finished(run.id, json!({})),
        ))).await.unwrap();
        publisher.finish(InterfaceStreamTerminal::Completed(CompatibilityBlockingOutput(run))).await.unwrap();
        let (events, completion) = invocation.into_parts();
        tokio::time::timeout(Duration::from_secs(2), project_compatibility_stream(
            events, completion, openai_responses_interface_projection("model".into(), None), frames,
        )).await.expect("a producer retaining its sender must not hold SSE delivery open after terminal");
        if !writer_closed { assert!(received.recv().await.is_some()); }
        assert!(received.recv().await.is_none(), "SSE sender must close after the independent receipt");
        completed_once(observed, InterfaceInvocationTerminal::Completed).await;
        drop(publisher);
    }
}
