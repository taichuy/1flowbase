//! Root #1998 F9: final HTTP variant selection consumes a frozen authentication attempt.
use super::*;
use crate::{
    CompiledInterfaceRegistry, DynamicInterfaceRegistry, HttpSiblingAuthenticationError,
    InterfaceAuthenticationAttempt,
};

fn sibling_snapshot(graph: &str) -> Arc<CompiledInterfaceRegistry> {
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new(graph).unwrap(),
        [AuthorizationOperation::new("review.invoke").unwrap()],
        [InterfaceOwner::new("review.owner").unwrap()],
    );
    for (name, method, path, adapter, activation, streaming, http) in [
        (
            "unary",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "stream",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v1",
            true,
            true,
        ),
        (
            "method",
            "GET",
            "/responses",
            "review.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "route",
            "POST",
            "/other",
            "review.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "adapter",
            "POST",
            "/responses",
            "other.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "activation",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v2",
            false,
            true,
        ),
        (
            "plugin",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "tier",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v1",
            false,
            true,
        ),
        (
            "protocol",
            "POST",
            "/responses",
            "review.authn",
            "review.authn.v1",
            false,
            false,
        ),
    ] {
        let mode = if streaming {
            InterfaceExecutionMode::ServerStream
        } else {
            InterfaceExecutionMode::Unary
        };
        let definition = definition(&format!("review.sibling.{name}"), mode);
        compiler.register_definition(definition.clone()).unwrap();
        let plugin = PluginIdentity::new(if name == "plugin" {
            "other.authentication"
        } else {
            "review.authentication"
        })
        .unwrap();
        let tier = if name == "tier" {
            InterfaceExtensionTier::HostExtension
        } else {
            InterfaceExtensionTier::BuiltIn
        };
        compiler
            .register_authentication_adapter(
                definition.interface_id(),
                1,
                InterfaceExtensionRegistration::new(
                    plugin.clone(),
                    tier,
                    InterfaceExtensionPoint::AuthenticationAdapter,
                    InterfaceExtensionPermission::Authenticate,
                    InterfaceScope::Workspace,
                    InterfaceExtensionIsolation::TrustedInProcess,
                    [],
                )
                .unwrap(),
                ActivatedAuthenticationAdapter::new(
                    plugin,
                    tier,
                    AuthenticationAdapterReference::new(adapter).unwrap(),
                    AuthenticationActivationIdentity::new(activation).unwrap(),
                    PrincipalProfile::User,
                ),
            )
            .unwrap();
        compiler
            .register_binding(
                ProtocolBinding::new(
                    BindingId::new(format!("http.sibling.{name}")).unwrap(),
                    definition.identity().clone(),
                    definition.contracts().clone(),
                    if name == "unary" {
                        ProtocolProjection::http(RouteIdentity::new(method, path).unwrap())
                    } else if http {
                        ProtocolProjection::http_variant(
                            RouteIdentity::new(method, path).unwrap(),
                            name,
                        )
                    } else {
                        ProtocolProjection::mcp("responses")
                    },
                ),
                plan(adapter, "review.authz"),
            )
            .unwrap();
        if streaming {
            compiler
                .bind_stream_handler::<Input, StreamEvent, Output, TargetError, UserPrincipal>(
                    definition.interface_id(),
                    definition.handler_reference().clone(),
                    Arc::new(StreamingHandler),
                )
                .unwrap();
        } else {
            compiler
                .bind_handler::<Input, Output, TargetError, UserPrincipal>(
                    definition.interface_id(),
                    definition.handler_reference().clone(),
                    Arc::new(ContributedUnaryHandler),
                )
                .unwrap();
        }
    }
    compiler.compile().unwrap()
}

fn attempt(
    snapshot: Arc<CompiledInterfaceRegistry>,
    protocol: InterfaceProtocol,
) -> InterfaceAuthenticationAttempt {
    InterfaceAuthenticationAttempt::resolve(
        snapshot,
        &BindingId::new("http.sibling.stream").unwrap(),
        protocol,
        InvocationLineage::root(InvocationId::now_v7())
            .child(InvocationId::now_v7())
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn root_1998_f9_http_sibling_keeps_lineage_snapshot_and_selected_kernel_plan() {
    let original = sibling_snapshot("graph:original");
    let registry = DynamicInterfaceRegistry::new(original.clone());
    let attempt = attempt(registry.snapshot(), InterfaceProtocol::Http);
    let lineage = attempt.lineage().clone();
    registry.publish(sibling_snapshot("graph:replacement"));
    let selected = BindingId::new("http.sibling.unary").unwrap();
    let envelope = attempt
        .into_same_http_carrier_envelope(
            &selected,
            UserPrincipal::server_delegation(actor()),
            Input(4),
        )
        .unwrap();
    assert_eq!(envelope.lineage(), &lineage);
    assert_eq!(envelope.binding_id(), &selected);
    let outcome = InterfaceInvocationKernel::new(Arc::new(Authorization("review.authz")))
        .invoke::<Input, Output, TargetError>(original.clone(), envelope)
        .await
        .unwrap();
    assert_eq!(outcome.value(), &Output(44));
    let receipt = outcome.receipt();
    assert_eq!(receipt.invocation_id(), lineage.invocation_id());
    assert_eq!(
        receipt.parent_invocation_id(),
        lineage.parent_invocation_id()
    );
    assert_eq!(receipt.graph_fingerprint(), original.graph_fingerprint());
    assert_eq!(receipt.registry_fingerprint(), original.fingerprint());
    assert_ne!(
        receipt.registry_fingerprint(),
        registry.snapshot().fingerprint()
    );
    assert_eq!(receipt.resolved().unwrap().binding_id(), &selected);
    assert_eq!(
        receipt.resolved().unwrap().plan_fingerprint(),
        original.plan(&selected).unwrap().fingerprint()
    );
}

#[test]
fn root_1998_f9_http_sibling_rejects_unknown_carrier_authentication_and_typed_contract_mismatch() {
    let snapshot = sibling_snapshot("graph:negatives");
    for (name, expected) in [
        ("unknown", HttpSiblingAuthenticationError::UnknownBinding),
        ("method", HttpSiblingAuthenticationError::CarrierMismatch),
        ("route", HttpSiblingAuthenticationError::CarrierMismatch),
        ("protocol", HttpSiblingAuthenticationError::CarrierMismatch),
        (
            "adapter",
            HttpSiblingAuthenticationError::AuthenticationMismatch,
        ),
        (
            "activation",
            HttpSiblingAuthenticationError::AuthenticationMismatch,
        ),
        (
            "plugin",
            HttpSiblingAuthenticationError::AuthenticationMismatch,
        ),
        (
            "tier",
            HttpSiblingAuthenticationError::AuthenticationMismatch,
        ),
    ] {
        let result = attempt(snapshot.clone(), InterfaceProtocol::Http)
            .into_same_http_carrier_envelope(
                &BindingId::new(format!("http.sibling.{name}")).unwrap(),
                UserPrincipal::server_delegation(actor()),
                Input(4),
            );
        assert_eq!(result.err(), Some(expected), "{name}");
    }
    let binding = BindingId::new("http.sibling.unary").unwrap();
    assert_eq!(
        attempt(snapshot.clone(), InterfaceProtocol::Mcp)
            .into_same_http_carrier_envelope(
                &binding,
                UserPrincipal::server_delegation(actor()),
                Input(4),
            )
            .err(),
        Some(HttpSiblingAuthenticationError::CarrierMismatch)
    );
    assert_eq!(
        attempt(snapshot.clone(), InterfaceProtocol::Http)
            .into_same_http_carrier_envelope(
                &binding,
                UserPrincipal::server_delegation(actor()),
                WrongInput,
            )
            .err(),
        Some(HttpSiblingAuthenticationError::ContractMismatch)
    );
    assert_eq!(
        attempt(snapshot, InterfaceProtocol::Http)
            .into_same_http_carrier_envelope(&binding, crate::PublicPrincipal::new(), Input(4),)
            .err(),
        Some(HttpSiblingAuthenticationError::ContractMismatch)
    );
}
