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

