use std::sync::Arc;

use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, ContractIdentity, GraphFingerprint, HandlerReference, InterfaceAccess,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope,
    InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan, PrincipalProfile,
    ProtocolBinding, ProtocolProjection, PublicPrincipal, RegistryCompiler, RouteIdentity,
    TargetReference,
};

use crate::{
    external_endpoint_catalog::{
        is_approved_external_control_http, ExternalEndpointCatalogCompiler,
        ExternalEndpointCatalogError, ExternalEndpointClassification, ExternalEndpointContribution,
        ExternalEndpointIdentity,
    },
    routes::console_route_assembly::migrated_core_console_route_assembly,
};

#[derive(Debug)]
struct FixtureInput;

impl interface_runtime::InterfaceContract for FixtureInput {
    const CONTRACT_ID: &'static str = "external-endpoint-fixture-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[test]
fn eil_f14r_d2_console_health_is_compiled_as_operational_control() {
    assert!(is_approved_external_control_http(
        "GET",
        "/api/console/health"
    ));
    assert!(!is_approved_external_control_http(
        "GET",
        "/api/console/settings/system-backups"
    ));
}

#[derive(Debug)]
struct FixtureOutput;

impl interface_runtime::InterfaceContract for FixtureOutput {
    const CONTRACT_ID: &'static str = "external-endpoint-fixture-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Debug)]
struct FixtureError;

impl interface_runtime::InterfaceContract for FixtureError {
    const CONTRACT_ID: &'static str = "external-endpoint-fixture-error";
    const CONTRACT_VERSION: &'static str = "1";
}

struct FixtureHandler;

impl interface_runtime::InterfaceHandler<FixtureInput, FixtureOutput, FixtureError, PublicPrincipal>
    for FixtureHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        _input: FixtureInput,
    ) -> InterfaceHandlerFuture<FixtureOutput, FixtureError> {
        Box::pin(async {
            Ok(FixtureOutput)
                .map_err(|error| InterfaceTargetFailure::new("external_endpoint_fixture", error))
        })
    }
}

fn compiled_fixture_registry(
    route_path: &str,
) -> Arc<interface_runtime::CompiledInterfaceRegistry> {
    let interface_id = interface_runtime::InterfaceId::new("fixture.external-endpoint").unwrap();
    let version = InterfaceVersion::new("1").unwrap();
    let identity = InterfaceIdentity::new(interface_id.clone(), version.clone());
    let contracts = InterfaceContracts::unary(
        contract::<FixtureInput>(),
        contract::<FixtureOutput>(),
        contract::<FixtureError>(),
    );
    let operation = AuthorizationOperation::new("fixture.external-endpoint.read").unwrap();
    let owner = InterfaceOwner::new("api-server.external-endpoint-fixture").unwrap();
    let definition = InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            PrincipalProfile::Public,
            InterfaceAuthenticationPolicy::Anonymous,
            operation.clone(),
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new("fixture.external-endpoint.handler").unwrap(),
            TargetReference::new("fixture.external-endpoint.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    );
    let binding = ProtocolBinding::new(
        BindingId::new("fixture.external-endpoint.http").unwrap(),
        identity,
        contracts,
        ProtocolProjection::http(RouteIdentity::new("GET", route_path).unwrap()),
    );
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("fixture-graph").unwrap(),
        [operation],
        [owner],
    );
    compiler.register_definition(definition).unwrap();
    compiler
        .register_authentication_adapter(
            &interface_id,
            1,
            interface_runtime::InterfaceExtensionRegistration::new(
                interface_runtime::PluginIdentity::new("fixture.public-authentication").unwrap(),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
                interface_runtime::InterfaceExtensionPermission::Authenticate,
                InterfaceScope::System,
                interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .unwrap(),
            interface_runtime::ActivatedAuthenticationAdapter::new(
                interface_runtime::PluginIdentity::new("fixture.public-authentication").unwrap(),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("fixture.public").unwrap(),
                interface_runtime::AuthenticationActivationIdentity::new(
                    "fixture.public.activation.v1",
                )
                .unwrap(),
                PrincipalProfile::Public,
            ),
        )
        .unwrap();
    compiler
        .register_binding(
            binding,
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new("fixture.public").unwrap(),
                AuthorizationAdapterReference::new("fixture.authorization").unwrap(),
                None,
            ),
        )
        .unwrap();
    compiler
        .bind_handler::<FixtureInput, FixtureOutput, FixtureError, PublicPrincipal>(
            &interface_id,
            HandlerReference::new("fixture.external-endpoint.handler").unwrap(),
            Arc::new(FixtureHandler),
        )
        .unwrap();
    compiler.compile().unwrap()
}

fn contract<T: interface_runtime::InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).unwrap()
}

#[test]
fn eil_f01_console_assembly_is_a_real_catalog_source_and_defaults_to_unclassified() {
    let assembly = migrated_core_console_route_assembly();
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    for contribution in assembly.external_endpoint_contributions() {
        compiler.contribute(contribution).unwrap();
    }
    let catalog = compiler.compile();

    assert_eq!(catalog.rows().len(), assembly.bindings().len());
    assert_eq!(
        catalog.classification_count(ExternalEndpointClassification::Unclassified),
        assembly.bindings().len()
    );
}

#[test]
fn eil_f01_compiled_binding_classifies_the_same_router_row_as_canonical_business() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::unclassified_http(
            "router",
            "GET",
            "/api/console/fixture/{fixture_id}",
        ))
        .unwrap();
    compiler
        .absorb_registry(
            "compiled-interface-registry",
            &compiled_fixture_registry("/api/console/fixture/:fixture_id"),
        )
        .unwrap();
    let catalog = compiler.compile();
    let row = catalog
        .row(&ExternalEndpointIdentity::http(
            "GET",
            "/api/console/fixture/:id",
        ))
        .unwrap();

    assert_eq!(
        row.classification(),
        ExternalEndpointClassification::CanonicalBusinessInterface
    );
    assert_eq!(row.binding_id(), Some("fixture.external-endpoint.http"));
    assert_eq!(row.sources().len(), 2);
}

#[test]
fn eil_f01_duplicate_source_identity_fails_closed() {
    let contribution =
        ExternalEndpointContribution::unclassified_http("router", "POST", "/api/public/fixture");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler.contribute(contribution.clone()).unwrap();

    assert!(matches!(
        compiler.contribute(contribution),
        Err(ExternalEndpointCatalogError::DuplicateContribution { .. })
    ));
}

#[test]
fn eil_f01_business_and_control_classification_conflict_fails_closed() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .absorb_registry(
            "compiled-interface-registry",
            &compiled_fixture_registry("/health"),
        )
        .unwrap();

    assert!(matches!(
        compiler.contribute_approved_controls(true),
        Err(ExternalEndpointCatalogError::ConflictingClassification { .. })
    ));
}

#[test]
fn eil_f01_operational_control_is_explicit_not_a_default() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    for contribution in crate::root_external_endpoint_contributions(true) {
        compiler.contribute(contribution).unwrap();
    }
    compiler.contribute_approved_controls(true).unwrap();
    let catalog = compiler.compile();

    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http("GET", "/health"))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::OperationalControl
    );
}

#[test]
fn eil_f02_unknown_control_cannot_be_declared_by_a_route() {
    let compiler = ExternalEndpointCatalogCompiler::default();

    assert!(matches!(
        compiler.reject_unapproved_control(ExternalEndpointIdentity::http(
            "POST",
            "/api/console/arbitrary-control",
        )),
        ExternalEndpointCatalogError::UnknownControl { .. }
    ));
}

#[test]
fn eil_f02_cors_and_head_are_derived_only_from_existing_routes() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::unclassified_http(
            "router",
            "GET",
            "/api/console/widgets",
        ))
        .unwrap();
    compiler.contribute_approved_controls(false).unwrap();
    let catalog = compiler.compile();

    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "OPTIONS",
                "/api/console/widgets",
                "cors-preflight",
            ))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::ProtocolControl
    );
    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "HEAD",
                "/api/console/widgets",
                "get-mirror",
            ))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::ProtocolControl
    );
}

#[test]
fn eil_f02_workflow_extension_options_is_never_derived_as_protocol_control() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::unclassified_http(
            "workflow-extension-router",
            "ANY",
            "/api/ex/*slug",
        ))
        .unwrap();
    compiler.contribute_approved_controls(false).unwrap();
    let catalog = compiler.compile();

    assert!(catalog
        .row(&ExternalEndpointIdentity::http_variant(
            "OPTIONS",
            "/api/ex/*slug",
            "cors-preflight",
        ))
        .is_none());
}

#[test]
fn eil_f14_complete_catalog_rejects_every_unclassified_route() {
    let registry = compiled_fixture_registry("/api/console/fixture");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::unclassified_http(
            "router",
            "POST",
            "/api/public/unbound",
        ))
        .unwrap();

    assert!(matches!(
        compiler.compile_complete(registry.as_ref()),
        Err(ExternalEndpointCatalogError::UnclassifiedRows { .. })
    ));
}

#[test]
fn eil_f14_mcp_business_method_requires_a_real_frozen_binding() {
    let registry = compiled_fixture_registry("/api/console/fixture");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute_mcp_protocol_surface("missing.mcp.binding")
        .unwrap();
    compiler.contribute_approved_controls(false).unwrap();

    assert!(matches!(
        compiler.compile_complete(registry.as_ref()),
        Err(ExternalEndpointCatalogError::UnknownBinding { binding_id, .. })
            if binding_id == "missing.mcp.binding"
    ));
}

#[test]
fn eil_f14_openapi_inventory_preserves_websocket_upgrade_as_transport_control() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute_openapi_document(
            "openapi",
            &serde_json::json!({
                "paths": {
                    "/api/console/assistant/runs/websocket": {"get": {}},
                    "/api/console/widgets": {"post": {}}
                }
            }),
        )
        .unwrap();
    compiler.contribute_approved_controls(false).unwrap();
    let catalog = compiler.compile();

    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "GET",
                "/api/console/assistant/runs/websocket",
                "websocket-upgrade",
            ))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::ProtocolControl
    );
    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http(
                "POST",
                "/api/console/widgets",
            ))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::Unclassified
    );
}

#[test]
fn eil_f15_runtime_model_descriptor_routes_use_method_specific_frozen_bindings() {
    let templates = runtime_core::data_model_template_registry::DataModelTemplateCatalog::core();
    let mut document = serde_json::json!({ "paths": {} });
    crate::runtime_data_model_docs::append_template_runtime_openapi_paths(
        &mut document,
        &templates,
    );
    let registry =
        crate::routes::runtime_models::compile_runtime_model_interface_registry_for_test()
            .expect("runtime model interface registry must compile");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute_openapi_document("authentic-runtime-model-openapi", &document)
        .unwrap();
    compiler
        .absorb_registry("compiled-interface-registry", registry.as_ref())
        .unwrap();
    let catalog = compiler.compile_complete(registry.as_ref()).unwrap();

    let paths = document["paths"]
        .as_object()
        .expect("descriptor fixture must publish concrete paths");
    assert_eq!(paths.len(), 12);
    for (path, path_item) in paths {
        let operations = path_item
            .as_object()
            .expect("descriptor path item must be an object");
        for method in operations.keys() {
            let expected_binding_id = registry
                .bindings()
                .find_map(|binding| match binding.projection() {
                    ProtocolProjection::Http(route)
                        if route.method().eq_ignore_ascii_case(method)
                            && route.path()
                                == "/api/runtime/models/:model_code/*operation_path" =>
                    {
                        Some(binding.binding_id().as_str())
                    }
                    _ => None,
                })
                .expect("descriptor method must have a frozen runtime model binding");
            let row = catalog
                .row(&ExternalEndpointIdentity::http(method, path))
                .expect("descriptor route must be cataloged");
            assert_eq!(
                row.classification(),
                ExternalEndpointClassification::CanonicalBusinessInterface,
                "{method} {path} must be canonical"
            );
            assert_eq!(
                row.binding_id(),
                Some(expected_binding_id),
                "{method} {path} must use its method-specific frozen binding"
            );
        }
    }
}

#[test]
fn eil_f15_unknown_runtime_model_route_remains_unclassified() {
    let registry =
        crate::routes::runtime_models::compile_runtime_model_interface_registry_for_test()
            .expect("runtime model interface registry must compile");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::unclassified_http(
            "unknown-runtime-router",
            "POST",
            "/api/runtime/models/{model_code}/actions/not-a-descriptor",
        ))
        .unwrap();
    compiler
        .absorb_registry("compiled-interface-registry", registry.as_ref())
        .unwrap();

    assert!(matches!(
        compiler.compile_complete(registry.as_ref()),
        Err(ExternalEndpointCatalogError::UnclassifiedRows { identities })
            if identities.contains(&ExternalEndpointIdentity::http(
                "POST",
                "/api/runtime/models/{model_code}/actions/not-a-descriptor",
            ))
    ));
}

#[tokio::test]
async fn eil_f14_production_assembly_has_no_unclassified_endpoint() {
    let (state, _database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let _router = crate::app_with_state_and_config(
        Arc::clone(&state),
        &crate::_tests::support::test_config(),
    );
    let catalog = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.external_endpoint_catalog())
        .expect("production assembly must publish the external endpoint catalog");
    let total = catalog.rows().len();
    let business =
        catalog.classification_count(ExternalEndpointClassification::CanonicalBusinessInterface);
    let protocol = catalog.classification_count(ExternalEndpointClassification::ProtocolControl);
    let operational =
        catalog.classification_count(ExternalEndpointClassification::OperationalControl);
    let unclassified = catalog.classification_count(ExternalEndpointClassification::Unclassified);

    println!(
        "EIL_F14_ENDPOINT_COUNTS total={total} business={business} protocol={protocol} operational={operational} unclassified={unclassified}"
    );
    assert_eq!(unclassified, 0);
    assert_eq!(total, business + protocol + operational);
}

#[test]
fn workflow_descriptor_requires_a_registered_frozen_binding() {
    let registry = compiled_fixture_registry("/api/console/fixture");
    for (path, operation) in [
        (
            "/api/ex/report",
            serde_json::json!({"operationId":"published_workflow_operation:00000000-0000-0000-0000-000000000001"}),
        ),
        ("/api/ex/unknown", serde_json::json!({})),
    ] {
        let mut compiler = ExternalEndpointCatalogCompiler::default();
        compiler
            .contribute_openapi_document(
                "workflow-openapi",
                &serde_json::json!({
                    "paths": { (path): {"get": operation} }
                }),
            )
            .unwrap();
        compiler
            .absorb_registry("registry", registry.as_ref())
            .unwrap();
        assert!(matches!(
            compiler.compile_complete(registry.as_ref()),
            Err(ExternalEndpointCatalogError::UnclassifiedRows { .. })
        ));
    }
}
