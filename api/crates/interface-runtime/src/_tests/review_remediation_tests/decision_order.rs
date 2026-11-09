use super::*;

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
    decision_snapshot_with_observers(mode, events, authorization, admission, None)
}

fn decision_snapshot_with_observers(
    mode: InterfaceExecutionMode,
    events: Arc<Mutex<Vec<&'static str>>>,
    authorization: DecisionBehavior,
    admission: DecisionBehavior,
    observers: Option<bool>,
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
    let mut hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:review").unwrap(),
    )
    .bind_before(
        before_plugin,
        Arc::new(RecordingBefore(Arc::clone(&events))),
    );
    if let Some(hang) = observers {
        for (order, name) in [
            (40, "policy.failure-low"),
            (50, "policy.failure-high"),
            (60, "policy.completion"),
        ] {
            let completion = order == 60;
            let plugin = PluginIdentity::new(name).unwrap();
            compiler
                .register_extension(
                    definition.interface_id(),
                    order,
                    InterfaceExtensionRegistration::new(
                        plugin.clone(),
                        InterfaceExtensionTier::HostExtension,
                        if completion {
                            InterfaceExtensionPoint::Completion
                        } else {
                            InterfaceExtensionPoint::Failure
                        },
                        if completion {
                            InterfaceExtensionPermission::ObserveCompletion
                        } else {
                            InterfaceExtensionPermission::ObserveFailure
                        },
                        definition.scope(),
                        InterfaceExtensionIsolation::TrustedInProcess,
                        [if completion {
                            InterfaceExtensionFact::Terminal
                        } else {
                            InterfaceExtensionFact::FailureClassification
                        }],
                    )
                    .unwrap(),
                )
                .unwrap();
            hooks = if completion {
                hooks.bind_completion(plugin, Arc::new(RecordingCompletion(Arc::clone(&events))))
            } else {
                hooks.bind_failure(
                    plugin,
                    Arc::new(PolicyFailure {
                        events: Arc::clone(&events),
                        name,
                        hang: hang && order == 50,
                    }),
                )
            };
        }
    }
    compiler
        .bind_hook_plan(definition.interface_id(), Arc::new(hooks))
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
async fn root_1998_ac_006_rr15_rr16_unary_and_stream_execute_core_then_ordered_veto_then_hook_handler(
) {
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
async fn root_1998_ac_006_rr15_core_deny_dominates_plugin_allow_and_extension_failures_fail_closed()
{
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
    let deadline_envelope = InvocationEnvelope::new(
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
    .invoke::<Input, Output, TargetError>(snapshot, deadline_envelope)
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
fn root_1998_ac_006_rr15_rr16_decision_bindings_fail_publish_when_missing_extra_or_contract_mismatched(
) {
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

struct PolicyFailure {
    events: Arc<Mutex<Vec<&'static str>>>,
    name: &'static str,
    hang: bool,
}
impl crate::InterfaceFailureHook for PolicyFailure {
    fn failed<'a>(
        &'a self,
        _: InterfaceHookContext,
        _: &'a str,
    ) -> crate::InterfaceFailureHookFuture<'a> {
        self.events.lock().unwrap().push(self.name);
        if self.hang {
            Box::pin(std::future::pending())
        } else {
            Box::pin(async {})
        }
    }
}

struct PolicyCore {
    events: Arc<Mutex<Vec<&'static str>>>,
    behavior: DecisionBehavior,
    check: fn(&dyn std::any::Any),
    cancel: Option<crate::InvocationCancellation>,
    source: Arc<u8>,
}
fn policy_core(events: Arc<Mutex<Vec<&'static str>>>, behavior: DecisionBehavior) -> PolicyCore {
    PolicyCore {
        events,
        behavior,
        check: |_| {},
        cancel: None,
        source: Arc::new(7),
    }
}
struct PolicyCoreFuture {
    events: Arc<Mutex<Vec<&'static str>>>,
    pending: bool,
    result: Option<Result<(), crate::InterfaceAuthorizationError>>,
}
impl std::future::Future for PolicyCoreFuture {
    type Output = Result<(), crate::InterfaceAuthorizationError>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        this.events.lock().unwrap().push("core-poll");
        if this.pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(this.result.take().unwrap())
        }
    }
}
impl Drop for PolicyCoreFuture {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("core-drop");
    }
}
impl<P: crate::InvocationPrincipal> InterfaceAuthorizationPort<P> for PolicyCore {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("review.authz").unwrap()
    }
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest<P>,
    ) -> InterfaceAuthorizationFuture<'_> {
        (self.check)(request.principal());
        self.events.lock().unwrap().push("core-authorization");
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        Box::pin(PolicyCoreFuture {
            events: Arc::clone(&self.events),
            pending: matches!(self.behavior, DecisionBehavior::Delay),
            result: Some(if matches!(self.behavior, DecisionBehavior::Reject) {
                Err(crate::InterfaceAuthorizationError::with_source(
                    "core-deny",
                    Arc::clone(&self.source),
                ))
            } else {
                Ok(())
            }),
        })
    }
}
struct PolicyAdmission(Arc<u8>);
impl InterfaceTargetAdmissionPort for PolicyAdmission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("review.admission").unwrap()
    }
    fn admit(&self, _: InterfaceTargetAdmissionRequest) -> InterfaceTargetAdmissionFuture<'_> {
        Box::pin(std::future::ready(Err(
            crate::InterfaceTargetAdmissionError::with_source(
                "admission-deny",
                Arc::clone(&self.0),
            ),
        )))
    }
}

#[tokio::test]
async fn policy_boundary_drops_core_future_before_veto_or_failure_and_retains_rejection_sources() {
    for (core, authorization, admission, core_admission_reject) in [
        (
            DecisionBehavior::Reject,
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
            false,
        ),
        (
            DecisionBehavior::Allow,
            DecisionBehavior::Reject,
            DecisionBehavior::Allow,
            false,
        ),
        (
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
            DecisionBehavior::Reject,
            false,
        ),
        (
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
            true,
        ),
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let snapshot = decision_snapshot_with_observers(
            InterfaceExecutionMode::Unary,
            Arc::clone(&events),
            authorization,
            admission,
            Some(false),
        );
        let core = policy_core(Arc::clone(&events), core);
        let source = Arc::clone(&core.source);
        let target_admission: Arc<dyn InterfaceTargetAdmissionPort> = if core_admission_reject {
            Arc::new(PolicyAdmission(Arc::clone(&source)))
        } else {
            Arc::new(RecordingCoreAdmission(Arc::clone(&events)))
        };
        let failure =
            InterfaceInvocationKernel::with_target_admission(Arc::new(core), target_admission)
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
        assert_eq!(
            failure.receipt().terminal(),
            InterfaceInvocationTerminal::Rejected
        );
        assert!(!failure
            .receipt()
            .stages()
            .any(|stage| stage == InterfaceInvocationStage::Admitted));
        let trace = events.lock().unwrap();
        assert_eq!(
            &trace[..3],
            ["core-authorization", "core-poll", "core-drop"]
        );
        assert_eq!(
            &trace[trace.len() - 3..],
            ["policy.failure-high", "policy.failure-low", "completion"]
        );
        assert!(!trace.contains(&"handler") && !trace.contains(&"before"));
        let authorized = matches!(
            failure.error(),
            InterfaceInvocationError::AdmissionRejected(_)
                | InterfaceInvocationError::AdmissionContributionRejected(_)
        );
        assert_eq!(
            failure.receipt().authorization_decision().is_some(),
            authorized
        );
        assert_eq!(
            failure
                .receipt()
                .stages()
                .any(|stage| stage == InterfaceInvocationStage::Authorized),
            authorized
        );
        drop(trace);
        match failure.into_error() {
            InterfaceInvocationError::AuthorizationRejected(error) => assert!(Arc::ptr_eq(
                &error.into_source::<Arc<u8>>().unwrap(),
                &source
            )),
            InterfaceInvocationError::AdmissionRejected(error) => assert!(Arc::ptr_eq(
                &error.into_source::<Arc<u8>>().unwrap(),
                &source
            )),
            InterfaceInvocationError::AuthorizationContributionRejected(error) => {
                assert_eq!(error.classification(), "extension-authorization-deny")
            }
            InterfaceInvocationError::AdmissionContributionRejected(error) => {
                assert_eq!(error.classification(), "extension-admission-reject")
            }
            _ => panic!("original rejection variant must be retained"),
        }
    }
}

#[tokio::test]
async fn policy_boundary_checks_controls_after_core_construction_and_never_runs_failure_on_interrupt(
) {
    for (construct_cancel, deadline) in [(true, false), (false, false), (false, true)] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let snapshot = decision_snapshot_with_observers(
            InterfaceExecutionMode::Unary,
            Arc::clone(&events),
            DecisionBehavior::Allow,
            DecisionBehavior::Allow,
            Some(false),
        );
        let cancel = crate::InvocationCancellation::new();
        let mut core = policy_core(Arc::clone(&events), DecisionBehavior::Delay);
        if construct_cancel {
            core.cancel = Some(cancel.clone());
        }
        let controls = crate::InvocationControls::new(
            deadline.then(|| SystemTime::now() + Duration::from_millis(50)),
            cancel.clone(),
            None,
        );
        let base = envelope(
            "http.review.decision-unary.v1",
            InterfaceProtocol::Http,
            "review.authn",
        );
        let input = InvocationEnvelope::with_principal_and_controls(
            base.lineage().clone(),
            base.binding_id().clone(),
            base.protocol(),
            base.authentication_adapter().clone(),
            base.authentication_activation().clone(),
            base.principal().clone(),
            controls,
            Input(4),
        );
        let kernel = InterfaceInvocationKernel::with_target_admission(
            Arc::new(core),
            Arc::new(RecordingCoreAdmission(Arc::clone(&events))),
        );
        let invocation = kernel.invoke::<Input, Output, TargetError>(snapshot, input);
        let result = if !construct_cancel && !deadline {
            let (result, _) = tokio::join!(invocation, async {
                tokio::time::sleep(Duration::from_millis(5)).await;
                cancel.cancel();
            });
            result
        } else {
            invocation.await
        };
        let failure = result.unwrap_err();
        assert_eq!(
            failure.receipt().terminal(),
            InterfaceInvocationTerminal::Cancelled
        );
        assert!(matches!(
            (deadline, failure.error()),
            (true, InterfaceInvocationError::DeadlineElapsed)
                | (false, InterfaceInvocationError::Cancelled)
        ));
        let trace = events.lock().unwrap();
        assert_eq!(trace.first(), Some(&"core-authorization"));
        assert_eq!(&trace[trace.len() - 2..], ["core-drop", "completion"]);
        if construct_cancel {
            assert!(!trace.contains(&"core-poll"));
        }
        assert!(trace.iter().all(|name| !name.starts_with("policy.failure")));
        assert!(!failure
            .receipt()
            .stages()
            .any(|stage| stage == InterfaceInvocationStage::Authorized));
    }
}

#[derive(Debug)]
struct PolicyValue(std::marker::PhantomPinned);
impl InterfaceContract for PolicyValue {
    const CONTRACT_ID: &'static str = "review-policy-value";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(serde_json::json!({"type":"null"}))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::Value::Null)
    }
}
struct PolicyHandler;
impl<P: crate::InvocationPrincipal> InterfaceHandler<PolicyValue, PolicyValue, PolicyValue, P>
    for PolicyHandler
{
    fn invoke(
        &self,
        _: InterfaceHandlerContext<P>,
        input: PolicyValue,
    ) -> InterfaceHandlerFuture<PolicyValue, PolicyValue> {
        Box::pin(async move { Ok(input) })
    }
}
struct PolicyFreeze(Arc<Mutex<Vec<&'static str>>>);
impl crate::ManagedInterfaceInvocationFactory for PolicyFreeze {
    fn freeze(
        &self,
        _: crate::ManagedInterfaceFreezeRequest,
    ) -> crate::ManagedInterfaceFreezeFuture<'_> {
        self.0.lock().unwrap().push("freeze");
        Box::pin(async { Err("freeze-deny") })
    }
}
async fn policy_profile<P: crate::InvocationPrincipal>(
    principal: P,
    check: fn(&dyn std::any::Any),
    managed: bool,
) {
    let contract = ContractIdentity::new(PolicyValue::CONTRACT_ID, "1").unwrap();
    let definition = InterfaceDefinition::new(
        identity("review.policy-profile"),
        InterfaceContracts::unary(contract.clone(), contract.clone(), contract),
        InterfaceAccess::new(
            P::PROFILE,
            if P::PROFILE == PrincipalProfile::Public {
                InterfaceAuthenticationPolicy::Anonymous
            } else {
                InterfaceAuthenticationPolicy::Authenticated
            },
            AuthorizationOperation::new("review.invoke").unwrap(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new("review.policy-profile.handler").unwrap(),
            TargetReference::new("review.policy-profile.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        InterfaceOwner::new("review.owner").unwrap(),
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut compiler = compiler();
    if managed {
        compiler = compiler.with_managed_invocations(Arc::new(PolicyFreeze(Arc::clone(&events))));
    }
    compiler.register_definition(definition.clone()).unwrap();
    activate_authentication(&mut compiler, &definition, "review.authn");
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.review.policy-profile.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(
                    RouteIdentity::new("POST", "/api/policy-profile").unwrap(),
                ),
            ),
            plan("review.authn", "review.authz"),
        )
        .unwrap();
    compiler
        .bind_handler::<PolicyValue, PolicyValue, PolicyValue, P>(
            definition.interface_id(),
            definition.handler_reference().clone(),
            Arc::new(PolicyHandler),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let mut core = policy_core(Arc::clone(&events), DecisionBehavior::Allow);
    core.check = check;
    let expected = principal.summary();
    let kernel = InterfaceInvocationKernel::<P>::new(Arc::new(core));
    let invocation = kernel.invoke::<PolicyValue, PolicyValue, PolicyValue>(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            BindingId::new("http.review.policy-profile.v1").unwrap(),
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new("review.authn").unwrap(),
            AuthenticationActivationIdentity::new("review.authn.activation.v1").unwrap(),
            principal,
            None,
            PolicyValue(std::marker::PhantomPinned),
        ),
    );
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&invocation);
    assert!(
        events.lock().unwrap().is_empty(),
        "unpolled unary construction is lazy"
    );
    let result = invocation.await;
    if managed {
        let failure = result.unwrap_err();
        assert!(
            matches!(failure.error(), InterfaceInvocationError::BeforeHookRejected(error) if error.classification() == "freeze-deny")
        );
        assert_eq!(*events.lock().unwrap(), ["freeze"]);
    } else {
        let result = result.unwrap();
        assert_eq!(result.receipt().principal(), &expected);
        assert_eq!(
            *events.lock().unwrap(),
            ["core-authorization", "core-poll", "core-drop"]
        );
        assert!(result
            .receipt()
            .stages()
            .any(|stage| stage == InterfaceInvocationStage::Admitted));
    }
}

#[tokio::test]
async fn policy_boundary_retains_full_public_user_application_principals_and_freeze_precedes_construction(
) {
    policy_profile(
        crate::PublicPrincipal::new(),
        |p| assert!(p.is::<crate::PublicPrincipal>()),
        false,
    )
    .await;
    policy_profile(
        UserPrincipal::with_authenticated_session(
            actor(),
            crate::AuthenticatedSessionIdentity::new("policy-session").unwrap(),
        ),
        |p| {
            assert_eq!(
                p.downcast_ref::<UserPrincipal>()
                    .unwrap()
                    .authenticated_session()
                    .unwrap()
                    .expose_to_trusted_handler(),
                "policy-session"
            )
        },
        false,
    )
    .await;
    let application_actor = actor();
    policy_profile(
        crate::ApplicationPrincipal::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            application_actor.current_workspace_id,
            application_actor,
        )
        .unwrap(),
        |p| {
            assert_eq!(
                p.downcast_ref::<crate::ApplicationPrincipal>()
                    .unwrap()
                    .authorized_actor()
                    .effective_display_role,
                "root"
            )
        },
        false,
    )
    .await;
    policy_profile(
        UserPrincipal::server_delegation(actor()),
        |_| panic!("freeze failure must not construct core authorization"),
        true,
    )
    .await;
}

struct PolicyAlias;
impl InterfaceContract for PolicyAlias {
    const CONTRACT_ID: &'static str = Input::CONTRACT_ID;
    const CONTRACT_VERSION: &'static str = Input::CONTRACT_VERSION;
}
#[tokio::test]
async fn policy_boundary_skips_real_typed_none_even_when_contract_identities_match() {
    let (mut compiler, definition) = bare_decision_compiler("review.policy-none");
    let events = Arc::new(Mutex::new(Vec::new()));
    for (order, name, point, permission) in [
        (
            10,
            "policy.none-auth",
            InterfaceExtensionPoint::Authorization,
            InterfaceExtensionPermission::Authorize,
        ),
        (
            20,
            "policy.none-admit",
            InterfaceExtensionPoint::Admission,
            InterfaceExtensionPermission::Admit,
        ),
    ] {
        compiler
            .register_extension(
                definition.interface_id(),
                order,
                InterfaceExtensionRegistration::new(
                    PluginIdentity::new(name).unwrap(),
                    InterfaceExtensionTier::HostExtension,
                    point,
                    permission,
                    definition.scope(),
                    InterfaceExtensionIsolation::TrustedInProcess,
                    [InterfaceExtensionFact::PrincipalSummary],
                )
                .unwrap(),
            )
            .unwrap();
    }
    compiler
        .bind_authorization_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAuthorizationPlan::<PolicyAlias, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    PluginIdentity::new("policy.none-auth").unwrap(),
                    Arc::new(RecordingAuthorizationContribution {
                        events: Arc::clone(&events),
                        behavior: DecisionBehavior::Reject,
                    }),
                ),
            ),
        )
        .unwrap();
    compiler
        .bind_admission_plan(
            definition.interface_id(),
            Arc::new(
                TypedInterfaceAdmissionPlan::<PolicyAlias, Output>::new(
                    GraphFingerprint::new("graph:review").unwrap(),
                )
                .bind(
                    PluginIdentity::new("policy.none-admit").unwrap(),
                    Arc::new(RecordingAdmissionContribution {
                        events: Arc::clone(&events),
                        behavior: DecisionBehavior::Reject,
                    }),
                ),
            ),
        )
        .unwrap();
    let snapshot = compiler.compile().unwrap();
    let plan = snapshot
        .plan(&BindingId::new("http.review.policy-none.v1").unwrap())
        .unwrap();
    assert!(
        plan.authorization_plan::<Input, Output>().is_none()
            && plan.admission_plan::<Input, Output>().is_none()
    );
    let result = InterfaceInvocationKernel::new(Arc::new(RecordingCoreAuthorization {
        events: Arc::clone(&events),
        reject: false,
    }))
    .invoke::<Input, Output, TargetError>(
        snapshot,
        envelope(
            "http.review.policy-none.v1",
            InterfaceProtocol::Http,
            "review.authn",
        ),
    )
    .await
    .unwrap();
    assert_eq!(result.value(), &Output(4));
    assert_eq!(*events.lock().unwrap(), ["core-authorization"]);
    assert!(result
        .receipt()
        .stages()
        .any(|stage| stage == InterfaceInvocationStage::Admitted));
}

#[tokio::test]
async fn policy_boundary_rejection_failure_and_completion_keep_one_budget() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let snapshot = decision_snapshot_with_observers(
        InterfaceExecutionMode::Unary,
        Arc::clone(&events),
        DecisionBehavior::Allow,
        DecisionBehavior::Allow,
        Some(true),
    );
    let failure = InterfaceInvocationKernel::with_target_admission(
        Arc::new(policy_core(Arc::clone(&events), DecisionBehavior::Reject)),
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
    let records = failure.receipt().observer_records();
    assert_eq!(
        records
            .iter()
            .map(|record| record.plugin().as_str())
            .collect::<Vec<_>>(),
        [
            "policy.failure-high",
            "policy.failure-low",
            "policy.completion"
        ]
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record.status())
            .collect::<Vec<_>>(),
        [
            crate::InterfaceObserverStatus::TimedOut,
            crate::InterfaceObserverStatus::NotRun,
            crate::InterfaceObserverStatus::NotRun
        ]
    );
    assert!(records
        .iter()
        .all(|record| record.reason() == Some("finalization-budget-exhausted")));
    assert_eq!(failure.error().classification(), "core-deny");
    assert_eq!(
        *events.lock().unwrap(),
        [
            "core-authorization",
            "core-poll",
            "core-drop",
            "policy.failure-high"
        ]
    );
}
