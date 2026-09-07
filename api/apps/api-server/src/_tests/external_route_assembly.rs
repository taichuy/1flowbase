//! Root #1998 A1: fixtures exercise executable construction, not catalog-only rows.
use crate::{
    external_endpoint_catalog::{
        ExternalEndpointCatalogCompiler, ExternalEndpointCatalogError, ExternalEndpointIdentity,
    },
    external_route_assembly::{get, ExternalRouteAssembly},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn root_1998_nested_executable_route_and_catalog_share_the_same_identity() {
    let assembly = ExternalRouteAssembly::<()>::new().nest(
        "/fixture",
        ExternalRouteAssembly::new().route("/:id", get(|| async { "mounted" })),
    );
    let registry = super::compiled_fixture_registry("/fixture/:id");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler.absorb_registry("fixture", &registry).unwrap();
    compiler
        .contribute_mounted_routes(&assembly.contributions())
        .unwrap();
    let catalog = compiler.compile_complete(&registry).unwrap();
    assert!(catalog
        .row(&ExternalEndpointIdentity::http("GET", "/fixture/:id"))
        .unwrap()
        .sources()
        .contains("external-route-assembly"));
    let response = assembly
        .into_router()
        .oneshot(
            Request::builder()
                .uri("/fixture/one")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn root_1998_actual_extra_mount_cannot_hide_behind_a_complete_registry() {
    let registry = super::compiled_fixture_registry("/fixture");
    let assembly = ExternalRouteAssembly::<()>::new()
        .route("/fixture", get(|| async {}))
        .merge(ExternalRouteAssembly::new().route("/unregistered", get(|| async {})));
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler.absorb_registry("fixture", &registry).unwrap();
    compiler
        .contribute_mounted_routes(&assembly.contributions())
        .unwrap();
    assert!(
        matches!(compiler.compile_complete(&registry), Err(ExternalEndpointCatalogError::UnclassifiedRows { identities }) if identities == vec![ExternalEndpointIdentity::http("GET", "/unregistered")])
    );
}

#[test]
fn root_1998_duplicate_actual_mounts_are_rejected_by_axum_construction() {
    assert!(std::panic::catch_unwind(|| {
        ExternalRouteAssembly::<()>::new()
            .route("/duplicate", get(|| async {}))
            .merge(ExternalRouteAssembly::new().route("/duplicate", get(|| async {})))
    })
    .is_err());
}

#[test]
fn root_1998_missing_business_binding_fails_even_when_openapi_declares_the_route() {
    let registry = super::compiled_fixture_registry("/other");
    let assembly = ExternalRouteAssembly::<()>::new().route("/business", get(|| async {}));
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute_openapi_document(
            "fixture",
            &serde_json::json!({"paths":{"/business":{"get":{}}}}),
        )
        .unwrap();
    compiler.absorb_registry("fixture", &registry).unwrap();
    compiler
        .contribute_mounted_routes(&assembly.contributions())
        .unwrap();
    assert!(matches!(
        compiler.compile_complete(&registry),
        Err(ExternalEndpointCatalogError::UnclassifiedRows { .. })
    ));
}

#[test]
fn root_1998_any_business_mount_cannot_claim_a_get_control() {
    let registry = super::compiled_fixture_registry("/fixture");
    let assembly = ExternalRouteAssembly::<()>::new()
        .route("/health", crate::external_route_assembly::any(|| async {}));
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler.contribute_approved_controls(false).unwrap();
    compiler
        .contribute_mounted_routes(&assembly.contributions())
        .unwrap();
    assert!(
        matches!(compiler.compile_complete(&registry), Err(ExternalEndpointCatalogError::UnclassifiedRows { identities }) if identities.contains(&ExternalEndpointIdentity::http("ANY", "/health")))
    );
}

#[tokio::test]
async fn root_1998_registry_only_http_cannot_claim_an_unmounted_endpoint() {
    let registry = super::compiled_fixture_registry("/registry-only");
    for with_openapi in [false, true] {
        let assembly = ExternalRouteAssembly::<()>::new();
        let mut compiler = ExternalEndpointCatalogCompiler::default();
        if with_openapi {
            compiler
                .contribute_openapi_document(
                    "registry-only-openapi",
                    &serde_json::json!({"paths":{"/registry-only":{"get":{}}}}),
                )
                .unwrap();
        }
        compiler
            .absorb_registry("registry-only", &registry)
            .unwrap();
        compiler
            .contribute_mounted_routes(&assembly.contributions())
            .unwrap();
        assert!(matches!(compiler.compile_complete(&registry),
            Err(ExternalEndpointCatalogError::UnmountedHttpRows { identities })
                if identities == vec![ExternalEndpointIdentity::http("GET", "/registry-only")]
        ));
        let response = assembly
            .into_router()
            .oneshot(
                Request::builder()
                    .uri("/registry-only")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[test]
fn root_1998_complete_catalog_requires_actual_mount_evidence() {
    let registry = super::compiled_fixture_registry("/registry-only");
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .absorb_registry("registry-only", &registry)
        .unwrap();
    assert!(matches!(compiler.compile_complete(&registry),
        Err(ExternalEndpointCatalogError::UnmountedHttpRows { identities })
            if identities == vec![ExternalEndpointIdentity::http("GET", "/registry-only")]
    ));
}

#[tokio::test]
async fn root_1998_production_docs_disabled_has_no_phantom_openapi_inventory() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut config = crate::_tests::support::test_config();
    config.env = crate::config::ApiEnvironment::Production;
    let router = crate::app_with_state_and_config(std::sync::Arc::clone(&state), &config);
    let catalog = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .external_endpoint_catalog()
        .expect("production must publish the mounted catalog");
    for path in ["/openapi.json", "/docs", "/docs/*rest"] {
        assert!(catalog
            .row(&ExternalEndpointIdentity::http("GET", path))
            .is_none());
        assert!(catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "HEAD",
                path,
                "get-mirror"
            ))
            .is_none());
        assert!(catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "OPTIONS",
                path,
                "cors-preflight"
            ))
            .is_none());
    }
    for path in ["/openapi.json", "/docs", "/docs/swagger-ui.css"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    // Derived protocol rows remain valid when their real HTTP owner is mounted.
    assert!(catalog
        .row(&ExternalEndpointIdentity::http_variant(
            "HEAD",
            "/health",
            "get-mirror"
        ))
        .is_some());
    assert!(catalog
        .row(&ExternalEndpointIdentity::http_variant(
            "OPTIONS",
            "/health",
            "cors-preflight"
        ))
        .is_some());
    let health = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
}

#[test]
fn root_1998_actual_mcp_http_carrier_is_owned_by_its_existing_protocol_binding() {
    let registry = super::compiled_fixture_registry_with_binding(
        interface_runtime::ProtocolProjection::mcp("mcp.json-rpc"),
        crate::routes::mcp_protocol::MCP_INVOCATION_BINDING_ID,
    );
    let assembly =
        ExternalRouteAssembly::new().nest("/api", crate::routes::mcp_protocol::route_assembly());
    let mut compiler = ExternalEndpointCatalogCompiler::default();
    compiler
        .absorb_registry("carrier-fixture", &registry)
        .unwrap();
    compiler
        .contribute_mounted_routes(&assembly.contributions())
        .unwrap();
    let catalog = compiler.compile_complete(&registry).unwrap();
    let carrier = catalog
        .row(&ExternalEndpointIdentity::http(
            "POST",
            "/api/mcp/:instance_id",
        ))
        .unwrap();
    assert_eq!(carrier.classification(), crate::external_endpoint_catalog::ExternalEndpointClassification::CanonicalBusinessInterface);
    assert_eq!(
        carrier.binding_id(),
        Some(crate::routes::mcp_protocol::MCP_INVOCATION_BINDING_ID)
    );
    assert!(catalog
        .row(&ExternalEndpointIdentity::mcp("mcp.json-rpc"))
        .is_some());
    assert!(catalog
        .row(&ExternalEndpointIdentity::http("POST", "/mcp/:instance_id"))
        .is_none());
}

#[test]
fn root_1998_mcp_carrier_rejects_missing_or_non_mcp_binding() {
    for registry in [
        super::compiled_fixture_registry("/fixture"),
        super::compiled_fixture_registry_with_binding(
            interface_runtime::ProtocolProjection::http(
                interface_runtime::RouteIdentity::new("POST", "/api/mcp/:instance_id").unwrap(),
            ),
            crate::routes::mcp_protocol::MCP_INVOCATION_BINDING_ID,
        ),
    ] {
        let assembly = ExternalRouteAssembly::new()
            .nest("/api", crate::routes::mcp_protocol::route_assembly());
        let mut compiler = ExternalEndpointCatalogCompiler::default();
        compiler
            .absorb_registry("carrier-fixture", &registry)
            .unwrap();
        compiler
            .contribute_mounted_routes(&assembly.contributions())
            .unwrap();
        assert!(matches!(
            compiler.compile_complete(&registry),
            Err(ExternalEndpointCatalogError::UnknownBinding { .. }
                | ExternalEndpointCatalogError::InvalidCarrierBinding { .. })
        ));
    }
}

#[tokio::test]
async fn root_1998_production_mcp_carrier_preserves_jsonrpc_method_classification() {
    use crate::external_endpoint_catalog::ExternalEndpointClassification;
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let _router = crate::app_with_state_and_config(
        std::sync::Arc::clone(&state),
        &crate::_tests::support::test_config(),
    );
    let catalog = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .external_endpoint_catalog()
        .expect("full production construction must publish the complete carrier inventory");
    let binding = crate::routes::mcp_protocol::MCP_INVOCATION_BINDING_ID;
    let carrier = catalog
        .row(&ExternalEndpointIdentity::http(
            "POST",
            "/api/mcp/:instance_id",
        ))
        .unwrap();
    assert_eq!(carrier.binding_id(), Some(binding));
    assert_eq!(
        carrier.classification(),
        ExternalEndpointClassification::CanonicalBusinessInterface
    );
    assert_eq!(
        catalog
            .row(&ExternalEndpointIdentity::http_variant(
                "OPTIONS",
                "/api/mcp/:instance_id",
                "cors-preflight",
            ))
            .unwrap()
            .classification(),
        ExternalEndpointClassification::ProtocolControl
    );
    for method in ["tools/list", "tools/call"] {
        let row = catalog.row(&ExternalEndpointIdentity::mcp(method)).unwrap();
        assert_eq!(row.binding_id(), Some(binding));
        assert_eq!(
            row.classification(),
            ExternalEndpointClassification::CanonicalBusinessInterface
        );
    }
    for method in ["initialize", "notifications/initialized"] {
        let row = catalog.row(&ExternalEndpointIdentity::mcp(method)).unwrap();
        assert_eq!(row.binding_id(), None);
        assert_eq!(
            row.classification(),
            ExternalEndpointClassification::ProtocolControl
        );
    }
}
