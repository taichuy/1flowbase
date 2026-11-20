use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

use super::{console_get, ConsoleRouteAssembly};
use crate::external_route_assembly::ExternalRouteAssembly;

#[tokio::test]
async fn console_assembly_applies_private_cache_policy_without_changing_public_routes() {
    let console = ConsoleRouteAssembly::<()>::new().route(
        "/settings/roles/admin/frontstage-routes",
        console_get(
            || async { "role permissions" },
            access_control::ConsoleRouteOwnership::Authenticated,
        ),
    );
    assert_eq!(console.bindings().len(), 1);
    let app = ExternalRouteAssembly::new()
        .nest("/api/console", ExternalRouteAssembly::console(console))
        .route(
            "/health",
            crate::external_route_assembly::get(|| async { "ok" }),
        )
        .into_router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/admin/frontstage-routes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );

    let public = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(public.status(), StatusCode::OK);
    assert!(!public.headers().contains_key(header::CACHE_CONTROL));
}
