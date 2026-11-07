use super::*;
use crate::{
    interface_stream_channel, CompiledInterfaceRegistry, InterfaceEvent, InterfaceEventObserver,
    InterfaceEventObserverFuture, InterfaceEventPrincipal, InterfaceObserverStatus,
    InterfaceStreamHandler, InterfaceStreamHandlerFuture, RegistryCompilationError,
};
use serde_json::json;

#[derive(Clone, Copy)]
enum ObserverBehavior {
    Record,
    Fail,
    PanicConstruction,
    PanicPolling,
    HangCompletion,
}
struct Observer {
    events: Arc<Mutex<Vec<InterfaceEvent>>>,
    behavior: ObserverBehavior,
}
impl InterfaceEventObserver for Observer {
    fn observe(&self, event: InterfaceEvent) -> InterfaceEventObserverFuture<'_> {
        if matches!(self.behavior, ObserverBehavior::PanicConstruction) {
            panic!("controlled host observer constructor panic");
        }
        Box::pin(async move {
            if matches!(self.behavior, ObserverBehavior::PanicPolling) {
                panic!("controlled host observer polling panic");
            }
            if matches!(self.behavior, ObserverBehavior::HangCompletion)
                && matches!(event, InterfaceEvent::Completion { .. })
            {
                std::future::pending::<()>().await;
            }
            self.events.lock().unwrap().push(event);
            if matches!(self.behavior, ObserverBehavior::Fail) {
                Err(Arc::from("host-observer-failed"))
            } else {
                Ok(())
            }
        })
    }
}
fn observer(behavior: ObserverBehavior) -> (Arc<Observer>, Arc<Mutex<Vec<InterfaceEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(Observer {
            events: events.clone(),
            behavior,
        }),
        events,
    )
}
fn event_compiler(observer: Arc<Observer>, enabled: bool) -> RegistryCompiler {
    let mut compiler = compiler("graph:events").with_event_observer(observer);
    compiler
        .register_definition(if enabled {
            definition().with_event_observation()
        } else {
            definition()
        })
        .unwrap();
    activate_authentication(&mut compiler);
    compiler
        .register_binding(binding(), adapter_plan())
        .unwrap();
    compiler
}
fn unary_snapshot(
    observer: Arc<Observer>,
    fail: bool,
    enabled: bool,
    slow: bool,
) -> Arc<CompiledInterfaceRegistry> {
    let mut compiler = event_compiler(observer, enabled);
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
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            handler,
        )
        .unwrap();
    compiler.compile().unwrap()
}
fn kernel(deny: bool) -> InterfaceInvocationKernel {
    InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: deny }),
        Arc::new(Admission { reject: false }),
    )
}
fn controlled_envelope(controls: InvocationControls) -> InvocationEnvelope<Input> {
    InvocationEnvelope::with_principal_and_controls(
        InvocationLineage::root(InvocationId::now_v7()),
        binding().binding_id().clone(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        AuthenticationActivationIdentity::new("test.authn.activation.v1").unwrap(),
        UserPrincipal::server_delegation(actor(true)),
        controls,
        Input(2),
    )
}
fn assert_completion(
    event: &InterfaceEvent,
    terminal_expected: InterfaceInvocationTerminal,
    error_expected: Option<&str>,
    output_expected: Option<serde_json::Value>,
) {
    let InterfaceEvent::Completion {
        terminal,
        classification,
        output,
        ..
    } = event
    else {
        panic!("expected completion")
    };
    assert_eq!(*terminal, terminal_expected);
    assert_eq!(classification.as_deref(), error_expected);
    assert_eq!(*output, output_expected);
}

#[tokio::test]
async fn event_capability_is_explicit_and_independent_of_managed_hooks_and_subscriptions() {
    let (observer, events) = observer(ObserverBehavior::Record);
    let snapshot = unary_snapshot(observer, false, true, false);
    assert_eq!(snapshot.event_capabilities().len(), 1);
    let capability = snapshot.event_capability(&interface_id()).unwrap();
    assert_eq!(
        capability.definition().authorization_operation(),
        &operation()
    );
    assert_eq!(
        capability.input_schema()["properties"]["value"]["type"],
        "integer"
    );
    assert!(snapshot
        .managed_contracts()
        .all(|contract| contract.schema.is_none()));
    assert!(!snapshot
        .plan_for_interface(&interface_id())
        .unwrap()
        .has_managed_invocation_bridge());
    let request = envelope(actor(true))
        .freeze_extension_context(Arc::new(String::from("frozen-ancestry")))
        .unwrap();
    let invocation_id = request.lineage().invocation_id();
    let expected_user_id = request.principal().actor().user_id;
    let outcome = kernel(false)
        .invoke::<Input, Output, TargetError>(snapshot.clone(), request)
        .await
        .unwrap();
    assert_eq!(outcome.value(), &Output(5));
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 2);
    let InterfaceEvent::Before { context, input } = &events[0] else {
        panic!("expected before")
    };
    assert_eq!(*input, json!({"value":2}));
    assert_eq!(context.invocation_id(), invocation_id);
    assert_eq!(context.registry_fingerprint(), snapshot.fingerprint());
    assert_eq!(
        context.resolved().plan_fingerprint(),
        snapshot
            .plan_for_interface(&interface_id())
            .unwrap()
            .fingerprint()
    );
    assert_eq!(
        context.extension_context::<String>().unwrap(),
        "frozen-ancestry"
    );
    let Some(InterfaceEventPrincipal::User(principal)) = context.principal() else {
        panic!("lost sealed user")
    };
    assert_eq!(principal.actor().user_id, expected_user_id);
    assert!(events[1].context().attempt().is_some());
    assert_completion(
        &events[1],
        InterfaceInvocationTerminal::Completed,
        None,
        Some(json!({"value":5})),
    );
}

#[tokio::test]
async fn undeclared_operations_produce_no_observations_even_with_registered_contract_projections() {
    let (observer, events) = observer(ObserverBehavior::Record);
    let snapshot = unary_snapshot(observer, false, false, false);
    assert_eq!(snapshot.event_capabilities().len(), 0);
    kernel(false)
        .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
        .await
        .unwrap();
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn core_deny_has_no_before_or_business_data_and_keeps_real_error_classification() {
    let (observer, events) = observer(ObserverBehavior::Record);
    let snapshot = unary_snapshot(observer, false, true, false);
    let failure = kernel(true)
        .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
        .await
        .unwrap_err();
    assert_eq!(failure.error().classification(), "permission_denied");
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_completion(
        &events[0],
        InterfaceInvocationTerminal::Rejected,
        Some("permission_denied"),
        None,
    );
    let InterfaceEvent::Completion { input, .. } = &events[0] else {
        unreachable!()
    };
    assert!(input.is_none());
}

#[tokio::test]
async fn unary_completion_preserves_target_failure_cancellation_and_deadline() {
    for scenario in [
        "failure",
        "cancel-before",
        "deadline-before",
        "cancel-inflight",
        "deadline-inflight",
    ] {
        let (observer, events) = observer(ObserverBehavior::Record);
        let inflight = scenario.ends_with("inflight");
        let snapshot = unary_snapshot(observer, scenario == "failure", true, inflight);
        let cancellation = InvocationCancellation::new();
        if scenario == "cancel-before" {
            cancellation.cancel();
        }
        let deadline = match scenario {
            "deadline-before" => Some(SystemTime::now() - Duration::from_secs(1)),
            "deadline-inflight" => Some(SystemTime::now() + Duration::from_millis(10)),
            _ => None,
        };
        let request = controlled_envelope(InvocationControls::new(
            deadline,
            cancellation.clone(),
            None,
        ));
        let cancel_task = if scenario == "cancel-inflight" {
            Some(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                cancellation.cancel();
            }))
        } else {
            None
        };
        let failure = kernel(false)
            .invoke::<Input, Output, TargetError>(snapshot, request)
            .await
            .unwrap_err();
        if let Some(task) = cancel_task {
            task.await.unwrap();
        }
        let classification = match scenario {
            "failure" => "target_failed",
            "cancel-before" | "cancel-inflight" => "cancelled",
            _ => "deadline-elapsed",
        };
        assert_eq!(
            failure.error().classification(),
            classification,
            "{scenario}"
        );
        let events = events.lock().unwrap();
        assert_eq!(
            events.len(),
            if scenario.ends_with("before") { 1 } else { 2 },
            "{scenario}"
        );
        assert_completion(
            events.last().unwrap(),
            if scenario == "failure" {
                InterfaceInvocationTerminal::Failed
            } else {
                InterfaceInvocationTerminal::Cancelled
            },
            Some(classification),
            None,
        );
    }
}

#[tokio::test]
async fn observer_error_panic_and_timeout_never_replace_primary_result_or_typed_failure() {
    for behavior in [
        ObserverBehavior::Fail,
        ObserverBehavior::PanicConstruction,
        ObserverBehavior::PanicPolling,
        ObserverBehavior::HangCompletion,
    ] {
        for fail in [false, true] {
            let (observer, _) = observer(behavior);
            let snapshot = unary_snapshot(observer, fail, true, false);
            let result = kernel(false)
                .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
                .await;
            let receipt = if fail {
                let failure = result.unwrap_err();
                assert_eq!(failure.error().classification(), "target_failed");
                failure.receipt().clone()
            } else {
                let outcome = result.unwrap();
                assert_eq!(outcome.value(), &Output(5));
                outcome.receipt().clone()
            };
            assert!(receipt.observer_records().iter().any(|record| matches!(
                record.status(),
                InterfaceObserverStatus::Failed | InterfaceObserverStatus::TimedOut
            )));
        }
    }
}

#[test]
fn missing_event_schema_fails_closed_instead_of_using_managed_schema_or_first_sample() {
    struct UndeclaredInput;
    impl InterfaceContract for UndeclaredInput {
        const CONTRACT_ID: &'static str = "undeclared-event-input";
        const CONTRACT_VERSION: &'static str = "1";
        fn managed_projection_schema() -> Option<serde_json::Value> {
            Some(json!({"type":"object"}))
        }
        fn project_for_event(&self) -> Option<serde_json::Value> {
            Some(json!({"sample":"is-not-a-schema"}))
        }
    }
    struct Handler;
    impl InterfaceHandler<UndeclaredInput, Output, TargetError> for Handler {
        fn invoke(
            &self,
            _: InterfaceHandlerContext,
            _: UndeclaredInput,
        ) -> InterfaceHandlerFuture<Output, TargetError> {
            Box::pin(async { Ok(Output(1)) })
        }
    }
    let contracts = InterfaceContracts::unary(
        ContractIdentity::new(UndeclaredInput::CONTRACT_ID, "1").unwrap(),
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
            InterfaceExecutionMode::Unary,
            HandlerReference::new("invocation.handler").unwrap(),
            TargetReference::new("invocation.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner(),
    )
    .with_event_observation();
    let mut compiler = compiler("graph:missing-event-schema");
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
        .bind_handler::<UndeclaredInput, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(Handler),
        )
        .unwrap();
    assert!(
        matches!(compiler.compile(), Err(RegistryCompilationError::EventProjectionMismatch(id)) if id == interface_id())
    );
}

#[derive(Clone, Copy)]
enum StreamOutcome {
    Success,
    Failure,
    Rejected,
    Cancelled,
    Missing,
}
struct StreamHandler(StreamOutcome);
impl InterfaceStreamHandler<Input, StreamEvent, Output, TargetError> for StreamHandler {
    fn invoke_stream(
        &self,
        _: InterfaceHandlerContext,
        _: Input,
    ) -> InterfaceStreamHandlerFuture<StreamEvent, Output, TargetError> {
        let outcome = self.0;
        Box::pin(async move {
            let (publisher, stream) = interface_stream_channel(1);
            match outcome {
                StreamOutcome::Missing => drop(publisher),
                _ => {
                    publisher.emit(StreamEvent(7)).await.unwrap();
                    let terminal = match outcome {
                        StreamOutcome::Success => InterfaceStreamTerminal::Completed(Output(9)),
                        StreamOutcome::Failure => InterfaceStreamTerminal::Failed(
                            InterfaceTargetFailure::new("real-stream-error", TargetError),
                        ),
                        StreamOutcome::Rejected => InterfaceStreamTerminal::Rejected {
                            classification: Arc::from("stream-rejected"),
                        },
                        StreamOutcome::Cancelled => InterfaceStreamTerminal::Cancelled,
                        _ => unreachable!(),
                    };
                    publisher.finish(terminal).await.unwrap();
                }
            }
            Ok(stream)
        })
    }
}
fn stream_snapshot(
    observer: Arc<Observer>,
    outcome: StreamOutcome,
) -> Arc<CompiledInterfaceRegistry> {
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
    )
    .with_event_observation();
    let mut compiler = compiler("graph:stream-events").with_event_observer(observer);
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
        .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(StreamHandler(outcome)),
        )
        .unwrap();
    compiler.compile().unwrap()
}
fn stream_target() -> ExecutionTargetPin {
    ExecutionTargetPin::BuiltIn {
        handler: HandlerReference::new("invocation.handler").unwrap(),
        target: TargetReference::new("invocation.target").unwrap(),
    }
}

#[tokio::test]
async fn stream_completion_waits_for_real_complete_and_preserves_all_terminal_facts() {
    for outcome in [
        StreamOutcome::Success,
        StreamOutcome::Failure,
        StreamOutcome::Rejected,
        StreamOutcome::Cancelled,
        StreamOutcome::Missing,
    ] {
        let (observer, events) = observer(ObserverBehavior::Record);
        let snapshot = stream_snapshot(observer, outcome);
        let stream = kernel(false)
            .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
                snapshot,
                envelope(actor(true)),
                stream_target(),
            )
            .await
            .unwrap();
        let (mut values, completion) = stream.into_parts();
        if !matches!(outcome, StreamOutcome::Missing) {
            assert_eq!(values.recv().await.unwrap(), StreamEvent(7));
        }
        assert_eq!(
            events.lock().unwrap().len(),
            1,
            "terminal delivery must not publish Completion"
        );
        let result = completion.complete().await;
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        let (terminal, error, output) = match outcome {
            StreamOutcome::Success => {
                assert!(result.is_ok());
                (
                    InterfaceInvocationTerminal::Completed,
                    None,
                    Some(json!({"value":9})),
                )
            }
            StreamOutcome::Failure => (
                InterfaceInvocationTerminal::Failed,
                Some("real-stream-error"),
                None,
            ),
            StreamOutcome::Rejected => (
                InterfaceInvocationTerminal::Rejected,
                Some("stream-rejected"),
                None,
            ),
            StreamOutcome::Cancelled => (
                InterfaceInvocationTerminal::Cancelled,
                Some("cancelled"),
                None,
            ),
            StreamOutcome::Missing => (
                InterfaceInvocationTerminal::Failed,
                Some("stream-terminal-missing"),
                None,
            ),
            _ => unreachable!(),
        };
        if error.is_some() {
            assert!(result.is_err());
        }
        assert_completion(&events[1], terminal, error, output);
    }
}

#[tokio::test]
async fn stream_cancellation_and_deadline_complete_once_and_drop_does_not_claim_completion() {
    for deadline in [false, true] {
        let (observer, events) = observer(ObserverBehavior::Record);
        let snapshot = stream_snapshot(observer, StreamOutcome::Success);
        let cancellation = InvocationCancellation::new();
        let request = controlled_envelope(InvocationControls::new(
            if deadline {
                Some(SystemTime::now() + Duration::from_millis(10))
            } else {
                None
            },
            cancellation.clone(),
            None,
        ));
        let stream = kernel(false)
            .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
                snapshot,
                request,
                stream_target(),
            )
            .await
            .unwrap();
        let (_, completion) = stream.into_parts();
        if deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        } else {
            cancellation.cancel();
        }
        assert!(completion.complete().await.is_err());
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_completion(
            &events[1],
            InterfaceInvocationTerminal::Cancelled,
            Some(if deadline {
                "deadline-elapsed"
            } else {
                "cancelled"
            }),
            None,
        );
    }
    let (observer, events) = observer(ObserverBehavior::Record);
    let snapshot = stream_snapshot(observer, StreamOutcome::Success);
    let stream = kernel(false)
        .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
            snapshot,
            envelope(actor(true)),
            stream_target(),
        )
        .await
        .unwrap();
    drop(stream);
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn authentication_rejection_retains_frozen_identity_without_fabricating_principal_or_input() {
    for class in [
        crate::InterfaceAuthenticationRejectionClass::CredentialRejected,
        crate::InterfaceAuthenticationRejectionClass::AuthenticationFailed,
    ] {
        let (observer, events) = observer(ObserverBehavior::Record);
        let snapshot = unary_snapshot(observer, false, true, false);
        let invocation_id = InvocationId::now_v7();
        let attempt = crate::InterfaceAuthenticationAttempt::resolve(
            snapshot.clone(),
            binding().binding_id(),
            InterfaceProtocol::Http,
            InvocationLineage::root(invocation_id),
        )
        .unwrap();
        let receipt = attempt.reject(class).await;
        assert_eq!(receipt.invocation_id(), invocation_id);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_completion(
            &events[0],
            InterfaceInvocationTerminal::Rejected,
            Some(class.as_str()),
            None,
        );
        let InterfaceEvent::Completion { context, input, .. } = &events[0] else {
            unreachable!()
        };
        assert!(input.is_none());
        assert!(context.principal().is_none());
        assert_eq!(context.invocation_id(), invocation_id);
        assert_eq!(context.registry_fingerprint(), snapshot.fingerprint());
    }
}

#[tokio::test]
async fn before_observes_the_actual_prepared_input_after_compiled_typed_mutation() {
    let (observer, events) = observer(ObserverBehavior::Record);
    let mut compiler = event_compiler(observer, true);
    let graph = GraphFingerprint::new("graph:events").unwrap();
    let hooks = TypedInterfaceHookPlan::<Input, Output>::new(graph);
    let plugin = PluginIdentity::new("test.mutate-before-events").unwrap();
    let hooks = hooks.bind_before(
        plugin.clone(),
        Arc::new(RecordingBeforeHook {
            name: "event-input",
            increment: 10,
            events: Arc::new(Mutex::new(Vec::new())),
        }),
    );
    compiler
        .register_extension(
            &interface_id(),
            1,
            hook_registration(plugin.as_str(), InterfaceExtensionPoint::Before),
        )
        .unwrap();
    compiler
        .bind_hook_plan(&interface_id(), Arc::new(hooks))
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(RecordingHandler {
                increment: 3,
                fail: false,
                seen_fingerprint: Arc::new(Mutex::new(None)),
            }),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let outcome = kernel(false)
        .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
        .await
        .unwrap();
    assert_eq!(outcome.value(), &Output(15));
    let events = events.lock().unwrap();
    let InterfaceEvent::Before { input, .. } = &events[0] else {
        unreachable!()
    };
    assert_eq!(*input, json!({"value":12}));
    assert_completion(
        &events[1],
        InterfaceInvocationTerminal::Completed,
        None,
        Some(json!({"value":15})),
    );
}
