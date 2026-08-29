use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use domain::ActorContext;
use uuid::Uuid;

use crate::{
    AdmissionAdapterReference, ArtifactIdentity, AuthenticationAdapterReference,
    AuthorizationAdapterReference, AuthorizationOperation, BindingId, ContractIdentity,
    DynamicInterfaceRegistry, ExecutionTargetPin, GraphFingerprint, HandlerReference,
    IdempotencyKey, InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationError, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceBeforeHook, InterfaceBeforeHookFuture,
    InterfaceCompletionHook, InterfaceCompletionHookFuture, InterfaceContract, InterfaceContracts,
    InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceHookContext,
    InterfaceId, InterfaceIdentity, InterfaceInvocationError, InterfaceInvocationKernel,
    InterfaceInvocationStage, InterfaceInvocationTerminal, InterfaceLifecycle, InterfaceOwner,
    InterfaceProtocol, InterfaceScope, InterfaceStreamAccumulator, InterfaceStreamStateError,
    InterfaceStreamTerminal, InterfaceTargetAdmissionError, InterfaceTargetAdmissionFuture,
    InterfaceTargetAdmissionPort, InterfaceTargetAdmissionRequest, InterfaceTargetFailure,
    InterfaceVersion, InvocationAdapterPlan, InvocationCancellation, InvocationControls,
    InvocationEnvelope, InvocationId, InvocationLineage, InvocationLineageError, PluginIdentity,
    PrincipalProfile, ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity,
    RuntimeGeneration, RuntimeTargetIdentity, TargetReference, TypedInterfaceHookPlan,
    UserPrincipal, WorkerGeneration,
};

#[derive(Clone)]
struct Input(u8);
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "invocation-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Debug, PartialEq, Eq)]
struct Output(u8);
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "invocation-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Debug, PartialEq, Eq)]
struct TargetError;
impl InterfaceContract for TargetError {
    const CONTRACT_ID: &'static str = "invocation-target-error";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Debug, PartialEq, Eq)]
struct StreamEvent(u8);
impl InterfaceContract for StreamEvent {
    const CONTRACT_ID: &'static str = "invocation-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

struct RecordingHandler {
    increment: u8,
    seen_fingerprint: Arc<Mutex<Option<String>>>,
    fail: bool,
}

impl InterfaceHandler<Input, Output, TargetError> for RecordingHandler {
    fn invoke(
        &self,
        context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        let increment = self.increment;
        let seen_fingerprint = Arc::clone(&self.seen_fingerprint);
        let fingerprint = context.registry_fingerprint().as_str().to_string();
        let fail = self.fail;
        Box::pin(async move {
            *seen_fingerprint.lock().unwrap() = Some(fingerprint);
            if fail {
                Err(InterfaceTargetFailure::new("target_failed", TargetError))
            } else {
                Ok(Output(input.0 + increment))
            }
        })
    }
}

struct Authorization {
    reject: bool,
}

impl InterfaceAuthorizationPort for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("test.authz").unwrap()
    }

    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let reject = self.reject || !request.principal().actor().is_root;
        Box::pin(async move {
            if reject {
                Err(InterfaceAuthorizationError::classified("permission_denied"))
            } else {
                Ok(())
            }
        })
    }
}

struct Admission {
    reject: bool,
}

struct SlowAuthorization;

impl InterfaceAuthorizationPort for SlowAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("test.authz").unwrap()
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        })
    }
}

struct SlowAdmission;

impl InterfaceTargetAdmissionPort for SlowAdmission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("test.admission").unwrap()
    }

    fn admit(
        &self,
        _request: InterfaceTargetAdmissionRequest,
    ) -> InterfaceTargetAdmissionFuture<'_> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        })
    }
}

struct SlowHandler;

impl InterfaceHandler<Input, Output, TargetError> for SlowHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(Output(input.0))
        })
    }
}

impl InterfaceTargetAdmissionPort for Admission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("test.admission").unwrap()
    }

    fn admit(
        &self,
        _request: InterfaceTargetAdmissionRequest,
    ) -> InterfaceTargetAdmissionFuture<'_> {
        let reject = self.reject;
        Box::pin(async move {
            if reject {
                Err(InterfaceTargetAdmissionError::classified(
                    "target_not_admitted",
                ))
            } else {
                Ok(())
            }
        })
    }
}

struct RecordingBeforeHook {
    name: &'static str,
    increment: u8,
    events: Arc<Mutex<Vec<String>>>,
}

impl InterfaceBeforeHook<Input> for RecordingBeforeHook {
    fn before<'a>(
        &'a self,
        _context: InterfaceHookContext,
        input: &'a mut Input,
    ) -> InterfaceBeforeHookFuture<'a> {
        Box::pin(async move {
            input.0 += self.increment;
            self.events
                .lock()
                .unwrap()
                .push(format!("before:{}", self.name));
            Ok(())
        })
    }
}

struct RecordingCompletionHook {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
}

impl InterfaceCompletionHook for RecordingCompletionHook {
    fn completed(
        &self,
        _context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.events
                .lock()
                .unwrap()
                .push(format!("completion:{}:{terminal:?}", self.name));
        })
    }
}

fn actor(root: bool) -> ActorContext {
    if root {
        ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root")
    } else {
        ActorContext::scoped(Uuid::now_v7(), Uuid::now_v7(), "member", [])
    }
}

fn interface_id() -> InterfaceId {
    InterfaceId::new("invocation.read").unwrap()
}

fn interface_identity() -> InterfaceIdentity {
    InterfaceIdentity::new(interface_id(), InterfaceVersion::new("1").unwrap())
}

fn operation() -> AuthorizationOperation {
    AuthorizationOperation::new("invocation.read").unwrap()
}

fn owner() -> InterfaceOwner {
    InterfaceOwner::new("core").unwrap()
}

fn contracts() -> InterfaceContracts {
    InterfaceContracts::unary(
        ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
        ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap(),
        ContractIdentity::new("invocation-target-error", "1").unwrap(),
    )
}

fn definition() -> InterfaceDefinition {
    InterfaceDefinition::new(
        interface_identity(),
        contracts(),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation(),
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new("invocation.handler").unwrap(),
            TargetReference::new("invocation.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner(),
    )
}

fn binding() -> ProtocolBinding {
    ProtocolBinding::new(
        BindingId::new("http.invocation.read.v1").unwrap(),
        interface_identity(),
        contracts(),
        ProtocolProjection::http(RouteIdentity::new("GET", "/api/console/invocation").unwrap()),
    )
}

fn compiler(graph: &str) -> RegistryCompiler {
    RegistryCompiler::new(
        GraphFingerprint::new(graph).unwrap(),
        [operation()],
        [owner()],
    )
}

fn adapter_plan() -> InvocationAdapterPlan {
    InvocationAdapterPlan::new(
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        AuthorizationAdapterReference::new("test.authz").unwrap(),
        Some(AdmissionAdapterReference::new("test.admission").unwrap()),
    )
}

fn compile_snapshot(
    graph: &str,
    increment: u8,
    fail: bool,
    seen_fingerprint: Arc<Mutex<Option<String>>>,
) -> Arc<crate::CompiledInterfaceRegistry> {
    let mut compiler = compiler(graph);
    compiler.register_definition(definition()).unwrap();
    compiler
        .register_binding(binding(), adapter_plan())
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(RecordingHandler {
                increment,
                seen_fingerprint,
                fail,
            }),
        )
        .unwrap();
    compiler.compile().unwrap()
}

fn envelope(actor: ActorContext) -> InvocationEnvelope<Input> {
    InvocationEnvelope::new(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("http.invocation.read.v1").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        actor,
        None,
        Input(2),
    )
}

#[tokio::test]
async fn executes_resolve_authorize_admit_invoke_and_records_terminal_receipt() {
    let seen = Arc::new(Mutex::new(None));
    let snapshot = compile_snapshot("graph:one", 3, false, Arc::clone(&seen));
    let kernel = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    );

    let outcome = kernel
        .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
        .await
        .unwrap();

    assert_eq!(outcome.value(), &Output(5));
    assert_eq!(
        outcome.receipt().stages().collect::<Vec<_>>(),
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
    assert_eq!(
        outcome.receipt().terminal(),
        InterfaceInvocationTerminal::Completed
    );
    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some(outcome.receipt().registry_fingerprint().as_str())
    );
}

#[tokio::test]
async fn rejects_authorization_admission_target_failure_and_elapsed_deadline() {
    let snapshot = compile_snapshot("graph:one", 0, false, Arc::new(Mutex::new(None)));
    let denied = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: true }))
        .invoke::<Input, Output, TargetError>(Arc::clone(&snapshot), envelope(actor(true)))
        .await
        .unwrap_err();
    assert!(matches!(
        denied.error(),
        InterfaceInvocationError::AuthorizationRejected(_)
    ));
    assert_eq!(
        denied.receipt().terminal(),
        InterfaceInvocationTerminal::Rejected
    );

    let admission = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: true }),
    )
    .invoke::<Input, Output, TargetError>(Arc::clone(&snapshot), envelope(actor(true)))
    .await
    .unwrap_err();
    assert!(matches!(
        admission.error(),
        InterfaceInvocationError::AdmissionRejected(_)
    ));

    let failed_snapshot = compile_snapshot("graph:failed", 0, true, Arc::new(Mutex::new(None)));
    let failed = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output, TargetError>(failed_snapshot, envelope(actor(true)))
        .await
        .unwrap_err();
    assert!(matches!(
        failed.error(),
        InterfaceInvocationError::TargetFailed(_)
    ));
    assert_eq!(
        failed.receipt().terminal(),
        InterfaceInvocationTerminal::Failed
    );

    let elapsed = InvocationEnvelope::new(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("http.invocation.read.v1").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        actor(true),
        Some(SystemTime::now() - Duration::from_secs(1)),
        Input(2),
    );
    let cancelled = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output, TargetError>(snapshot, elapsed)
        .await
        .unwrap_err();
    assert_eq!(
        cancelled.receipt().terminal(),
        InterfaceInvocationTerminal::Cancelled
    );
}

#[tokio::test]
async fn deadline_cancels_each_in_flight_stage() {
    let snapshot = compile_snapshot(
        "graph:slow-authorization",
        0,
        false,
        Arc::new(Mutex::new(None)),
    );
    let deadline = Some(SystemTime::now() + Duration::from_millis(10));
    let authorization = InterfaceInvocationKernel::new(Arc::new(SlowAuthorization))
        .invoke::<Input, Output, TargetError>(
            Arc::clone(&snapshot),
            InvocationEnvelope::new(
                InvocationLineage::root(InvocationId::now_v7()),
                BindingId::new("http.invocation.read.v1").unwrap(),
                InterfaceProtocol::Http,
                AuthenticationAdapterReference::new("test.authn").unwrap(),
                actor(true),
                deadline,
                Input(2),
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        authorization.error(),
        InterfaceInvocationError::DeadlineElapsed
    ));
    assert_eq!(
        authorization.receipt().terminal(),
        InterfaceInvocationTerminal::Cancelled
    );
    assert_eq!(
        authorization.receipt().stages().collect::<Vec<_>>(),
        vec![
            InterfaceInvocationStage::Received,
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::PrincipalEstablished,
        ]
    );

    let admission = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(SlowAdmission),
    )
    .invoke::<Input, Output, TargetError>(
        Arc::clone(&snapshot),
        InvocationEnvelope::new(
            InvocationLineage::root(InvocationId::now_v7()),
            BindingId::new("http.invocation.read.v1").unwrap(),
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new("test.authn").unwrap(),
            actor(true),
            Some(SystemTime::now() + Duration::from_millis(10)),
            Input(2),
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        admission.error(),
        InterfaceInvocationError::DeadlineElapsed
    ));
    assert_eq!(
        admission.receipt().terminal(),
        InterfaceInvocationTerminal::Cancelled
    );
    assert_eq!(
        admission.receipt().stages().collect::<Vec<_>>(),
        vec![
            InterfaceInvocationStage::Received,
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::PrincipalEstablished,
            InterfaceInvocationStage::Authorized,
        ]
    );

    let mut compiler = compiler("graph:slow-handler");
    compiler.register_definition(definition()).unwrap();
    compiler
        .register_binding(binding(), adapter_plan())
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(SlowHandler),
        )
        .unwrap();
    let handler = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output, TargetError>(
            compiler.compile().unwrap(),
            InvocationEnvelope::new(
                InvocationLineage::root(InvocationId::now_v7()),
                BindingId::new("http.invocation.read.v1").unwrap(),
                InterfaceProtocol::Http,
                AuthenticationAdapterReference::new("test.authn").unwrap(),
                actor(true),
                Some(SystemTime::now() + Duration::from_millis(10)),
                Input(2),
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        handler.error(),
        InterfaceInvocationError::DeadlineElapsed
    ));
    assert_eq!(
        handler.receipt().terminal(),
        InterfaceInvocationTerminal::Cancelled
    );
    assert_eq!(
        handler.receipt().stages().collect::<Vec<_>>(),
        vec![
            InterfaceInvocationStage::Received,
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::PrincipalEstablished,
            InterfaceInvocationStage::Authorized,
            InterfaceInvocationStage::Admitted,
            InterfaceInvocationStage::Prepared,
            InterfaceInvocationStage::Dispatched,
            InterfaceInvocationStage::Executing,
        ]
    );
}

#[tokio::test]
async fn active_invocation_uses_its_frozen_snapshot_after_candidate_publish() {
    let old_seen = Arc::new(Mutex::new(None));
    let old = compile_snapshot("graph:old", 1, false, Arc::clone(&old_seen));
    let registry = DynamicInterfaceRegistry::new(Arc::clone(&old));
    let active_snapshot = registry.snapshot();
    let new = compile_snapshot("graph:new", 8, false, Arc::new(Mutex::new(None)));
    registry.publish(new);
    let kernel = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }));

    let old_outcome = kernel
        .invoke::<Input, Output, TargetError>(active_snapshot, envelope(actor(true)))
        .await
        .unwrap();
    let new_outcome = kernel
        .invoke::<Input, Output, TargetError>(registry.snapshot(), envelope(actor(true)))
        .await
        .unwrap();

    assert_eq!(old_outcome.value(), &Output(3));
    assert_eq!(new_outcome.value(), &Output(10));
    assert_ne!(
        old_outcome.receipt().registry_fingerprint(),
        new_outcome.receipt().registry_fingerprint()
    );
}

#[tokio::test]
async fn lcf_002_lcf_004_typed_hook_plan_runs_after_authorization_and_admission() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = compile_snapshot("graph:hooks", 3, false, Arc::new(Mutex::new(None)));
    let plan = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:hooks").unwrap(),
        snapshot
            .plan(&BindingId::new("http.invocation.read.v1").unwrap())
            .unwrap()
            .extension_plan()
            .fingerprint()
            .clone(),
    )
    .bind_before(Arc::new(RecordingBeforeHook {
        name: "alpha",
        increment: 1,
        events: Arc::clone(&events),
    }))
    .bind_before(Arc::new(RecordingBeforeHook {
        name: "beta",
        increment: 2,
        events: Arc::clone(&events),
    }))
    .bind_completion(Arc::new(RecordingCompletionHook {
        name: "alpha",
        events: Arc::clone(&events),
    }))
    .bind_completion(Arc::new(RecordingCompletionHook {
        name: "beta",
        events: Arc::clone(&events),
    }));
    let outcome = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    )
    .invoke_with_hook_plan::<Input, Output, TargetError>(snapshot, envelope(actor(true)), &plan)
    .await
    .unwrap();

    assert_eq!(outcome.value(), &Output(8));
    assert_eq!(
        events.lock().unwrap().as_slice(),
        [
            "before:alpha",
            "before:beta",
            "completion:beta:Completed",
            "completion:alpha:Completed",
        ]
    );
    assert_eq!(
        outcome.receipt().stages().collect::<Vec<_>>(),
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

#[tokio::test]
async fn lcf_006_completion_is_emitted_once_for_target_failure() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = compile_snapshot("graph:failed-hooks", 0, true, Arc::new(Mutex::new(None)));
    let plan = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:failed-hooks").unwrap(),
        snapshot
            .plan(&BindingId::new("http.invocation.read.v1").unwrap())
            .unwrap()
            .extension_plan()
            .fingerprint()
            .clone(),
    )
    .bind_completion(Arc::new(RecordingCompletionHook {
        name: "terminal",
        events: Arc::clone(&events),
    }));
    let failure = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke_with_hook_plan::<Input, Output, TargetError>(snapshot, envelope(actor(true)), &plan)
        .await
        .unwrap_err();

    assert_eq!(
        failure.receipt().terminal(),
        InterfaceInvocationTerminal::Failed
    );
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["completion:terminal:Failed"]
    );
}

#[test]
fn parent_lineage_rejects_cycles() {
    let root_id = InvocationId::now_v7();
    let child_id = InvocationId::now_v7();
    let lineage = InvocationLineage::root(root_id).child(child_id).unwrap();
    assert_eq!(lineage.parent_invocation_id(), Some(root_id));
    assert!(matches!(
        lineage.child(root_id),
        Err(InvocationLineageError::Cycle(_))
    ));
}

#[test]
fn stream_requires_exactly_one_terminal_and_rejects_events_after_terminal() {
    let mut stream = InterfaceStreamAccumulator::<StreamEvent, Output, TargetError>::new();
    stream.emit(StreamEvent(1)).unwrap();
    assert_eq!(
        stream.finish(InterfaceStreamTerminal::Completed(Output(2))),
        Ok(())
    );
    assert_eq!(
        stream.finish(InterfaceStreamTerminal::Cancelled),
        Err(InterfaceStreamStateError::DuplicateTerminal)
    );
    assert_eq!(
        stream.emit(StreamEvent(3)),
        Err(InterfaceStreamStateError::EventAfterTerminal)
    );
    let stream = stream.into_stream().unwrap();
    assert_eq!(stream.events(), &[StreamEvent(1)]);
    assert_eq!(
        stream.terminal(),
        &InterfaceStreamTerminal::Completed(Output(2))
    );

    let empty = InterfaceStreamAccumulator::<StreamEvent, Output, TargetError>::new();
    assert_eq!(
        empty.into_stream(),
        Err(InterfaceStreamStateError::MissingTerminal)
    );
}

#[tokio::test]
async fn dispatch_pin_retry_and_receipt_controls_are_immutable() {
    let snapshot = compile_snapshot("graph:runtime-pin", 1, false, Arc::new(Mutex::new(None)));
    let first_target = ExecutionTargetPin::Runtime {
        handler: HandlerReference::new("invocation.handler").unwrap(),
        target: TargetReference::new("invocation.target").unwrap(),
        plugin: PluginIdentity::new("official.runtime").unwrap(),
        artifact: ArtifactIdentity::new("sha256:artifact-one").unwrap(),
        runtime: RuntimeTargetIdentity::new("runtime.local").unwrap(),
        runtime_generation: RuntimeGeneration::new("generation:1").unwrap(),
        worker_generation: WorkerGeneration::new("worker:1").unwrap(),
    };
    let idempotency_key = IdempotencyKey::new("request-1944").unwrap();
    let envelope = InvocationEnvelope::with_principal_and_controls(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("http.invocation.read.v1").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        UserPrincipal::server_delegation(actor(true)),
        InvocationControls::new(
            None,
            InvocationCancellation::new(),
            Some(idempotency_key.clone()),
        ),
        Input(2),
    );
    let outcome = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke_with_dispatch_target::<Input, Output, TargetError>(
            snapshot,
            envelope,
            first_target.clone(),
        )
        .await
        .unwrap();
    let attempt = outcome.receipt().attempt().unwrap();
    assert_eq!(attempt.ordinal(), 1);
    assert_eq!(attempt.target(), &first_target);
    assert_eq!(outcome.receipt().idempotency_key(), Some(&idempotency_key));
    assert!(outcome.receipt().resolved().is_some());
    let projected = outcome.receipt().clone().projected().projected();
    assert_eq!(
        projected
            .stages()
            .filter(|stage| *stage == InterfaceInvocationStage::Projected)
            .count(),
        1
    );

    let retry_target = ExecutionTargetPin::Runtime {
        handler: HandlerReference::new("invocation.handler").unwrap(),
        target: TargetReference::new("invocation.target").unwrap(),
        plugin: PluginIdentity::new("official.runtime").unwrap(),
        artifact: ArtifactIdentity::new("sha256:artifact-two").unwrap(),
        runtime: RuntimeTargetIdentity::new("runtime.local").unwrap(),
        runtime_generation: RuntimeGeneration::new("generation:2").unwrap(),
        worker_generation: WorkerGeneration::new("worker:2").unwrap(),
    };
    let retry = attempt.retry(retry_target.clone());
    assert_ne!(retry.attempt_id(), attempt.attempt_id());
    assert_eq!(retry.ordinal(), 2);
    assert_eq!(retry.target(), &retry_target);
    assert_eq!(attempt.target(), &first_target);
}

#[tokio::test]
async fn explicit_cancellation_terminates_without_dispatch() {
    let cancellation = InvocationCancellation::new();
    cancellation.cancel();
    let envelope = InvocationEnvelope::with_principal_and_controls(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("http.invocation.read.v1").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        UserPrincipal::server_delegation(actor(true)),
        InvocationControls::new(None, cancellation, None),
        Input(2),
    );
    let failure = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output, TargetError>(
            compile_snapshot("graph:cancelled", 1, false, Arc::new(Mutex::new(None))),
            envelope,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::Cancelled
    ));
    assert_eq!(
        failure.receipt().terminal(),
        InterfaceInvocationTerminal::Cancelled
    );
    assert!(failure.receipt().attempt().is_none());
}
