use std::sync::Arc;

use domain::ActorContext;
use uuid::Uuid;

use crate::{
    interface_stream_channel, AdmissionAdapterReference, ArtifactIdentity,
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, ContractIdentity, ExecutionTargetPin, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceExtensionFact,
    InterfaceExtensionIsolation, InterfaceExtensionPermission, InterfaceExtensionPoint,
    InterfaceExtensionRegistration, InterfaceExtensionTier, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceIdentity,
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceInvocationStage,
    InterfaceLifecycle, InterfaceOwner, InterfaceProtocol, InterfaceScope, InterfaceStreamHandler,
    InterfaceStreamHandlerFuture, InterfaceStreamTerminal, InterfaceVersion, InvocationAdapterPlan,
    InvocationEnvelope, InvocationId, InvocationLineage, PluginIdentity, PrincipalProfile,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, RuntimeGeneration,
    RuntimeTargetIdentity, TargetReference, UserPrincipal, WorkerGeneration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Input(u8);
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "review-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Output(u8);
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "review-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StreamEvent(u8);
impl InterfaceContract for StreamEvent {
    const CONTRACT_ID: &'static str = "review-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetError;
impl InterfaceContract for TargetError {
    const CONTRACT_ID: &'static str = "review-target-error";
    const CONTRACT_VERSION: &'static str = "1";
}

struct UnaryHandler;
impl InterfaceHandler<Input, Output, TargetError> for UnaryHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        Box::pin(async move { Ok(Output(input.0)) })
    }
}

struct StreamingHandler;
impl InterfaceStreamHandler<Input, StreamEvent, Output, TargetError> for StreamingHandler {
    fn invoke_stream(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceStreamHandlerFuture<StreamEvent, Output, TargetError> {
        Box::pin(async move {
            let (publisher, stream) = interface_stream_channel(4);
            tokio::spawn(async move {
                publisher.emit(StreamEvent(input.0)).await.unwrap();
                publisher
                    .finish(InterfaceStreamTerminal::Completed(Output(input.0 + 1)))
                    .await
                    .unwrap();
            });
            Ok(stream)
        })
    }
}

struct Authorization(&'static str);
impl InterfaceAuthorizationPort for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(self.0).unwrap()
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

fn actor() -> ActorContext {
    ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root")
}

fn interface_id(value: &str) -> InterfaceId {
    InterfaceId::new(value).unwrap()
}

fn identity(value: &str) -> InterfaceIdentity {
    InterfaceIdentity::new(interface_id(value), InterfaceVersion::new("1").unwrap())
}

fn contracts(mode: InterfaceExecutionMode) -> InterfaceContracts {
    let input = ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap();
    let output = ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap();
    let error =
        ContractIdentity::new(TargetError::CONTRACT_ID, TargetError::CONTRACT_VERSION).unwrap();
    match mode {
        InterfaceExecutionMode::Unary => InterfaceContracts::unary(input, output, error),
        InterfaceExecutionMode::ServerStream => InterfaceContracts::server_stream(
            input,
            ContractIdentity::new(StreamEvent::CONTRACT_ID, StreamEvent::CONTRACT_VERSION).unwrap(),
            output,
            error,
        ),
        InterfaceExecutionMode::AsyncAck => unreachable!(),
    }
}

fn definition(value: &str, mode: InterfaceExecutionMode) -> InterfaceDefinition {
    InterfaceDefinition::new(
        identity(value),
        contracts(mode),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            AuthorizationOperation::new("review.invoke").unwrap(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            mode,
            HandlerReference::new(format!("{value}.handler")).unwrap(),
            TargetReference::new(format!("{value}.target")).unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        InterfaceOwner::new("review.owner").unwrap(),
    )
}

fn plan(authn: &str, authz: &str) -> InvocationAdapterPlan {
    InvocationAdapterPlan::new(
        AuthenticationAdapterReference::new(authn).unwrap(),
        AuthorizationAdapterReference::new(authz).unwrap(),
        None::<AdmissionAdapterReference>,
    )
}

fn compiler() -> RegistryCompiler {
    RegistryCompiler::new(
        GraphFingerprint::new("graph:review").unwrap(),
        [AuthorizationOperation::new("review.invoke").unwrap()],
        [InterfaceOwner::new("review.owner").unwrap()],
    )
}

fn envelope(binding: &str, protocol: InterfaceProtocol, authn: &str) -> InvocationEnvelope<Input> {
    InvocationEnvelope::new(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new(binding).unwrap(),
        protocol,
        AuthenticationAdapterReference::new(authn).unwrap(),
        actor(),
        None,
        Input(4),
    )
}

#[tokio::test]
async fn review_binding_identity_drives_resolve_and_rejects_protocol_or_adapter_mismatch() {
    let mut compiler = compiler();
    let definition = definition("review.multi", InterfaceExecutionMode::Unary);
    compiler.register_definition(definition.clone()).unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.multi.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("GET", "/api/review").unwrap()),
            ),
            plan("review.http-authn", "review.authz"),
        )
        .unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("mcp.review.multi.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::mcp("review_tool"),
            ),
            plan("review.mcp-authn", "review.authz"),
        )
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.multi.handler").unwrap(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let kernel = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")));

    let outcome = kernel
        .invoke::<Input, Output, TargetError>(
            Arc::clone(&snapshot),
            envelope(
                "mcp.review.multi.v1",
                InterfaceProtocol::Mcp,
                "review.mcp-authn",
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        outcome.receipt().interface_id(),
        Some(definition.interface_id())
    );
    assert_eq!(
        outcome.receipt().resolved().unwrap().binding_id().as_str(),
        "mcp.review.multi.v1"
    );

    let wrong_protocol = kernel
        .invoke::<Input, Output, TargetError>(
            Arc::clone(&snapshot),
            envelope(
                "mcp.review.multi.v1",
                InterfaceProtocol::Http,
                "review.mcp-authn",
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        wrong_protocol.error(),
        InterfaceInvocationError::ProtocolBindingMismatch
    ));

    let wrong_authn = kernel
        .invoke::<Input, Output, TargetError>(
            Arc::clone(&snapshot),
            envelope(
                "mcp.review.multi.v1",
                InterfaceProtocol::Mcp,
                "review.http-authn",
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        wrong_authn.error(),
        InterfaceInvocationError::AuthenticationAdapterMismatch
    ));

    let unknown_binding = kernel
        .invoke::<Input, Output, TargetError>(
            snapshot,
            envelope(
                "mcp.review.unknown.v1",
                InterfaceProtocol::Mcp,
                "review.mcp-authn",
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        unknown_binding.error(),
        InterfaceInvocationError::UnknownBinding
    ));
}

#[test]
fn review_registry_compiles_ordered_extension_plan_from_real_registrations() {
    let mut compiler = compiler();
    let definition = definition("review.extensions", InterfaceExecutionMode::Unary);
    compiler.register_definition(definition.clone()).unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.extensions.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("GET", "/api/extensions").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            20,
            InterfaceExtensionRegistration::new(
                PluginIdentity::new("review.after").unwrap(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::After,
                InterfaceExtensionPermission::ObserveOutput,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::TypedOutput],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                PluginIdentity::new("review.before").unwrap(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Before,
                InterfaceExtensionPermission::ObserveInput,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::TypedInput],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.extensions.handler").unwrap(),
            Arc::new(UnaryHandler),
        )
        .unwrap();

    let snapshot = compiler.compile().unwrap();
    let plan = snapshot
        .plan(&BindingId::new("http.review.extensions.v1").unwrap())
        .unwrap();
    assert_eq!(plan.extension_plan().registrations().len(), 2);
    assert_eq!(plan.extension_plan().registrations()[0].order(), 10);
    assert!(plan
        .extension_plan()
        .fingerprint()
        .as_str()
        .starts_with("sha256:"));
}

#[tokio::test]
async fn review_live_server_stream_finishes_after_events_with_one_runtime_pinned_terminal() {
    let mut compiler = compiler();
    let definition = definition("review.stream", InterfaceExecutionMode::ServerStream);
    compiler.register_definition(definition.clone()).unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.stream.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/stream").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.stream.handler").unwrap(),
            Arc::new(StreamingHandler),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let target = ExecutionTargetPin::Runtime {
        handler: HandlerReference::new("review.stream.handler").unwrap(),
        target: TargetReference::new("review.stream.target").unwrap(),
        plugin: PluginIdentity::new("review.runtime").unwrap(),
        artifact: ArtifactIdentity::new("review.artifact.v1").unwrap(),
        runtime: RuntimeTargetIdentity::new("review.runtime.target").unwrap(),
        runtime_generation: RuntimeGeneration::new("review.runtime.generation.1").unwrap(),
        worker_generation: WorkerGeneration::new("review.worker.generation.1").unwrap(),
    };
    let invocation = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")))
        .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
            snapshot,
            envelope(
                "http.review.stream.v1",
                InterfaceProtocol::Http,
                "review.authn",
            ),
            target.clone(),
        )
        .await
        .unwrap();
    let (mut events, completion) = invocation.into_parts();
    assert_eq!(events.recv().await, Some(StreamEvent(4)));
    assert!(events.recv().await.is_none());
    let terminal = completion.complete().await.unwrap();
    assert!(matches!(
        terminal.terminal(),
        InterfaceStreamTerminal::Completed(Output(5))
    ));
    assert_eq!(terminal.receipt().attempt().unwrap().target(), &target);
    assert_eq!(
        terminal.receipt().stages().collect::<Vec<_>>(),
        vec![
            InterfaceInvocationStage::Received,
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::PrincipalEstablished,
            InterfaceInvocationStage::Authorized,
            InterfaceInvocationStage::Admitted,
            InterfaceInvocationStage::Prepared,
            InterfaceInvocationStage::Dispatched,
            InterfaceInvocationStage::Executing,
            InterfaceInvocationStage::PostProcessed,
        ]
    );
}
