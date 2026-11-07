use super::*;
use crate::{
    DocumentOperationCodec, DocumentOperationDescriptor, DocumentOperationEvent,
    DocumentOperationInterestFuture, DocumentOperationObserver, DocumentOperationObserverFuture,
    DocumentOperationProjection, DocumentOperationRegistration, DocumentOperationSchemas,
    DocumentOperationSelector, DocumentOperationSession,
};
use std::sync::atomic::{AtomicUsize, Ordering};

type Events = Arc<Mutex<Vec<DocumentOperationEvent>>>;
struct DocumentObserver {
    events: Events,
    behavior: ObserverBehavior,
    interested: bool,
    checks: AtomicUsize,
}
impl DocumentOperationObserver for DocumentObserver {
    fn capture_context(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(String::from("document-ancestry")))
    }
    fn interested<'a>(
        &'a self,
        _: &'a crate::InterfaceEventContext,
        _: &'a DocumentOperationDescriptor,
    ) -> DocumentOperationInterestFuture<'a> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(self.interested) })
    }
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        if matches!(self.behavior, ObserverBehavior::PanicConstruction) {
            panic!("controlled constructor panic");
        }
        Box::pin(async move {
            if matches!(self.behavior, ObserverBehavior::PanicPolling) {
                panic!("controlled polling panic");
            }
            if matches!(self.behavior, ObserverBehavior::HangCompletion)
                && matches!(event, DocumentOperationEvent::Completion { .. })
            {
                std::future::pending::<()>().await;
            }
            self.events.lock().unwrap().push(event);
            if matches!(self.behavior, ObserverBehavior::Fail) {
                Err(Arc::from("controlled-observer-error"))
            } else {
                Ok(())
            }
        })
    }
}
struct Codec {
    calls: Arc<AtomicUsize>,
}
impl DocumentOperationCodec<Input, Output> for Codec {
    fn project_input(&self, input: &Input) -> Result<serde_json::Value, Arc<str>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"path":{"resource":"owner"},"body":{"number":input.0}}))
    }
    fn project_output(&self, output: &Output) -> Result<serde_json::Value, Arc<str>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"number":output.0}))
    }
}
fn descriptor() -> DocumentOperationDescriptor {
    DocumentOperationDescriptor {
        operation_id: Arc::from("document-api.operation"),
        binding_id: binding().binding_id().clone(),
        interface_identity: interface_identity(),
        registration_generation: Arc::from("catalog:v1"),
        target_identity: Arc::from("resource:owner"),
        target_generation: Arc::from("publication:7"),
        schemas: DocumentOperationSchemas {
            request: Some(
                json!({"type":"object","properties":{"path":{"type":"object"},"body":{"type":"object"}}}),
            ),
            result: Some(json!({"type":"object","properties":{"number":{"type":"integer"}}})),
            event: None,
            request_unavailable: None,
            result_unavailable: None,
        },
    }
}
fn projection(
    descriptor: DocumentOperationDescriptor,
    calls: Arc<AtomicUsize>,
) -> DocumentOperationProjection<Input, Output> {
    DocumentOperationProjection {
        descriptor,
        codec: Arc::new(Codec { calls }),
    }
}
fn document_compiler(
    behavior: ObserverBehavior,
    interested: bool,
    legacy: bool,
) -> (
    RegistryCompiler,
    Arc<DocumentObserver>,
    Arc<Mutex<Vec<InterfaceEvent>>>,
) {
    let (old, old_events) = observer(ObserverBehavior::Record);
    let observer = Arc::new(DocumentObserver {
        events: Arc::new(Mutex::new(Vec::new())),
        behavior,
        interested,
        checks: AtomicUsize::new(0),
    });
    (
        event_compiler(old, legacy).with_document_operation_observer(observer.clone()),
        observer,
        old_events,
    )
}
fn bind_handler(compiler: &mut RegistryCompiler, fail: bool, slow: bool) {
    let handler: Arc<dyn InterfaceHandler<Input, Output, TargetError>> = if slow {
        Arc::new(SlowHandler)
    } else {
        Arc::new(RecordingHandler {
            increment: 3,
            fail,
            seen_fingerprint: Arc::new(Mutex::new(None)),
        })
    };
    compiler
        .bind_handler(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            handler,
        )
        .unwrap();
}
fn register_static(
    compiler: &mut RegistryCompiler,
    descriptor: DocumentOperationDescriptor,
    calls: Arc<AtomicUsize>,
) {
    compiler
        .bind_document_operation(
            binding().binding_id(),
            DocumentOperationRegistration::static_operation(
                "catalog:v1",
                projection(descriptor, calls),
            ),
        )
        .unwrap();
}
fn completion(
    event: &DocumentOperationEvent,
    expected: InterfaceInvocationTerminal,
    expected_input: bool,
    expected_output: Option<serde_json::Value>,
) {
    let DocumentOperationEvent::Completion {
        terminal,
        input,
        output,
        ..
    } = event
    else {
        panic!("expected Completion");
    };
    assert_eq!(*terminal, expected);
    assert_eq!(input.is_some(), expected_input);
    assert_eq!(*output, expected_output);
}

#[tokio::test]
async fn document_channel_does_not_require_legacy_opt_in_and_coexists_without_changing_legacy_values(
) {
    for legacy in [false, true] {
        let (mut compiler, observer, old) =
            document_compiler(ObserverBehavior::Record, true, legacy);
        let calls = Arc::new(AtomicUsize::new(0));
        register_static(&mut compiler, descriptor(), calls.clone());
        bind_handler(&mut compiler, false, false);
        let snapshot = compiler.compile().unwrap();
        let outcome = kernel(false)
            .invoke::<Input, Output, TargetError>(snapshot.clone(), envelope(actor(true)))
            .await
            .unwrap();
        assert_eq!(outcome.value(), &Output(5));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(observer.checks.load(Ordering::SeqCst), 1);
        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        let DocumentOperationEvent::Before { input, .. } = &events[0] else {
            panic!("expected Before")
        };
        assert_eq!(
            *input,
            json!({"path":{"resource":"owner"},"body":{"number":2}})
        );
        completion(
            &events[1],
            InterfaceInvocationTerminal::Completed,
            true,
            Some(json!({"number":5})),
        );
        assert_eq!(
            events[1].context().registry_fingerprint(),
            snapshot.fingerprint()
        );
        assert_eq!(
            events[1].context().observer_context::<String>().unwrap(),
            "document-ancestry"
        );
        assert!(events[1].context().principal().is_some());
        assert!(events[1].context().attempt().is_some());
        let old = old.lock().unwrap();
        assert_eq!(old.len(), if legacy { 2 } else { 0 });
        if legacy {
            assert_completion(
                &old[1],
                InterfaceInvocationTerminal::Completed,
                None,
                Some(json!({"value":5})),
            );
        }
    }
}

#[tokio::test]
async fn no_interest_or_missing_phase_schema_skips_codecs_without_fabricating_values() {
    for interested in [false, true] {
        let (mut compiler, observer, _) =
            document_compiler(ObserverBehavior::Record, interested, false);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut desc = descriptor();
        if interested {
            desc.schemas.request = None;
            desc.schemas.result = None;
        }
        register_static(&mut compiler, desc, calls.clone());
        bind_handler(&mut compiler, false, false);
        let result = kernel(false)
            .invoke::<Input, Output, TargetError>(
                compiler.compile().unwrap(),
                envelope(actor(true)),
            )
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(observer.checks.load(Ordering::SeqCst), 1);
        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), usize::from(interested));
        if interested {
            completion(
                &events[0],
                InterfaceInvocationTerminal::Completed,
                false,
                None,
            );
        }
        assert!(result
            .receipt()
            .observer_records()
            .iter()
            .all(|r| r.status() == InterfaceObserverStatus::Executed));
    }
}

#[tokio::test]
async fn document_failure_denial_and_cancellation_never_project_success_output() {
    for scenario in ["failure", "denied", "cancel-before", "cancel-inflight"] {
        let (mut compiler, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
        let calls = Arc::new(AtomicUsize::new(0));
        register_static(&mut compiler, descriptor(), calls.clone());
        bind_handler(
            &mut compiler,
            scenario == "failure",
            scenario == "cancel-inflight",
        );
        let cancel = InvocationCancellation::new();
        if scenario == "cancel-before" {
            cancel.cancel();
        }
        let request = controlled_envelope(InvocationControls::new(None, cancel.clone(), None));
        let task = if scenario == "cancel-inflight" {
            Some(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                cancel.cancel();
            }))
        } else {
            None
        };
        let result = kernel(scenario == "denied")
            .invoke::<Input, Output, TargetError>(compiler.compile().unwrap(), request)
            .await
            .unwrap_err();
        if let Some(task) = task {
            task.await.unwrap();
        }
        let observed = matches!(scenario, "failure" | "cancel-inflight");
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(observed));
        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), if observed { 2 } else { 1 });
        completion(
            events.last().unwrap(),
            result.receipt().terminal(),
            observed,
            None,
        );
    }
}

#[test]
fn document_registration_rejects_binding_interface_contract_and_generation_mismatch() {
    for variant in ["binding", "interface", "generation", "unknown"] {
        let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
        bind_handler(&mut compiler, false, false);
        let mut desc = descriptor();
        match variant {
            "binding" => desc.binding_id = BindingId::new("wrong-binding").unwrap(),
            "interface" => {
                desc.interface_identity = InterfaceIdentity::new(
                    InterfaceId::new("wrong-interface").unwrap(),
                    InterfaceVersion::new("1").unwrap(),
                )
            }
            "generation" => desc.registration_generation = Arc::from("catalog:replaced"),
            _ => {}
        }
        let id = if variant == "unknown" {
            BindingId::new("unknown").unwrap()
        } else {
            binding().binding_id().clone()
        };
        compiler
            .bind_document_operation(
                &id,
                DocumentOperationRegistration::static_operation(
                    "catalog:v1",
                    projection(desc, Arc::new(AtomicUsize::new(0))),
                ),
            )
            .unwrap();
        assert!(matches!(
            compiler.compile(),
            Err(RegistryCompilationError::DocumentOperationMismatch(_))
        ));
    }
}

struct OwnerSnapshot {
    generation: &'static str,
    mode: &'static str,
}
struct Selector {
    calls: Arc<AtomicUsize>,
}
impl DocumentOperationSelector<Input, Output> for Selector {
    fn select(
        &self,
        snapshot: &(dyn std::any::Any + Send + Sync),
    ) -> Result<DocumentOperationProjection<Input, Output>, Arc<str>> {
        let snapshot = snapshot
            .downcast_ref::<OwnerSnapshot>()
            .ok_or("not-owner-snapshot")?;
        if snapshot.generation != "publication:7" {
            return Err(Arc::from("owner-generation-mismatch"));
        }
        let mut desc = descriptor();
        if snapshot.mode == "wrong-binding" {
            desc.binding_id = BindingId::new("other").unwrap();
        }
        if snapshot.mode == "wrong-generation" {
            desc.registration_generation = Arc::from("catalog:other");
        }
        Ok(projection(desc, self.calls.clone()))
    }
}
struct DynamicHandler {
    mode: &'static str,
    saved: Arc<Mutex<Option<DocumentOperationSession>>>,
    snapshot_weak: Arc<Mutex<Option<std::sync::Weak<OwnerSnapshot>>>>,
}
impl InterfaceHandler<Input, Output, TargetError> for DynamicHandler {
    fn invoke(
        &self,
        context: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        let session = context.document_operation().unwrap().clone();
        let mode = self.mode;
        let saved = self.saved.clone();
        let snapshot_weak = self.snapshot_weak.clone();
        Box::pin(async move {
            *saved.lock().unwrap() = Some(session.clone());
            let snapshot = Arc::new(OwnerSnapshot {
                generation: if mode == "owner-generation" {
                    "publication:8"
                } else {
                    "publication:7"
                },
                mode,
            });
            *snapshot_weak.lock().unwrap() = Some(Arc::downgrade(&snapshot));
            let selected = session.select(snapshot);
            if matches!(
                mode,
                "wrong-binding" | "wrong-generation" | "owner-generation"
            ) {
                assert!(selected.is_err());
                return Ok(Output(5));
            }
            selected.unwrap();
            if mode == "identity-only" {
                return Err(InterfaceTargetFailure::new("owner-rejected", TargetError));
            }
            session.observe_before(&input).await.unwrap();
            assert!(session
                .select(Arc::new(OwnerSnapshot {
                    generation: "publication:7",
                    mode: "ok"
                }))
                .is_err());
            assert!(session.observe_before(&Input(99)).await.is_err());
            Ok(Output(input.0 + 3))
        })
    }
}
#[tokio::test]
async fn dynamic_snapshot_selects_once_and_invalid_target_only_disables_observation() {
    for mode in [
        "ok",
        "identity-only",
        "wrong-binding",
        "wrong-generation",
        "owner-generation",
    ] {
        let (mut compiler, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
        let calls = Arc::new(AtomicUsize::new(0));
        let saved = Arc::new(Mutex::new(None));
        let snapshot_weak = Arc::new(Mutex::new(None));
        compiler
            .bind_document_operation(
                binding().binding_id(),
                DocumentOperationRegistration::dynamic(
                    "catalog:v1",
                    Arc::new(Selector {
                        calls: calls.clone(),
                    }),
                ),
            )
            .unwrap();
        compiler
            .bind_handler(
                &interface_id(),
                HandlerReference::new("invocation.handler").unwrap(),
                Arc::new(DynamicHandler {
                    mode,
                    saved: saved.clone(),
                    snapshot_weak: snapshot_weak.clone(),
                }),
            )
            .unwrap();
        let result = kernel(false)
            .invoke::<Input, Output, TargetError>(
                compiler.compile().unwrap(),
                envelope(actor(true)),
            )
            .await;
        if mode == "identity-only" {
            assert_eq!(
                result.unwrap_err().error().classification(),
                "owner-rejected"
            );
        } else {
            let outcome = result.unwrap();
            assert_eq!(outcome.value(), &Output(5));
            assert!(
                outcome
                    .receipt()
                    .observer_records()
                    .iter()
                    .any(|r| r.status() == InterfaceObserverStatus::Failed),
                "rejected replacement or invalid selection must record a diagnostic"
            );
        }
        assert!(
            snapshot_weak
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .upgrade()
                .is_none(),
            "owner snapshot released after finalization despite retained session"
        );
        let events = observer.events.lock().unwrap();
        match mode {
            "ok" => {
                assert_eq!(events.len(), 2);
                completion(
                    &events[1],
                    InterfaceInvocationTerminal::Completed,
                    true,
                    Some(json!({"number":5})),
                );
            }
            "identity-only" => {
                assert_eq!(events.len(), 1);
                completion(&events[0], InterfaceInvocationTerminal::Failed, false, None);
            }
            _ => assert!(events.is_empty()),
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            if mode == "ok" { 2 } else { 0 }
        );
        assert!(saved
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .select(Arc::new(OwnerSnapshot {
                generation: "publication:7",
                mode: "ok"
            }))
            .is_err());
    }
}

#[tokio::test]
async fn document_observer_faults_preserve_primary_success_and_failure() {
    for behavior in [
        ObserverBehavior::Fail,
        ObserverBehavior::PanicConstruction,
        ObserverBehavior::PanicPolling,
        ObserverBehavior::HangCompletion,
    ] {
        for fail in [false, true] {
            let (mut compiler, _, _) = document_compiler(behavior, true, false);
            register_static(&mut compiler, descriptor(), Arc::new(AtomicUsize::new(0)));
            bind_handler(&mut compiler, fail, false);
            let result = kernel(false)
                .invoke::<Input, Output, TargetError>(
                    compiler.compile().unwrap(),
                    envelope(actor(true)),
                )
                .await;
            let receipt = match result {
                Ok(outcome) => {
                    assert!(!fail);
                    assert_eq!(outcome.value(), &Output(5));
                    outcome.receipt().clone()
                }
                Err(failure) => {
                    assert!(fail);
                    assert_eq!(failure.error().classification(), "target_failed");
                    failure.receipt().clone()
                }
            };
            assert!(receipt.observer_records().iter().any(|r| matches!(
                r.status(),
                InterfaceObserverStatus::Failed | InterfaceObserverStatus::TimedOut
            )));
        }
    }
}

#[tokio::test]
async fn document_before_observes_mutated_typed_before_input() {
    let (mut compiler, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
    register_static(&mut compiler, descriptor(), Arc::new(AtomicUsize::new(0)));
    bind_handler(&mut compiler, false, false);
    let hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:events").unwrap(),
    )
    .bind_before(
        PluginIdentity::new("hook.mutate").unwrap(),
        Arc::new(RecordingBeforeHook {
            name: "mutate",
            increment: 4,
            events: Arc::new(Mutex::new(Vec::new())),
        }),
    );
    compiler
        .register_extension(
            &interface_id(),
            1,
            hook_registration("hook.mutate", InterfaceExtensionPoint::Before),
        )
        .unwrap();
    compiler
        .bind_hook_plan(&interface_id(), Arc::new(hooks))
        .unwrap();
    let result = kernel(false)
        .invoke::<Input, Output, TargetError>(compiler.compile().unwrap(), envelope(actor(true)))
        .await
        .unwrap();
    assert_eq!(result.value(), &Output(9));
    let events = observer.events.lock().unwrap();
    let DocumentOperationEvent::Before { input, .. } = &events[0] else {
        panic!("Before")
    };
    assert_eq!(input["body"]["number"], 6);
}

fn document_stream_compiler(
    observer: Arc<DocumentObserver>,
    outcome: StreamOutcome,
) -> RegistryCompiler {
    let contracts = InterfaceContracts::server_stream(
        ContractIdentity::new(Input::CONTRACT_ID, "1").unwrap(),
        ContractIdentity::new(StreamEvent::CONTRACT_ID, "1").unwrap(),
        ContractIdentity::new(Output::CONTRACT_ID, "1").unwrap(),
        ContractIdentity::new(TargetError::CONTRACT_ID, "1").unwrap(),
    );
    let definition = InterfaceDefinition::new(
        interface_identity(),
        contracts.clone(),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation(),
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::ServerStream,
            HandlerReference::new("invocation.handler").unwrap(),
            TargetReference::new("invocation.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner(),
    );
    let mut compiler = compiler("graph:document-stream").with_document_operation_observer(observer);
    compiler.register_definition(definition).unwrap();
    activate_authentication(&mut compiler);
    compiler
        .register_binding(
            ProtocolBinding::new(
                binding().binding_id().clone(),
                interface_identity(),
                contracts,
                binding().projection().clone(),
            ),
            adapter_plan(),
        )
        .unwrap();
    compiler
        .bind_stream_handler(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(StreamHandler(outcome)),
        )
        .unwrap();
    compiler
}
#[tokio::test]
async fn document_stream_uses_unique_real_terminal_and_drop_does_not_fabricate_completion() {
    for scenario in [
        "success",
        "failure",
        "rejected",
        "cancelled",
        "missing",
        "drop-values",
        "drop-completion",
    ] {
        let observer = Arc::new(DocumentObserver {
            events: Arc::new(Mutex::new(Vec::new())),
            behavior: ObserverBehavior::Record,
            interested: true,
            checks: AtomicUsize::new(0),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let outcome = match scenario {
            "failure" => StreamOutcome::Failure,
            "rejected" => StreamOutcome::Rejected,
            "cancelled" => StreamOutcome::Cancelled,
            "missing" => StreamOutcome::Missing,
            _ => StreamOutcome::Success,
        };
        let mut compiler = document_stream_compiler(observer.clone(), outcome);
        let mut desc = descriptor();
        desc.schemas.event =
            Some(json!({"type":"object","properties":{"value":{"type":"integer"}}}));
        register_static(&mut compiler, desc, calls.clone());
        let stream = kernel(false)
            .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
                compiler.compile().unwrap(),
                envelope(actor(true)),
                stream_target(),
            )
            .await
            .unwrap();
        let (mut values, completion_owner) = stream.into_parts();
        assert_eq!(observer.events.lock().unwrap().len(), 1);
        if scenario == "drop-completion" {
            drop(completion_owner);
            drop(values);
            assert_eq!(observer.events.lock().unwrap().len(), 1);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            continue;
        }
        if scenario == "drop-values" {
            drop(values);
        } else if scenario != "missing" {
            assert_eq!(values.recv().await, Some(StreamEvent(7)));
        }
        let result = completion_owner.complete().await;
        let (terminal, output) = match scenario {
            "success" | "drop-values" => {
                assert!(result.is_ok());
                (
                    InterfaceInvocationTerminal::Completed,
                    Some(json!({"number":9})),
                )
            }
            "cancelled" => {
                assert!(result.is_ok());
                (InterfaceInvocationTerminal::Cancelled, None)
            }
            "rejected" => {
                assert!(result.is_ok());
                (InterfaceInvocationTerminal::Rejected, None)
            }
            _ => {
                assert!(result.is_err());
                (InterfaceInvocationTerminal::Failed, None)
            }
        };
        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        completion(&events[1], terminal, true, output);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            if terminal == InterfaceInvocationTerminal::Completed {
                2
            } else {
                1
            }
        );
    }
}

struct InterestFault {
    behavior: &'static str,
    events: Events,
}
impl DocumentOperationObserver for InterestFault {
    fn interested<'a>(
        &'a self,
        _: &'a crate::InterfaceEventContext,
        _: &'a DocumentOperationDescriptor,
    ) -> DocumentOperationInterestFuture<'a> {
        if self.behavior == "construct-panic" {
            panic!("controlled interest constructor panic");
        }
        Box::pin(async move {
            match self.behavior {
                "poll-panic" => panic!("controlled interest poll panic"),
                "timeout" => std::future::pending().await,
                _ => Err(Arc::from("controlled interest failure")),
            }
        })
    }
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        self.events.lock().unwrap().push(event);
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn interest_faults_fail_closed_without_projecting_or_replacing_primary_result() {
    for behavior in ["error", "construct-panic", "poll-panic", "timeout"] {
        let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
        let events = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        compiler = compiler.with_document_operation_observer(Arc::new(InterestFault {
            behavior,
            events: events.clone(),
        }));
        register_static(&mut compiler, descriptor(), calls.clone());
        bind_handler(&mut compiler, false, false);
        let result = kernel(false)
            .invoke::<Input, Output, TargetError>(
                compiler.compile().unwrap(),
                envelope(actor(true)),
            )
            .await
            .unwrap();
        assert_eq!(result.value(), &Output(5));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(events.lock().unwrap().is_empty());
        assert!(result.receipt().observer_records().iter().any(|r| matches!(
            r.status(),
            InterfaceObserverStatus::Failed | InterfaceObserverStatus::TimedOut
        )));
    }
}

#[test]
fn document_schema_and_codec_generation_change_registry_proof() {
    let mut fingerprints = Vec::new();
    for variant in ["original", "schema", "target-generation"] {
        let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
        bind_handler(&mut compiler, false, false);
        let mut desc = descriptor();
        if variant == "schema" {
            desc.schemas.result = Some(json!({"type":"integer"}));
        }
        if variant == "target-generation" {
            desc.target_generation = Arc::from("publication:8");
        }
        register_static(&mut compiler, desc, Arc::new(AtomicUsize::new(0)));
        fingerprints.push(compiler.compile().unwrap().fingerprint().clone());
    }
    assert_ne!(fingerprints[0], fingerprints[1]);
    assert_ne!(fingerprints[0], fingerprints[2]);
}

#[tokio::test]
async fn rejected_authentication_has_static_identity_completion_without_business_data() {
    let (mut compiler, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
    let calls = Arc::new(AtomicUsize::new(0));
    register_static(&mut compiler, descriptor(), calls.clone());
    bind_handler(&mut compiler, false, false);
    let snapshot = compiler.compile().unwrap();
    let attempt = crate::InterfaceAuthenticationAttempt::resolve(
        snapshot,
        binding().binding_id(),
        InterfaceProtocol::Http,
        InvocationLineage::root(InvocationId::now_v7()),
    )
    .unwrap();
    let receipt = attempt
        .reject(crate::InterfaceAuthenticationRejectionClass::CredentialRejected)
        .await;
    assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Rejected);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let events = observer.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    completion(
        &events[0],
        InterfaceInvocationTerminal::Rejected,
        false,
        None,
    );
    assert!(events[0].context().principal().is_none());
}

struct WrongInputCodec;
impl DocumentOperationCodec<Output, Output> for WrongInputCodec {
    fn project_input(&self, _: &Output) -> Result<serde_json::Value, Arc<str>> {
        panic!("mismatched input codec must never execute")
    }
    fn project_output(&self, _: &Output) -> Result<serde_json::Value, Arc<str>> {
        panic!("mismatched output codec must never execute")
    }
}
#[test]
fn document_registration_rejects_a_codec_for_a_different_typed_input_contract() {
    let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
    bind_handler(&mut compiler, false, false);
    compiler
        .bind_document_operation(
            binding().binding_id(),
            DocumentOperationRegistration::static_operation(
                "catalog:v1",
                DocumentOperationProjection {
                    descriptor: descriptor(),
                    codec: Arc::new(WrongInputCodec),
                },
            ),
        )
        .unwrap();
    assert!(matches!(
        compiler.compile(),
        Err(RegistryCompilationError::DocumentOperationMismatch(_))
    ));
}

struct CapturePanic;
impl DocumentOperationObserver for CapturePanic {
    fn capture_context(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        panic!("controlled capture panic")
    }
    fn observe(&self, _: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn capture_context_panic_is_a_diagnostic_and_preserves_primary_result() {
    let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
    compiler = compiler.with_document_operation_observer(Arc::new(CapturePanic));
    register_static(&mut compiler, descriptor(), Arc::new(AtomicUsize::new(0)));
    bind_handler(&mut compiler, false, false);
    let result = kernel(false)
        .invoke::<Input, Output, TargetError>(compiler.compile().unwrap(), envelope(actor(true)))
        .await
        .unwrap();
    assert_eq!(result.value(), &Output(5));
    assert!(result
        .receipt()
        .observer_records()
        .iter()
        .any(|r| r.status() == InterfaceObserverStatus::Failed));
}

#[test]
fn compiled_document_metadata_reports_static_dynamic_and_missing_bindings_without_codecs() {
    for dynamic in [false, true] {
        let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
        let calls = Arc::new(AtomicUsize::new(0));
        bind_handler(&mut compiler, false, false);
        if dynamic {
            compiler
                .bind_document_operation(
                    binding().binding_id(),
                    DocumentOperationRegistration::dynamic(
                        "catalog:v1",
                        Arc::new(Selector {
                            calls: calls.clone(),
                        }),
                    ),
                )
                .unwrap();
        } else {
            register_static(&mut compiler, descriptor(), calls.clone());
        }
        let snapshot = compiler.compile().unwrap();
        assert_eq!(snapshot.document_operations().count(), 1);
        let metadata = snapshot.document_operation(binding().binding_id()).unwrap();
        assert_eq!(metadata.binding_id, binding().binding_id());
        assert_eq!(metadata.interface_identity, &interface_identity());
        assert_eq!(metadata.registration_generation, "catalog:v1");
        assert_eq!(metadata.dynamic, dynamic);
        assert!(metadata.observer_enabled);
        assert_eq!(metadata.static_descriptor.is_none(), dynamic);
        if let Some(desc) = metadata.static_descriptor {
            assert_eq!(desc, &descriptor());
            assert!(
                std::ptr::eq(
                    desc,
                    snapshot
                        .document_operation(binding().binding_id())
                        .unwrap()
                        .static_descriptor
                        .unwrap()
                ),
                "read-only inventory borrows the compiled descriptor"
            );
        }
        assert!(snapshot
            .document_operation(&BindingId::new("missing").unwrap())
            .is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn absorbing_snapshot_or_interface_retains_document_registration_and_observer() {
    for single in [false, true] {
        let (mut source, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
        let calls = Arc::new(AtomicUsize::new(0));
        register_static(&mut source, descriptor(), calls.clone());
        bind_handler(&mut source, false, false);
        let source = source.compile().unwrap();
        let mut merged = compiler("graph:merged-document");
        if single {
            merged.absorb_interface(&source, &interface_id()).unwrap();
        } else {
            merged.absorb_snapshot(&source).unwrap();
        }
        let merged = merged.compile().unwrap();
        let capability = merged.document_operation(binding().binding_id()).unwrap();
        assert!(capability.observer_enabled);
        assert_eq!(capability.static_descriptor, Some(&descriptor()));
        let result = kernel(false)
            .invoke::<Input, Output, TargetError>(merged.clone(), envelope(actor(true)))
            .await
            .unwrap();
        assert_eq!(result.value(), &Output(5));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1].context().registry_fingerprint(),
            merged.fingerprint()
        );
        assert_eq!(
            events[1].context().graph_fingerprint(),
            merged.graph_fingerprint()
        );
        completion(
            &events[1],
            InterfaceInvocationTerminal::Completed,
            true,
            Some(json!({"number":5})),
        );
    }
}

#[test]
fn compiled_document_metadata_distinguishes_registered_codec_from_active_observer() {
    let (legacy, _) = observer(ObserverBehavior::Record);
    let mut source = event_compiler(legacy, false);
    register_static(&mut source, descriptor(), Arc::new(AtomicUsize::new(0)));
    bind_handler(&mut source, false, false);
    let snapshot = source.compile().unwrap();
    let capability = snapshot.document_operation(binding().binding_id()).unwrap();
    assert!(!capability.observer_enabled);
    assert!(capability.static_descriptor.is_some());
}

#[test]
fn composition_metadata_refinement_rejects_cross_binding_generation_and_dynamic_replacement() {
    for variant in ["binding", "interface", "generation", "dynamic"] {
        let (mut compiler, _, _) = document_compiler(ObserverBehavior::Record, true, false);
        bind_handler(&mut compiler, false, false);
        if variant == "dynamic" {
            compiler
                .bind_document_operation(
                    binding().binding_id(),
                    DocumentOperationRegistration::dynamic(
                        "catalog:v1",
                        Arc::new(Selector {
                            calls: Arc::new(AtomicUsize::new(0)),
                        }),
                    ),
                )
                .unwrap();
        } else {
            register_static(&mut compiler, descriptor(), Arc::new(AtomicUsize::new(0)));
        }
        let mut replacement = descriptor();
        replacement.operation_id = Arc::from("document-refined");
        match variant {
            "binding" => replacement.binding_id = BindingId::new("other").unwrap(),
            "interface" => {
                replacement.interface_identity = InterfaceIdentity::new(
                    InterfaceId::new("other").unwrap(),
                    InterfaceVersion::new("1").unwrap(),
                )
            }
            "generation" => replacement.registration_generation = Arc::from("catalog:other"),
            _ => {}
        }
        assert!(matches!(
            compiler.rebind_document_operation_descriptor(binding().binding_id(), replacement),
            Err(RegistryCompilationError::DocumentOperationMismatch(_))
        ));
        let registry = compiler.compile().unwrap();
        let metadata = registry.document_operation(binding().binding_id()).unwrap();
        assert_eq!(metadata.dynamic, variant == "dynamic");
        if let Some(descriptor) = metadata.static_descriptor {
            assert_eq!(descriptor.operation_id.as_ref(), "document-api.operation");
        }
    }
}
#[tokio::test]
async fn opaque_owner_phase_reasons_disable_projection_without_kernel_policy_or_fault() {
    let (mut compiler, observer, _) = document_compiler(ObserverBehavior::Record, true, false);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut descriptor = descriptor();
    descriptor.schemas.request = None;
    descriptor.schemas.result = None;
    descriptor.schemas.request_unavailable = Some(Arc::from("owner-specific-input-reason"));
    descriptor.schemas.result_unavailable = Some(Arc::from("owner-specific-output-reason"));
    register_static(&mut compiler, descriptor, calls.clone());
    bind_handler(&mut compiler, false, false);
    let outcome = kernel(false)
        .invoke::<Input, Output, TargetError>(compiler.compile().unwrap(), envelope(actor(true)))
        .await
        .unwrap();
    assert_eq!(outcome.value(), &Output(5));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let events = observer.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    completion(
        &events[0],
        InterfaceInvocationTerminal::Completed,
        false,
        None,
    );
    assert!(outcome
        .receipt()
        .observer_records()
        .iter()
        .all(|record| record.status() == InterfaceObserverStatus::Executed));
}
