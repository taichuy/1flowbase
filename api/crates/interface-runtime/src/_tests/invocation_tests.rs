use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use domain::ActorContext;
use uuid::Uuid;

use crate::{
    ContractIdentity, DynamicInterfaceRegistry, GraphFingerprint, HandlerReference,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceAuthorizationError,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceBeforeHook, InterfaceBeforeHookFuture, InterfaceCompletionHook,
    InterfaceCompletionHookFuture, InterfaceContract, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceHookContext,
    InterfaceId, InterfaceInvocationError, InterfaceInvocationKernel, InterfaceInvocationStage,
    InterfaceInvocationTerminal, InterfaceLifecycle, InterfaceOwner, InterfaceProtocol,
    InterfaceScope, InterfaceTargetAdmissionError, InterfaceTargetAdmissionFuture,
    InterfaceTargetAdmissionPort, InterfaceTargetAdmissionRequest, InterfaceTargetError,
    InvocationEnvelope, InvocationId, InvocationLineage, InvocationLineageError,
    PermissionIdentity, RegistryCompiler, RouteIdentity, TargetReference, TypedInterfaceHookPlan,
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

struct RecordingHandler {
    increment: u8,
    seen_fingerprint: Arc<Mutex<Option<String>>>,
    fail: bool,
}

impl InterfaceHandler<Input, Output> for RecordingHandler {
    fn invoke(
        &self,
        context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output> {
        let increment = self.increment;
        let seen_fingerprint = Arc::clone(&self.seen_fingerprint);
        let fingerprint = context.registry_fingerprint().as_str().to_string();
        let fail = self.fail;
        Box::pin(async move {
            *seen_fingerprint.lock().unwrap() = Some(fingerprint);
            if fail {
                Err(InterfaceTargetError::classified("target_failed"))
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
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let reject = self.reject || !request.actor().is_root;
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

impl InterfaceHandler<Input, Output> for SlowHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(Output(input.0))
        })
    }
}

impl InterfaceTargetAdmissionPort for Admission {
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

fn compile_snapshot(
    graph: &str,
    increment: u8,
    fail: bool,
    seen_fingerprint: Arc<Mutex<Option<String>>>,
) -> Arc<crate::CompiledInterfaceRegistry> {
    let permission = PermissionIdentity::new("invocation.read").unwrap();
    let mut compiler =
        RegistryCompiler::new(GraphFingerprint::new(graph).unwrap(), [permission.clone()]);
    compiler
        .register_definition(InterfaceDefinition::new(
            interface_id(),
            ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
            ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap(),
            Some(RouteIdentity::new("GET", "/api/console/invocation").unwrap()),
            permission,
            InterfaceAuthenticationPolicy::Authenticated,
            InterfaceAuditPolicy::ReadOnly,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceScope::System,
            InterfaceLifecycle::BootSnapshot,
            HandlerReference::new("invocation.handler").unwrap(),
            TargetReference::new("invocation.target").unwrap(),
            InterfaceOwner::new("core").unwrap(),
        ))
        .unwrap();
    compiler
        .bind_handler::<Input, Output>(
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
        interface_id(),
        InterfaceProtocol::Http,
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
        .invoke::<Input, Output>(snapshot, envelope(actor(true)))
        .await
        .unwrap();

    assert_eq!(outcome.value(), &Output(5));
    assert_eq!(
        outcome.receipt().stages(),
        &[
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::Authorized,
            InterfaceInvocationStage::Admitted,
            InterfaceInvocationStage::Invoking,
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
        .invoke::<Input, Output>(Arc::clone(&snapshot), envelope(actor(true)))
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
    .invoke::<Input, Output>(Arc::clone(&snapshot), envelope(actor(true)))
    .await
    .unwrap_err();
    assert!(matches!(
        admission.error(),
        InterfaceInvocationError::AdmissionRejected(_)
    ));

    let failed_snapshot = compile_snapshot("graph:failed", 0, true, Arc::new(Mutex::new(None)));
    let failed = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output>(failed_snapshot, envelope(actor(true)))
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
        interface_id(),
        InterfaceProtocol::Http,
        actor(true),
        Some(SystemTime::now() - Duration::from_secs(1)),
        Input(2),
    );
    let cancelled = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output>(snapshot, elapsed)
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
        .invoke::<Input, Output>(
            Arc::clone(&snapshot),
            InvocationEnvelope::new(
                InvocationLineage::root(InvocationId::now_v7()),
                interface_id(),
                InterfaceProtocol::Http,
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
        authorization.receipt().stages(),
        &[InterfaceInvocationStage::Resolved]
    );

    let admission = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(SlowAdmission),
    )
    .invoke::<Input, Output>(
        Arc::clone(&snapshot),
        InvocationEnvelope::new(
            InvocationLineage::root(InvocationId::now_v7()),
            interface_id(),
            InterfaceProtocol::Http,
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
        admission.receipt().stages(),
        &[
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::Authorized
        ]
    );

    let permission = PermissionIdentity::new("invocation.read").unwrap();
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:slow-handler").unwrap(),
        [permission.clone()],
    );
    compiler
        .register_definition(InterfaceDefinition::new(
            interface_id(),
            ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
            ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap(),
            Some(RouteIdentity::new("GET", "/api/console/invocation").unwrap()),
            permission,
            InterfaceAuthenticationPolicy::Authenticated,
            InterfaceAuditPolicy::ReadOnly,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceScope::System,
            InterfaceLifecycle::BootSnapshot,
            HandlerReference::new("invocation.handler").unwrap(),
            TargetReference::new("invocation.target").unwrap(),
            InterfaceOwner::new("core").unwrap(),
        ))
        .unwrap();
    compiler
        .bind_handler::<Input, Output>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(SlowHandler),
        )
        .unwrap();
    let handler = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke::<Input, Output>(
            compiler.compile().unwrap(),
            InvocationEnvelope::new(
                InvocationLineage::root(InvocationId::now_v7()),
                interface_id(),
                InterfaceProtocol::Http,
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
        handler.receipt().stages(),
        &[
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::Authorized,
            InterfaceInvocationStage::Invoking,
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
        .invoke::<Input, Output>(active_snapshot, envelope(actor(true)))
        .await
        .unwrap();
    let new_outcome = kernel
        .invoke::<Input, Output>(registry.snapshot(), envelope(actor(true)))
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
    let plan =
        TypedInterfaceHookPlan::<Input, Output>::new(GraphFingerprint::new("graph:hooks").unwrap())
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
    .invoke_with_hook_plan(snapshot, envelope(actor(true)), &plan)
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
        outcome.receipt().stages(),
        &[
            InterfaceInvocationStage::Resolved,
            InterfaceInvocationStage::Authorized,
            InterfaceInvocationStage::Admitted,
            InterfaceInvocationStage::BeforeHooksCompleted,
            InterfaceInvocationStage::Invoking,
            InterfaceInvocationStage::AfterHooksCompleted,
        ]
    );
}

#[tokio::test]
async fn lcf_006_completion_is_emitted_once_for_target_failure() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = compile_snapshot("graph:failed-hooks", 0, true, Arc::new(Mutex::new(None)));
    let plan = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:failed-hooks").unwrap(),
    )
    .bind_completion(Arc::new(RecordingCompletionHook {
        name: "terminal",
        events: Arc::clone(&events),
    }));
    let failure = InterfaceInvocationKernel::new(Arc::new(Authorization { reject: false }))
        .invoke_with_hook_plan(snapshot, envelope(actor(true)), &plan)
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
