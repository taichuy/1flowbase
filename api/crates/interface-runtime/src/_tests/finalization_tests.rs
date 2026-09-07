//! Root #1998 A3: unary/stream × cancellation/deadline, immutable success/failure,
//! observer panic/hang/total-budget exhaustion. Executed only in the Root Test Batch.
use super::*;
use crate::{
    CompiledInterfaceRegistry, InterfaceFailureHook, InterfaceFailureHookFuture,
    InterfaceInvocationReceipt, InterfaceObserverStatus, InterfaceTargetFailure,
    InvocationCancellation, InvocationControls,
};

#[derive(Clone, Copy)]
enum Business {
    Success,
    Failure,
    Pending,
}
struct Target {
    business: Business,
    pending: Mutex<Vec<crate::InterfaceStreamPublisher<StreamEvent, Output, TargetError>>>,
}
impl Target {
    fn new(business: Business) -> Self {
        Self {
            business,
            pending: Mutex::new(Vec::new()),
        }
    }
}
impl InterfaceHandler<Input, Output, TargetError> for Target {
    fn invoke(
        &self,
        _: InterfaceHandlerContext,
        _: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        let business = self.business;
        Box::pin(async move {
            match business {
                Business::Success => Ok(Output(42)),
                Business::Failure => {
                    Err(InterfaceTargetFailure::new("business-failed", TargetError))
                }
                Business::Pending => std::future::pending().await,
            }
        })
    }
}
impl InterfaceStreamHandler<Input, StreamEvent, Output, TargetError> for Target {
    fn invoke_stream(
        &self,
        _: InterfaceHandlerContext,
        _: Input,
    ) -> InterfaceStreamHandlerFuture<StreamEvent, Output, TargetError> {
        let business = self.business;
        let (publisher, stream) = interface_stream_channel(1);
        if matches!(business, Business::Pending) {
            self.pending.lock().unwrap().push(publisher);
            return Box::pin(async move { Ok(stream) });
        }
        Box::pin(async move {
            match business {
                Business::Success => publisher
                    .finish(InterfaceStreamTerminal::Completed(Output(42)))
                    .await
                    .unwrap(),
                Business::Failure => publisher
                    .finish(InterfaceStreamTerminal::Failed(
                        InterfaceTargetFailure::new("business-failed", TargetError),
                    ))
                    .await
                    .unwrap(),
                Business::Pending => unreachable!(),
            }
            Ok(stream)
        })
    }
}

#[derive(Clone, Copy)]
enum Observer {
    Quick,
    Cancel,
    Hang,
    ConstructionPanic,
    PollPanic,
}
struct Observation(Observer, InvocationCancellation);
impl Observation {
    fn future(&self) -> InterfaceCompletionHookFuture<'_> {
        if matches!(self.0, Observer::ConstructionPanic) {
            panic!("private panic payload");
        }
        Box::pin(async move {
            match self.0 {
                Observer::Quick => {}
                Observer::Cancel => self.1.cancel(),
                Observer::Hang => std::future::pending().await,
                Observer::PollPanic => panic!("private panic payload"),
                Observer::ConstructionPanic => unreachable!(),
            }
        })
    }
}
impl InterfaceAfterHook<Output> for Observation {
    fn after<'a>(&'a self, _: InterfaceHookContext, _: &'a Output) -> InterfaceAfterHookFuture<'a> {
        self.future()
    }
}
impl InterfaceFailureHook for Observation {
    fn failed<'a>(&'a self, _: InterfaceHookContext, _: &'a str) -> InterfaceFailureHookFuture<'a> {
        self.future()
    }
}
impl InterfaceCompletionHook for Observation {
    fn completed(
        &self,
        _: InterfaceHookContext,
        _: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        self.future()
    }
}

fn snapshot(
    mode: InterfaceExecutionMode,
    business: Business,
    observer: Observer,
    cancellation: &InvocationCancellation,
) -> Arc<CompiledInterfaceRegistry> {
    let mut compiler = compiler();
    let definition = definition("review.finalization", mode);
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.finalization.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/finalization").unwrap()),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    let hook = Arc::new(Observation(observer, cancellation.clone()));
    let mut hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:review").unwrap(),
    );
    for (order, name, point, permission, fact) in [
        (
            10,
            "review.after",
            InterfaceExtensionPoint::After,
            InterfaceExtensionPermission::ObserveOutput,
            InterfaceExtensionFact::TypedOutput,
        ),
        (
            20,
            "review.failure",
            InterfaceExtensionPoint::Failure,
            InterfaceExtensionPermission::ObserveFailure,
            InterfaceExtensionFact::FailureClassification,
        ),
        (
            30,
            "review.completion",
            InterfaceExtensionPoint::Completion,
            InterfaceExtensionPermission::ObserveCompletion,
            InterfaceExtensionFact::Terminal,
        ),
    ] {
        let plugin = PluginIdentity::new(name).unwrap();
        compiler
            .register_extension(
                definition.interface_id(),
                order,
                InterfaceExtensionRegistration::new(
                    plugin.clone(),
                    InterfaceExtensionTier::HostExtension,
                    point,
                    permission,
                    InterfaceScope::Workspace,
                    InterfaceExtensionIsolation::TrustedInProcess,
                    [fact],
                )
                .unwrap(),
            )
            .unwrap();
        hooks = match point {
            InterfaceExtensionPoint::After => hooks.bind_after(plugin, hook.clone()),
            InterfaceExtensionPoint::Failure => hooks.bind_failure(plugin, hook.clone()),
            InterfaceExtensionPoint::Completion => hooks.bind_completion(
                plugin,
                Arc::new(Observation(Observer::Quick, cancellation.clone())),
            ),
            _ => unreachable!(),
        };
    }
    compiler
        .bind_hook_plan(definition.interface_id(), Arc::new(hooks))
        .unwrap();
    if mode == InterfaceExecutionMode::Unary {
        compiler
            .bind_handler::<Input, Output, TargetError, UserPrincipal>(
                definition.interface_id(),
                definition.handler_reference().clone(),
                Arc::new(Target::new(business)),
            )
            .unwrap();
    } else {
        compiler
            .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
                definition.interface_id(),
                definition.handler_reference().clone(),
                Arc::new(Target::new(business)),
            )
            .unwrap();
    }
    compiler.compile().unwrap()
}

async fn invoke(
    mode: InterfaceExecutionMode,
    snapshot: Arc<CompiledInterfaceRegistry>,
    controls: InvocationControls,
) -> Result<InterfaceInvocationReceipt, crate::InterfaceInvocationFailure> {
    let base = envelope(
        "http.review.finalization.v1",
        InterfaceProtocol::Http,
        "review.authn",
    );
    let envelope = InvocationEnvelope::with_principal_and_controls(
        base.lineage().clone(),
        base.binding_id().clone(),
        base.protocol(),
        base.authentication_adapter().clone(),
        base.authentication_activation().clone(),
        base.principal().clone(),
        controls,
        Input(4),
    );
    let kernel = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")));
    if mode == InterfaceExecutionMode::Unary {
        let outcome = kernel
            .invoke::<Input, Output, TargetError>(snapshot, envelope)
            .await?;
        assert_eq!(outcome.value(), &Output(42));
        Ok(outcome.receipt().clone())
    } else {
        let invocation = kernel
            .invoke_server_stream_with_dispatch_target::<Input, StreamEvent, Output, TargetError>(
                Arc::clone(&snapshot),
                envelope,
                ExecutionTargetPin::BuiltIn {
                    handler: HandlerReference::new("review.finalization.handler").unwrap(),
                    target: TargetReference::new("review.finalization.target").unwrap(),
                },
            )
            .await?;
        let (_, completion) = invocation.into_parts();
        // Target owns the pending publisher; keep its registry alive until cancellation
        // or deadline settles completion, rather than closing the terminal channel early.
        let outcome = completion.complete().await;
        drop(snapshot);
        let outcome = outcome?;
        assert_eq!(
            outcome.terminal(),
            &InterfaceStreamTerminal::Completed(Output(42))
        );
        Ok(outcome.receipt().clone())
    }
}

#[tokio::test]
async fn root_1998_cancellation_and_deadline_still_execute_completion() {
    for mode in [
        InterfaceExecutionMode::Unary,
        InterfaceExecutionMode::ServerStream,
    ] {
        for deadline in [false, true] {
            let cancellation = InvocationCancellation::new();
            let snapshot = snapshot(mode, Business::Pending, Observer::Quick, &cancellation);
            let controls = InvocationControls::new(
                deadline.then(|| SystemTime::now() + Duration::from_millis(20)),
                cancellation.clone(),
                None,
            );
            let (outcome, _) = tokio::join!(invoke(mode, snapshot, controls), async {
                if !deadline {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    cancellation.cancel();
                }
            });
            let failure = outcome.unwrap_err();
            assert_eq!(
                failure.receipt().terminal(),
                InterfaceInvocationTerminal::Cancelled
            );
            assert!(matches!(
                (deadline, failure.error()),
                (true, InterfaceInvocationError::DeadlineElapsed)
                    | (false, InterfaceInvocationError::Cancelled)
            ));
            let records = failure.receipt().observer_records();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].point(), InterfaceExtensionPoint::Completion);
            assert_eq!(records[0].status(), InterfaceObserverStatus::Executed);
        }
    }
}

#[tokio::test]
async fn root_1998_observers_never_replace_business_output_or_failure() {
    for mode in [
        InterfaceExecutionMode::Unary,
        InterfaceExecutionMode::ServerStream,
    ] {
        for business in [Business::Success, Business::Failure] {
            for observer in [
                Observer::Quick,
                Observer::Cancel,
                Observer::ConstructionPanic,
                Observer::PollPanic,
                Observer::Hang,
            ] {
                let cancellation = InvocationCancellation::new();
                let snapshot = snapshot(mode, business, observer, &cancellation);
                let started = tokio::time::Instant::now();
                let outcome = invoke(
                    mode,
                    snapshot,
                    InvocationControls::new(None, cancellation, None),
                )
                .await;
                let receipt = match business {
                    Business::Success => outcome.unwrap(),
                    Business::Failure => {
                        let failure = outcome.unwrap_err();
                        assert_eq!(
                            failure.receipt().terminal(),
                            InterfaceInvocationTerminal::Failed
                        );
                        assert!(
                            matches!(failure.error(), InterfaceInvocationError::TargetFailed(error) if error.classification() == "business-failed")
                        );
                        failure.receipt().clone()
                    }
                    Business::Pending => unreachable!(),
                };
                let records = receipt.observer_records();
                assert_eq!(records.len(), 2);
                assert_eq!(
                    records[0].point(),
                    if matches!(business, Business::Success) {
                        InterfaceExtensionPoint::After
                    } else {
                        InterfaceExtensionPoint::Failure
                    }
                );
                assert_eq!(records[1].plugin().as_str(), "review.completion");
                if matches!(observer, Observer::Hang) {
                    assert_eq!(records[0].status(), InterfaceObserverStatus::TimedOut);
                    assert_eq!(records[1].status(), InterfaceObserverStatus::NotRun);
                    assert!(
                        started.elapsed() < Duration::from_millis(1800),
                        "one total 1000ms budget"
                    );
                } else {
                    assert_eq!(
                        records[0].status(),
                        if matches!(observer, Observer::ConstructionPanic | Observer::PollPanic) {
                            InterfaceObserverStatus::Failed
                        } else {
                            InterfaceObserverStatus::Executed
                        }
                    );
                    assert_eq!(records[1].status(), InterfaceObserverStatus::Executed);
                }
                assert!(records
                    .iter()
                    .all(|record| !record.reason().unwrap_or("").contains("private")));
            }
        }
    }
}

#[tokio::test]
async fn root_1998_resolved_plan_early_returns_finalize_once_without_typed_io() {
    for mode in [
        InterfaceExecutionMode::Unary,
        InterfaceExecutionMode::ServerStream,
    ] {
        for expired in [false, true] {
            let cancellation = InvocationCancellation::new();
            if !expired {
                cancellation.cancel();
            }
            let snapshot = snapshot(mode, Business::Pending, Observer::Quick, &cancellation);
            let failure = invoke(
                mode,
                snapshot,
                InvocationControls::new(
                    expired.then(|| SystemTime::now() - Duration::from_secs(1)),
                    cancellation,
                    None,
                ),
            )
            .await
            .unwrap_err();
            assert_eq!(failure.receipt().observer_records().len(), 1);
            assert_eq!(
                failure.receipt().observer_records()[0].status(),
                InterfaceObserverStatus::Executed
            );
        }
        let cancellation = InvocationCancellation::new();
        let snapshot = snapshot(mode, Business::Success, Observer::Quick, &cancellation);
        let kernel = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")));
        let envelope = envelope(
            "http.review.finalization.v1",
            InterfaceProtocol::Http,
            "review.authn",
        );
        let failure = if mode == InterfaceExecutionMode::Unary {
            kernel
                .invoke::<Input, WrongOutput, TargetError>(snapshot, envelope)
                .await
                .err()
                .unwrap()
        } else {
            kernel.invoke_server_stream_with_dispatch_target::<Input, StreamEvent, WrongOutput, TargetError>(snapshot, envelope,
                ExecutionTargetPin::BuiltIn { handler: HandlerReference::new("review.finalization.handler").unwrap(), target: TargetReference::new("review.finalization.target").unwrap() }).await.err().unwrap()
        };
        assert!(matches!(
            failure.error(),
            InterfaceInvocationError::ContractMismatch
        ));
        assert_eq!(failure.receipt().observer_records().len(), 1);
        assert_eq!(
            failure.receipt().observer_records()[0].point(),
            InterfaceExtensionPoint::Completion
        );
        assert_eq!(
            failure.receipt().observer_records()[0].status(),
            InterfaceObserverStatus::Executed
        );
    }
}

// Root #1998 A4: no principal or typed I/O may be invented for authentication failure.
#[tokio::test]
async fn root_1998_authentication_rejection_uses_frozen_plan_and_absent_principal() {
    let seen = Arc::new(Mutex::new(None));
    let mut compiler = compiler();
    let definition = definition("review.finalization", InterfaceExecutionMode::Unary);
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.finalization.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/api/finalization").unwrap()),
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
    let plugin = PluginIdentity::new("review.rejection-observer").unwrap();
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
        .bind_hook_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind_completion(plugin, Arc::new(UnestablishedCompletion(seen.clone()))),
            ),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let lineage = InvocationLineage::root(InvocationId::now_v7());
    let attempt = crate::InterfaceAuthenticationAttempt::resolve(
        snapshot.clone(),
        &BindingId::new("http.review.finalization.v1").unwrap(),
        InterfaceProtocol::Http,
        lineage.clone(),
    )
    .unwrap();
    let receipt = attempt
        .reject(crate::InterfaceAuthenticationRejectionClass::CredentialRejected)
        .await;
    assert_eq!(receipt.invocation_id(), lineage.invocation_id());
    assert_eq!(*seen.lock().unwrap(), Some(lineage.invocation_id()));
    assert!(receipt.principal().is_none());
    assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Rejected);
    assert_eq!(receipt.graph_fingerprint(), snapshot.graph_fingerprint());
    assert_eq!(receipt.registry_fingerprint(), snapshot.fingerprint());
    assert_eq!(receipt.binding_id().as_str(), "http.review.finalization.v1");
    assert_eq!(
        receipt.activation().activation().as_str(),
        "review.authn.activation.v1"
    );
    assert_eq!(
        receipt.observer_records()[0].status(),
        InterfaceObserverStatus::Executed
    );
    assert!(crate::InterfaceAuthenticationAttempt::resolve(
        snapshot,
        &BindingId::new("unknown.binding").unwrap(),
        InterfaceProtocol::Http,
        lineage
    )
    .is_none());
}

struct UnestablishedCompletion(Arc<Mutex<Option<InvocationId>>>);
impl InterfaceCompletionHook for UnestablishedCompletion {
    fn completed(
        &self,
        context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            assert!(context.principal().is_none());
            assert_eq!(terminal, InterfaceInvocationTerminal::Rejected);
            assert_eq!(context.graph_fingerprint().as_str(), "graph:review");
            *self.0.lock().unwrap() = Some(context.invocation_id());
        })
    }
}
