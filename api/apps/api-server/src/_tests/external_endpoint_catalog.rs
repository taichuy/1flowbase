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
        ExternalEndpointCatalogCompiler, ExternalEndpointCatalogError,
        ExternalEndpointClassification, ExternalEndpointContribution, ExternalEndpointIdentity,
    },
    routes::console_route_assembly::migrated_core_console_route_assembly,
};

#[derive(Debug)]
struct FixtureInput;

impl interface_runtime::InterfaceContract for FixtureInput {
    const CONTRACT_ID: &'static str = "external-endpoint-fixture-input";
    const CONTRACT_VERSION: &'static str = "1";
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

fn compiled_fixture_registry() -> Arc<interface_runtime::CompiledInterfaceRegistry> {
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
        ProtocolProjection::http(
            RouteIdentity::new("GET", "/api/console/fixture/:fixture_id").unwrap(),
        ),
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
        .absorb_registry("compiled-interface-registry", &compiled_fixture_registry())
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
        .absorb_registry("compiled-interface-registry", &compiled_fixture_registry())
        .unwrap();

    assert!(matches!(
        compiler.contribute(ExternalEndpointContribution::protocol_control_http(
            "control-allowlist",
            "GET",
            "/api/console/fixture/:fixture_id",
        )),
        Err(ExternalEndpointCatalogError::ConflictingClassification { .. })
    ));
}

#[test]
fn eil_f01_operational_control_is_explicit_not_a_default() {
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute(ExternalEndpointContribution::operational_control_http(
            "root-router",
            "GET",
            "/health",
        ))
        .unwrap();
    let catalog = compiler.compile();

    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http("GET", "/health"))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::OperationalControl
    );
}
