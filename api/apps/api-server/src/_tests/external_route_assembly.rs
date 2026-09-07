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
