use crate::{
    compile_effective_handler, HandlerReference, InterfaceExtensionCompilationError,
    InterfaceExtensionFact, InterfaceExtensionIsolation, InterfaceExtensionPermission,
    InterfaceExtensionPoint, InterfaceExtensionRegistration, InterfaceExtensionTier,
    InterfaceHandlerCandidate, InterfaceId, InterfaceScope, PluginIdentity, TargetReference,
};

fn registration(
    tier: InterfaceExtensionTier,
    point: InterfaceExtensionPoint,
    permission: InterfaceExtensionPermission,
    facts: impl IntoIterator<Item = InterfaceExtensionFact>,
) -> Result<InterfaceExtensionRegistration, InterfaceExtensionCompilationError> {
    InterfaceExtensionRegistration::new(
        PluginIdentity::new("test.extension").unwrap(),
        tier,
        point,
        permission,
        InterfaceScope::Workspace,
        if matches!(
            tier,
            InterfaceExtensionTier::RuntimeExtension | InterfaceExtensionTier::CapabilityPlugin
        ) {
            InterfaceExtensionIsolation::ProcessWire
        } else {
            InterfaceExtensionIsolation::TrustedInProcess
        },
        facts,
    )
}

#[test]
fn approved_points_bind_permission_and_typed_facts() {
    let rows = [
        (
            InterfaceExtensionPoint::Definition,
            InterfaceExtensionPermission::Define,
            vec![InterfaceExtensionFact::DefinitionIdentity],
        ),
        (
            InterfaceExtensionPoint::AuthenticationAdapter,
            InterfaceExtensionPermission::Authenticate,
            vec![],
        ),
        (
            InterfaceExtensionPoint::Authorization,
            InterfaceExtensionPermission::Authorize,
            vec![InterfaceExtensionFact::PrincipalSummary],
        ),
        (
            InterfaceExtensionPoint::Admission,
            InterfaceExtensionPermission::Admit,
            vec![InterfaceExtensionFact::AuthorizationDecision],
        ),
        (
            InterfaceExtensionPoint::Before,
            InterfaceExtensionPermission::MutateInput,
            vec![InterfaceExtensionFact::TypedInput],
        ),
        (
            InterfaceExtensionPoint::Handler,
            InterfaceExtensionPermission::Handle,
            vec![InterfaceExtensionFact::AttemptIdentity],
        ),
        (
            InterfaceExtensionPoint::After,
            InterfaceExtensionPermission::ObserveOutput,
            vec![InterfaceExtensionFact::TypedOutput],
        ),
        (
            InterfaceExtensionPoint::Failure,
            InterfaceExtensionPermission::ObserveFailure,
            vec![InterfaceExtensionFact::FailureClassification],
        ),
        (
            InterfaceExtensionPoint::Completion,
            InterfaceExtensionPermission::ObserveCompletion,
            vec![InterfaceExtensionFact::Terminal],
        ),
    ];
    for (point, permission, facts) in rows {
        assert_eq!(
            registration(
                InterfaceExtensionTier::HostExtension,
                point,
                permission,
                facts,
            )
            .unwrap()
            .point(),
            point
        );
    }
}

#[test]
fn runtime_and_capability_plugins_cannot_authenticate_or_escape_process_wire() {
    for tier in [
        InterfaceExtensionTier::RuntimeExtension,
        InterfaceExtensionTier::CapabilityPlugin,
    ] {
        assert!(matches!(
            registration(
                tier,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                [],
            ),
            Err(InterfaceExtensionCompilationError::IllegalPoint { .. })
        ));
        assert!(matches!(
            InterfaceExtensionRegistration::new(
                PluginIdentity::new("test.extension").unwrap(),
                tier,
                InterfaceExtensionPoint::After,
                InterfaceExtensionPermission::ObserveOutput,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::TypedOutput],
            ),
            Err(InterfaceExtensionCompilationError::IsolationMismatch { .. })
        ));
    }
}

#[test]
fn mutation_illegal_facts_and_multiple_handlers_fail_closed() {
    assert!(matches!(
        registration(
            InterfaceExtensionTier::HostExtension,
            InterfaceExtensionPoint::Before,
            InterfaceExtensionPermission::Authorize,
            [InterfaceExtensionFact::TypedInput],
        ),
        Err(InterfaceExtensionCompilationError::PermissionMismatch { .. })
    ));
    assert!(matches!(
        registration(
            InterfaceExtensionTier::HostExtension,
            InterfaceExtensionPoint::Completion,
            InterfaceExtensionPermission::ObserveCompletion,
            [InterfaceExtensionFact::TypedOutput],
        ),
        Err(InterfaceExtensionCompilationError::IllegalFacts { .. })
    ));

    let interface_id = InterfaceId::new("test.interface").unwrap();
    let candidate = || {
        InterfaceHandlerCandidate::new(
            PluginIdentity::new("test.extension").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            TargetReference::new("test.target").unwrap(),
        )
    };
    assert!(matches!(
        compile_effective_handler(&interface_id, []),
        Err(InterfaceExtensionCompilationError::MissingEffectiveHandler(
            _
        ))
    ));
    assert!(matches!(
        compile_effective_handler(&interface_id, [candidate(), candidate()]),
        Err(InterfaceExtensionCompilationError::MultipleEffectiveHandlers(_))
    ));
    assert_eq!(
        compile_effective_handler(&interface_id, [candidate()])
            .unwrap()
            .handler()
            .as_str(),
        "test.handler"
    );
}
