use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use domain::ActorContext;
use uuid::Uuid;

use crate::{
    interface_stream_channel, ActivatedAuthenticationAdapter, AdmissionAdapterReference,
    ArtifactIdentity, AuthenticationActivationIdentity, AuthenticationAdapterReference,
    AuthorizationAdapterReference, AuthorizationOperation, BindingId, ContractIdentity,
    ContributedProtocolBinding, ExecutionTargetPin, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAdmissionContribution, InterfaceAdmissionContributionError,
    InterfaceAdmissionContributionFuture, InterfaceAdmissionContributionRequest,
    InterfaceAfterHook, InterfaceAfterHookFuture, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationContribution,
    InterfaceAuthorizationContributionError, InterfaceAuthorizationContributionFuture,
    InterfaceAuthorizationContributionRequest, InterfaceAuthorizationFuture,
    InterfaceAuthorizationPort, InterfaceAuthorizationRequest, InterfaceBeforeHook,
    InterfaceBeforeHookFuture, InterfaceCompletionHook, InterfaceCompletionHookFuture,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceExtensionFact,
    InterfaceExtensionIsolation, InterfaceExtensionPermission, InterfaceExtensionPoint,
    InterfaceExtensionRegistration, InterfaceExtensionTier, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceHookContext, InterfaceId,
    InterfaceIdentity, InterfaceInvocationError, InterfaceInvocationKernel,
    InterfaceInvocationStage, InterfaceInvocationTerminal, InterfaceLifecycle, InterfaceOwner,
    InterfaceProtocol, InterfaceScope, InterfaceStreamHandler, InterfaceStreamHandlerFuture,
    InterfaceStreamTerminal, InterfaceTargetAdmissionError, InterfaceTargetAdmissionFuture,
    InterfaceTargetAdmissionPort, InterfaceTargetAdmissionRequest, InterfaceVersion,
    InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage, PluginIdentity,
    PrincipalProfile, ProtocolBinding, ProtocolProjection, RegistryCompilationError,
    RegistryCompiler, RouteIdentity, RuntimeGeneration, RuntimeTargetIdentity, TargetReference,
    TypedInterfaceAdmissionPlan, TypedInterfaceAuthorizationPlan,
    TypedInterfaceDefinitionContribution, TypedInterfaceHookPlan, UserPrincipal, WorkerGeneration,
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
struct WrongInput;
impl InterfaceContract for WrongInput {
    const CONTRACT_ID: &'static str = "review-wrong-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WrongOutput;
impl InterfaceContract for WrongOutput {
    const CONTRACT_ID: &'static str = "review-wrong-output";
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

struct ContributedUnaryHandler;
impl InterfaceHandler<Input, Output, TargetError> for ContributedUnaryHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        Box::pin(async move { Ok(Output(input.0 + 40)) })
    }
}

struct RecordingBefore(Arc<Mutex<Vec<&'static str>>>);
impl InterfaceBeforeHook<Input> for RecordingBefore {
    fn before<'a>(
        &'a self,
        _context: InterfaceHookContext,
        _input: &'a mut Input,
    ) -> InterfaceBeforeHookFuture<'a> {
        self.0.lock().unwrap().push("before");
        Box::pin(async { Ok(()) })
    }
}

struct RecordingAfter(Arc<Mutex<Vec<&'static str>>>);
impl InterfaceAfterHook<Output> for RecordingAfter {
    fn after<'a>(
        &'a self,
        _context: InterfaceHookContext,
        _output: &'a Output,
    ) -> InterfaceAfterHookFuture<'a> {
        self.0.lock().unwrap().push("after");
        Box::pin(async {})
    }
}

struct RecordingCompletion(Arc<Mutex<Vec<&'static str>>>);
impl InterfaceCompletionHook for RecordingCompletion {
    fn completed(
        &self,
        _context: InterfaceHookContext,
        _terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        self.0.lock().unwrap().push("completion");
        Box::pin(async {})
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

fn activate_authentication(
    compiler: &mut RegistryCompiler,
    definition: &InterfaceDefinition,
    adapter: &str,
) {
    let plugin = PluginIdentity::new("review.authentication").unwrap();
    compiler
        .register_authentication_adapter(
            definition.interface_id(),
            1,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                definition.scope(),
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .unwrap(),
            ActivatedAuthenticationAdapter::new(
                plugin,
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new(adapter).unwrap(),
                AuthenticationActivationIdentity::new(format!("{adapter}.activation.v1")).unwrap(),
                definition.principal_profile(),
            ),
        )
        .unwrap();
}

fn envelope(binding: &str, protocol: InterfaceProtocol, authn: &str) -> InvocationEnvelope<Input> {
    InvocationEnvelope::new(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new(binding).unwrap(),
        protocol,
        AuthenticationAdapterReference::new(authn).unwrap(),
        AuthenticationActivationIdentity::new(format!("{authn}.activation.v1")).unwrap(),
        actor(),
        None,
        Input(4),
    )
}

#[test]
fn rr12_hook_contract_mismatch_fails_registry_compilation_for_unary_and_stream() {
    for (mode, wrong_input) in [
        (InterfaceExecutionMode::Unary, true),
        (InterfaceExecutionMode::Unary, false),
        (InterfaceExecutionMode::ServerStream, true),
        (InterfaceExecutionMode::ServerStream, false),
    ] {
        let mut compiler = compiler();
        let value = if mode == InterfaceExecutionMode::Unary {
            "review.hook-contract-unary"
        } else {
            "review.hook-contract-stream"
        };
        let definition = definition(value, mode);
        compiler.register_definition(definition.clone()).unwrap();
        activate_authentication(&mut compiler, &definition, "review.authn");
        compiler
            .register_binding(
                ProtocolBinding::new(
                    BindingId::new(format!("http.{value}.v1")).unwrap(),
                    definition.identity().clone(),
                    definition.contracts().clone(),
                    ProtocolProjection::http(
                        RouteIdentity::new("POST", format!("/api/{value}")).unwrap(),
                    ),
                ),
                plan("review.authn", "review.authz"),
            )
            .unwrap();
        if mode == InterfaceExecutionMode::Unary {
            compiler
                .bind_handler::<Input, Output, TargetError, UserPrincipal>(
                    definition.interface_id(),
                    definition.handler_reference().clone(),
                    Arc::new(UnaryHandler),
                )
                .unwrap();
        } else {
            compiler
                .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
                    definition.interface_id(),
                    definition.handler_reference().clone(),
                    Arc::new(StreamingHandler),
                )
                .unwrap();
        }
        if wrong_input {
            compiler
                .bind_hook_plan(
                    definition.interface_id(),
                    Arc::new(TypedInterfaceHookPlan::<WrongInput, Output>::new(
                        GraphFingerprint::new("graph:review").unwrap(),
                    )),
                )
                .unwrap();
        } else {
            compiler
                .bind_hook_plan(
                    definition.interface_id(),
                    Arc::new(TypedInterfaceHookPlan::<Input, WrongOutput>::new(
                        GraphFingerprint::new("graph:review").unwrap(),
                    )),
                )
                .unwrap();
        }
        assert!(matches!(
            compiler.compile(),
            Err(RegistryCompilationError::HookContractMismatch(_))
        ));
    }
}

#[tokio::test]
async fn review_binding_identity_drives_resolve_and_rejects_protocol_or_adapter_mismatch() {
    let mut compiler = compiler();
    let definition = definition("review.multi", InterfaceExecutionMode::Unary);
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.http-authn");
    compiler
        .bind_authentication_activation(
            definition.interface_id().clone(),
            ActivatedAuthenticationAdapter::new(
                PluginIdentity::new("review.authentication").unwrap(),
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("review.mcp-authn").unwrap(),
                AuthenticationActivationIdentity::new("review.mcp-authn.activation.v1").unwrap(),
                PrincipalProfile::User,
            ),
        )
        .unwrap();
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
    activate_authentication(&mut compiler, &definition, "review.authn");
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
    let events = Arc::new(Mutex::new(Vec::new()));
    compiler
        .bind_hook_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind_before(
                    PluginIdentity::new("review.before").unwrap(),
                    Arc::new(RecordingBefore(Arc::clone(&events))),
                )
                .bind_after(
                    PluginIdentity::new("review.after").unwrap(),
                    Arc::new(RecordingAfter(events)),
                ),
            ),
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
async fn review_compiled_completion_binding_is_mandatory_and_cannot_be_skipped_by_invoke() {
    let mut missing_compiler = compiler();
    let definition = definition("review.completion", InterfaceExecutionMode::Unary);
    missing_compiler
        .register_definition(definition.clone())
        .unwrap();
    activate_authentication(&mut missing_compiler, &definition, "review.authn");
    missing_compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.completion.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/completion").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    let plugin = PluginIdentity::new("review.completion-hook").unwrap();
    missing_compiler
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Completion,
                InterfaceExtensionPermission::ObserveCompletion,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::Terminal],
            )
            .unwrap(),
        )
        .unwrap();
    missing_compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.completion.handler").unwrap(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    let missing = missing_compiler.compile().unwrap_err();
    assert!(matches!(
        missing,
        RegistryCompilationError::MissingExecutableExtension(
            _,
            _,
            InterfaceExtensionPoint::Completion
        )
    ));

    let mut compiler = compiler();
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.completion.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/completion").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Completion,
                InterfaceExtensionPermission::ObserveCompletion,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::Terminal],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.completion.handler").unwrap(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    compiler
        .bind_hook_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind_completion(plugin, Arc::new(RecordingCompletion(Arc::clone(&events)))),
            ),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")))
        .invoke::<Input, Output, TargetError>(
            snapshot,
            envelope(
                "http.review.completion.v1",
                InterfaceProtocol::Http,
                "review.authn",
            ),
        )
        .await
        .unwrap();
    assert_eq!(events.lock().unwrap().as_slice(), ["completion"]);
}

fn handler_extension_compiler(plugins: &[&str], bind_executables: bool) -> RegistryCompiler {
    let mut compiler = compiler();
    let definition = definition("review.handler-extension", InterfaceExecutionMode::Unary);
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.handler-extension.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(
                    RouteIdentity::new("POST", "/api/handler-extension").unwrap(),
                ),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    for (index, plugin) in plugins.iter().enumerate() {
        let plugin = PluginIdentity::new(plugin).unwrap();
        compiler
            .register_extension(
                definition.interface_id(),
                10 + index as u32,
                InterfaceExtensionRegistration::new(
                    plugin.clone(),
                    InterfaceExtensionTier::HostExtension,
                    InterfaceExtensionPoint::Handler,
                    InterfaceExtensionPermission::Handle,
                    InterfaceScope::Workspace,
                    InterfaceExtensionIsolation::TrustedInProcess,
                    [InterfaceExtensionFact::TypedInput],
                )
                .unwrap(),
            )
            .unwrap();
        if bind_executables {
            compiler
                .bind_extension_handler::<Input, Output, TargetError, UserPrincipal>(
                    definition.interface_id(),
                    plugin.clone(),
                    HandlerReference::new(format!("{}.invoke", plugin.as_str())).unwrap(),
                    TargetReference::new(format!("{}.target", plugin.as_str())).unwrap(),
                    Arc::new(ContributedUnaryHandler),
                )
                .unwrap();
        }
    }
    compiler
}

#[tokio::test]
async fn review_host_extension_handler_is_the_real_exactly_one_effective_target() {
    let missing = handler_extension_compiler(&["review.handler-missing"], false)
        .compile()
        .unwrap_err();
    assert!(matches!(
        missing,
        RegistryCompilationError::MissingExecutableExtension(
            _,
            _,
            InterfaceExtensionPoint::Handler
        )
    ));
    let multiple = handler_extension_compiler(&["review.handler-one", "review.handler-two"], true)
        .compile()
        .unwrap_err();
    assert!(matches!(
        multiple,
        RegistryCompilationError::Extension(
            crate::InterfaceExtensionCompilationError::MultipleEffectiveHandlers(_)
        )
    ));
    let compiler = handler_extension_compiler(&["review.handler-plugin"], true);
    let snapshot = compiler.compile().unwrap();
    let outcome = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")))
        .invoke::<Input, Output, TargetError>(
            snapshot,
            envelope(
                "http.review.handler-extension.v1",
                InterfaceProtocol::Http,
                "review.authn",
            ),
        )
        .await
        .unwrap();
    assert_eq!(outcome.value(), &Output(44));
}

#[tokio::test]
async fn review_live_server_stream_finishes_after_events_with_one_runtime_pinned_terminal() {
    let mut compiler = compiler();
    let definition = definition("review.stream", InterfaceExecutionMode::ServerStream);
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
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
    for (order, plugin, point, permission, facts) in [
        (
            10,
            "review.stream-before",
            InterfaceExtensionPoint::Before,
            InterfaceExtensionPermission::ObserveInput,
            vec![InterfaceExtensionFact::TypedInput],
        ),
        (
            20,
            "review.stream-after",
            InterfaceExtensionPoint::After,
            InterfaceExtensionPermission::ObserveOutput,
            vec![InterfaceExtensionFact::TypedOutput],
        ),
        (
            30,
            "review.stream-completion",
            InterfaceExtensionPoint::Completion,
            InterfaceExtensionPermission::ObserveCompletion,
            vec![InterfaceExtensionFact::Terminal],
        ),
    ] {
        compiler
            .register_extension(
                definition.interface_id(),
                order,
                InterfaceExtensionRegistration::new(
                    PluginIdentity::new(plugin).unwrap(),
                    InterfaceExtensionTier::HostExtension,
                    point,
                    permission,
                    InterfaceScope::Workspace,
                    InterfaceExtensionIsolation::TrustedInProcess,
                    facts,
                )
                .unwrap(),
            )
            .unwrap();
    }
    compiler
        .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            HandlerReference::new("review.stream.handler").unwrap(),
            Arc::new(StreamingHandler),
        )
        .unwrap();
    let lifecycle = Arc::new(Mutex::new(Vec::new()));
    compiler
        .bind_hook_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind_before(
                    PluginIdentity::new("review.stream-before").unwrap(),
                    Arc::new(RecordingBefore(Arc::clone(&lifecycle))),
                )
                .bind_after(
                    PluginIdentity::new("review.stream-after").unwrap(),
                    Arc::new(RecordingAfter(Arc::clone(&lifecycle))),
                )
                .bind_completion(
                    PluginIdentity::new("review.stream-completion").unwrap(),
                    Arc::new(RecordingCompletion(Arc::clone(&lifecycle))),
                ),
            ),
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
    lifecycle.lock().unwrap().push("event");
    assert!(events.recv().await.is_none());
    let terminal = completion.complete().await.unwrap();
    assert!(matches!(
        terminal.terminal(),
        InterfaceStreamTerminal::Completed(Output(5))
    ));
    assert_eq!(terminal.receipt().attempt().unwrap().target(), &target);
    assert_eq!(
        lifecycle.lock().unwrap().as_slice(),
        ["before", "event", "after", "completion"]
    );
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

#[test]
fn rr13_definition_contribution_is_real_registry_input_and_metadata_only_fails() {
    let contributed = definition("review.contributed", InterfaceExecutionMode::Unary);
    let binding = ProtocolBinding::new(
        BindingId::new("http.review.contributed.v1").unwrap(),
        contributed.identity().clone(),
        contributed.contracts().clone(),
        ProtocolProjection::http(RouteIdentity::new("POST", "/api/contributed").unwrap()),
    );
    let plugin = PluginIdentity::new("review.definition-contributor").unwrap();
    let registration = InterfaceExtensionRegistration::new(
        plugin,
        InterfaceExtensionTier::HostExtension,
        InterfaceExtensionPoint::Definition,
        InterfaceExtensionPermission::Define,
        InterfaceScope::Workspace,
        InterfaceExtensionIsolation::TrustedInProcess,
        [
            InterfaceExtensionFact::DefinitionIdentity,
            InterfaceExtensionFact::BindingIdentity,
        ],
    )
    .unwrap();
    let contribution =
        TypedInterfaceDefinitionContribution::<Input, Output, TargetError, UserPrincipal>::new(
            contributed.clone(),
            [ContributedProtocolBinding::new(
                binding,
                plan("review.authn", "review.authz"),
            )],
        )
        .unwrap();
    let mut compiler = compiler();
    compiler.register_definition_contribution(0, registration.clone(), Arc::new(contribution));
    activate_authentication(&mut compiler, &contributed, "review.authn");
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            contributed.interface_id(),
            contributed.handler_reference().clone(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    assert_eq!(snapshot.definitions().len(), 1);
    assert_eq!(snapshot.bindings().len(), 1);

    let inert = definition("review.inert-definition", InterfaceExecutionMode::Unary);
    let mut compiler = compiler();
    compiler.register_definition(inert.clone()).unwrap();
    activate_authentication(&mut compiler, &inert, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.inert-definition.v1").unwrap(),
                inert.identity().clone(),
                inert.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/inert").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .register_extension(inert.interface_id(), 0, registration)
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            inert.interface_id(),
            inert.handler_reference().clone(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    assert!(matches!(
        compiler.compile(),
        Err(RegistryCompilationError::MissingDefinitionContribution(_))
    ));
}

#[test]
fn rr14_authentication_activation_missing_duplicate_and_identity_mismatch_fail_closed() {
    let definition = definition(
        "review.authentication-negative",
        InterfaceExecutionMode::Unary,
    );
    let mut missing = compiler();
    missing.register_definition(definition.clone()).unwrap();
    missing
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.authentication-negative.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/auth-negative").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    missing
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            definition.handler_reference().clone(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    assert!(matches!(
        missing.compile(),
        Err(RegistryCompilationError::MissingAuthenticationActivation(_))
    ));

    let mut mismatch = compiler();
    mismatch.register_definition(definition.clone()).unwrap();
    mismatch
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.authentication-negative.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/auth-negative").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    let registration = InterfaceExtensionRegistration::new(
        PluginIdentity::new("review.authentication").unwrap(),
        InterfaceExtensionTier::BuiltIn,
        InterfaceExtensionPoint::AuthenticationAdapter,
        InterfaceExtensionPermission::Authenticate,
        InterfaceScope::Workspace,
        InterfaceExtensionIsolation::TrustedInProcess,
        [],
    )
    .unwrap();
    mismatch
        .register_authentication_adapter(
            definition.interface_id(),
            1,
            registration,
            ActivatedAuthenticationAdapter::new(
                PluginIdentity::new("review.wrong-authentication").unwrap(),
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("review.authn").unwrap(),
                AuthenticationActivationIdentity::new("review.authn.activation.v1").unwrap(),
                PrincipalProfile::User,
            ),
        )
        .unwrap();
    assert!(matches!(
        mismatch.bind_authentication_activation(
            definition.interface_id().clone(),
            ActivatedAuthenticationAdapter::new(
                PluginIdentity::new("review.wrong-authentication").unwrap(),
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("review.authn").unwrap(),
                AuthenticationActivationIdentity::new("review.duplicate.activation.v1").unwrap(),
                PrincipalProfile::User,
            )
        ),
        Err(RegistryCompilationError::DuplicateAuthenticationActivation(
            _
        ))
    ));
    mismatch
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            definition.handler_reference().clone(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    assert!(matches!(
        mismatch.compile(),
        Err(RegistryCompilationError::AuthenticationActivationMismatch(
            _
        ))
    ));
}

#[derive(Clone, Copy)]
enum DecisionBehavior {
    Allow,
    Reject,
    Delay,
}

struct RecordingAuthorizationContribution {
    events: Arc<Mutex<Vec<&'static str>>>,
    behavior: DecisionBehavior,
}

impl InterfaceAuthorizationContribution for RecordingAuthorizationContribution {
    fn authorize(
        &self,
        _request: InterfaceAuthorizationContributionRequest,
    ) -> InterfaceAuthorizationContributionFuture<'_> {
        self.events.lock().unwrap().push("extension-authorization");
        Box::pin(async move {
            match self.behavior {
                DecisionBehavior::Allow => Ok(()),
                DecisionBehavior::Reject => {
                    Err(InterfaceAuthorizationContributionError::classified(
                        "extension-authorization-deny",
                    ))
                }
                DecisionBehavior::Delay => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    Ok(())
                }
            }
        })
    }
}

struct RecordingAdmissionContribution {
    events: Arc<Mutex<Vec<&'static str>>>,
    behavior: DecisionBehavior,
}

impl InterfaceAdmissionContribution for RecordingAdmissionContribution {
    fn admit(
        &self,
        _request: InterfaceAdmissionContributionRequest,
    ) -> InterfaceAdmissionContributionFuture<'_> {
        self.events.lock().unwrap().push("extension-admission");
        Box::pin(async move {
            match self.behavior {
                DecisionBehavior::Allow => Ok(()),
                DecisionBehavior::Reject => Err(InterfaceAdmissionContributionError::classified(
                    "extension-admission-reject",
                )),
                DecisionBehavior::Delay => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    Ok(())
                }
            }
        })
    }
}

struct RecordingCoreAuthorization {
    events: Arc<Mutex<Vec<&'static str>>>,
    reject: bool,
}

impl InterfaceAuthorizationPort for RecordingCoreAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("review.authz").unwrap()
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        self.events.lock().unwrap().push("core-authorization");
        Box::pin(async move {
            if self.reject {
                Err(crate::InterfaceAuthorizationError::classified("core-deny"))
            } else {
                Ok(())
            }
        })
    }
}

struct RecordingCoreAdmission(Arc<Mutex<Vec<&'static str>>>);

impl InterfaceTargetAdmissionPort for RecordingCoreAdmission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("review.admission").unwrap()
    }

    fn admit(
        &self,
        _request: InterfaceTargetAdmissionRequest,
    ) -> InterfaceTargetAdmissionFuture<'_> {
        self.0.lock().unwrap().push("core-admission");
        Box::pin(async { Ok(()) })
    }
}

struct DecisionUnaryHandler(Arc<Mutex<Vec<&'static str>>>);

impl InterfaceHandler<Input, Output, TargetError> for DecisionUnaryHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        self.0.lock().unwrap().push("handler");
        Box::pin(async move { Ok(Output(input.0)) })
    }
}

struct DecisionStreamHandler(Arc<Mutex<Vec<&'static str>>>);

impl InterfaceStreamHandler<Input, StreamEvent, Output, TargetError> for DecisionStreamHandler {
    fn invoke_stream(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceStreamHandlerFuture<StreamEvent, Output, TargetError> {
        self.0.lock().unwrap().push("handler");
        Box::pin(async move {
            let (publisher, stream) = interface_stream_channel(2);
            tokio::spawn(async move {
                publisher.emit(StreamEvent(input.0)).await.unwrap();
                publisher
                    .finish(InterfaceStreamTerminal::Completed(Output(input.0)))
                    .await
                    .unwrap();
            });
            Ok(stream)
        })
    }
}

fn decision_snapshot(
    mode: InterfaceExecutionMode,
    events: Arc<Mutex<Vec<&'static str>>>,
    authorization: DecisionBehavior,
    admission: DecisionBehavior,
) -> Arc<crate::CompiledInterfaceRegistry> {
    let value = if mode == InterfaceExecutionMode::Unary {
        "review.decision-unary"
    } else {
        "review.decision-stream"
    };
    let definition = definition(value, mode);
    let mut compiler = compiler();
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new(format!("http.{value}.v1")).unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(
                    RouteIdentity::new("POST", format!("/api/{value}")).unwrap(),
                ),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new("review.authn").unwrap(),
                AuthorizationAdapterReference::new("review.authz").unwrap(),
                Some(AdmissionAdapterReference::new("review.admission").unwrap()),
            ),
        )
        .unwrap();
    let authorization_plugin = PluginIdentity::new("review.authorization-veto").unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                authorization_plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Authorization,
                InterfaceExtensionPermission::Authorize,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [
                    InterfaceExtensionFact::DefinitionIdentity,
                    InterfaceExtensionFact::PrincipalSummary,
                ],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_authorization_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAuthorizationPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    authorization_plugin,
                    Arc::new(RecordingAuthorizationContribution {
                        events: Arc::clone(&events),
                        behavior: authorization,
                    }),
                ),
            ),
        )
        .unwrap();
    let admission_plugin = PluginIdentity::new("review.admission-veto").unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            20,
            InterfaceExtensionRegistration::new(
                admission_plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Admission,
                InterfaceExtensionPermission::Admit,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [
                    InterfaceExtensionFact::DefinitionIdentity,
                    InterfaceExtensionFact::PrincipalSummary,
                    InterfaceExtensionFact::AuthorizationDecision,
                ],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_admission_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAdmissionPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    admission_plugin,
                    Arc::new(RecordingAdmissionContribution {
                        events: Arc::clone(&events),
                        behavior: admission,
                    }),
                ),
            ),
        )
        .unwrap();
    let before_plugin = PluginIdentity::new("review.decision-before").unwrap();
    compiler
        .register_extension(
            definition.interface_id(),
            30,
            InterfaceExtensionRegistration::new(
                before_plugin.clone(),
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
        .bind_hook_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind_before(
                    before_plugin,
                    Arc::new(RecordingBefore(Arc::clone(&events))),
                ),
            ),
        )
        .unwrap();
    if mode == InterfaceExecutionMode::Unary {
        compiler
            .bind_handler::<Input, Output, TargetError, UserPrincipal>(
                definition.interface_id(),
                definition.handler_reference().clone(),
                Arc::new(DecisionUnaryHandler(events)),
            )
            .unwrap();
    } else {
        compiler
            .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
                definition.interface_id(),
                definition.handler_reference().clone(),
                Arc::new(DecisionStreamHandler(events)),
            )
            .unwrap();
    }
    compiler.compile().unwrap()
}

#[tokio::test]
async fn rr15_rr16_unary_and_stream_execute_core_then_ordered_veto_then_hook_handler() {
    for mode in [
        InterfaceExecutionMode::Unary,
        InterfaceExecutionMode::ServerStream,
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let snapshot = decision_snapshot(
            mode,
            Arc::clone(&events),
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
        );
        let kernel = InterfaceInvocationKernel::with_target_admission(
            Arc::new(RecordingCoreAuthorization {
                events: Arc::clone(&events),
                reject: false,
            }),
            Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
        );
        let binding = if mode == InterfaceExecutionMode::Unary {
            "http.review.decision-unary.v1"
        } else {
            "http.review.decision-stream.v1"
        };
        if mode == InterfaceExecutionMode::Unary {
            let outcome = kernel
                .invoke::<Input, Output, TargetError>(
                    Arc::clone(&snapshot),
                    envelope(binding, InterfaceProtocol::Http, "review.authn"),
                )
                .await
                .unwrap();
            assert!(outcome.receipt().authorization_decision().is_some());
        } else {
            let invocation = kernel
                .invoke_server_stream_with_dispatch_target::<
                    Input,
                    StreamEvent,
                    Output,
                    TargetError,
                >(
                    snapshot,
                    envelope(binding, InterfaceProtocol::Http, "review.authn"),
                    ExecutionTargetPin::BuiltIn {
                        handler: HandlerReference::new("review.decision-stream.handler").unwrap(),
                        target: TargetReference::new("review.decision-stream.target").unwrap(),
                    },
                )
                .await
                .unwrap();
            let (mut stream, completion) = invocation.into_parts();
            assert_eq!(stream.recv().await, Some(StreamEvent(4)));
            assert!(stream.recv().await.is_none());
            assert!(completion
                .complete()
                .await
                .unwrap()
                .receipt()
                .authorization_decision()
                .is_some());
        }
        assert_eq!(
            events.lock().unwrap().as_slice(),
            [
                "core-authorization",
                "extension-authorization",
                "core-admission",
                "extension-admission",
                "before",
                "handler",
            ]
        );
    }
}

#[tokio::test]
async fn rr15_core_deny_dominates_plugin_allow_and_extension_failures_fail_closed() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = decision_snapshot(
        InterfaceExecutionMode::Unary,
        Arc::clone(&events),
        DecisionBehavior::Allow,
        DecisionBehavior::Allow,
    );
    let failure = InterfaceInvocationKernel::with_target_admission(
        Arc::new(RecordingCoreAuthorization {
            events: Arc::clone(&events),
            reject: true,
        }),
        Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
    )
    .invoke::<Input, Output, TargetError>(
        snapshot,
        envelope(
            "http.review.decision-unary.v1",
            InterfaceProtocol::Http,
            "review.authn",
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::AuthorizationRejected(_)
    ));
    assert_eq!(events.lock().unwrap().as_slice(), ["core-authorization"]);

    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = decision_snapshot(
        InterfaceExecutionMode::Unary,
        Arc::clone(&events),
        DecisionBehavior::Reject,
        DecisionBehavior::Allow,
    );
    let failure = InterfaceInvocationKernel::with_target_admission(
        Arc::new(RecordingCoreAuthorization {
            events: Arc::clone(&events),
            reject: false,
        }),
        Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
    )
    .invoke::<Input, Output, TargetError>(
        snapshot,
        envelope(
            "http.review.decision-unary.v1",
            InterfaceProtocol::Http,
            "review.authn",
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::AuthorizationContributionRejected(_)
    ));
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["core-authorization", "extension-authorization"]
    );
}

#[tokio::test]
async fn rr15_rr16_extension_timeout_and_admission_reject_fail_closed() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = decision_snapshot(
        InterfaceExecutionMode::Unary,
        Arc::clone(&events),
        DecisionBehavior::Delay,
        DecisionBehavior::Allow,
    );
    let envelope = InvocationEnvelope::new(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("http.review.decision-unary.v1").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("review.authn").unwrap(),
        AuthenticationActivationIdentity::new("review.authn.activation.v1").unwrap(),
        actor(),
        Some(SystemTime::now() + Duration::from_millis(10)),
        Input(4),
    );
    let failure = InterfaceInvocationKernel::with_target_admission(
        Arc::new(RecordingCoreAuthorization {
            events: Arc::clone(&events),
            reject: false,
        }),
        Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
    )
    .invoke::<Input, Output, TargetError>(snapshot, envelope)
    .await
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::DeadlineElapsed
    ));

    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = decision_snapshot(
        InterfaceExecutionMode::Unary,
        Arc::clone(&events),
        DecisionBehavior::Allow,
        DecisionBehavior::Reject,
    );
    let failure = InterfaceInvocationKernel::with_target_admission(
        Arc::new(RecordingCoreAuthorization {
            events: Arc::clone(&events),
            reject: false,
        }),
        Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
    )
    .invoke::<Input, Output, TargetError>(
        snapshot,
        envelope(
            "http.review.decision-unary.v1",
            InterfaceProtocol::Http,
            "review.authn",
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::AdmissionContributionRejected(_)
    ));
}

fn bare_decision_compiler(value: &str) -> (RegistryCompiler, InterfaceDefinition) {
    let definition = definition(value, InterfaceExecutionMode::Unary);
    let mut compiler = compiler();
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new(format!("http.{value}.v1")).unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(
                    RouteIdentity::new("POST", format!("/api/{value}")).unwrap(),
                ),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            definition.interface_id(),
            definition.handler_reference().clone(),
            Arc::new(UnaryHandler),
        )
        .unwrap();
    (compiler, definition)
}

#[test]
fn rr15_rr16_decision_bindings_fail_publish_when_missing_extra_or_contract_mismatched() {
    let (mut missing, definition) = bare_decision_compiler("review.authz-missing");
    let plugin = PluginIdentity::new("review.authz-missing-plugin").unwrap();
    missing
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                plugin,
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Authorization,
                InterfaceExtensionPermission::Authorize,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::PrincipalSummary],
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        missing.compile(),
        Err(RegistryCompilationError::MissingExecutableExtension(
            _,
            _,
            InterfaceExtensionPoint::Authorization
        ))
    ));

    let (mut extra, definition) = bare_decision_compiler("review.admission-extra");
    extra
        .bind_admission_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAdmissionPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    PluginIdentity::new("review.admission-extra-plugin").unwrap(),
                    Arc::new(RecordingAdmissionContribution {
                        events: Arc::new(Mutex::new(Vec::new())),
                        behavior: DecisionBehavior::Allow,
                    }),
                ),
            ),
        )
        .unwrap();
    assert!(matches!(
        extra.compile(),
        Err(RegistryCompilationError::UnexpectedExecutableExtension(
            _,
            _,
            InterfaceExtensionPoint::Admission
        ))
    ));

    let (mut mismatch, definition) = bare_decision_compiler("review.authz-contract");
    let plugin = PluginIdentity::new("review.authz-contract-plugin").unwrap();
    mismatch
        .register_extension(
            definition.interface_id(),
            10,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Authorization,
                InterfaceExtensionPermission::Authorize,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::PrincipalSummary],
            )
            .unwrap(),
        )
        .unwrap();
    mismatch
        .bind_authorization_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAuthorizationPlan::<WrongInput, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    plugin,
                    Arc::new(RecordingAuthorizationContribution {
                        events: Arc::new(Mutex::new(Vec::new())),
                        behavior: DecisionBehavior::Allow,
                    }),
                ),
            ),
        )
        .unwrap();
    assert!(matches!(
        mismatch.compile(),
        Err(RegistryCompilationError::DecisionContractMismatch(
            _,
            InterfaceExtensionPoint::Authorization
        ))
    ));
}
