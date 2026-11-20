use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    middleware::map_response,
    routing::get,
    Router,
};
use tower::ServiceExt;

use super::prevent_implicit_console_caching;

#[tokio::test]
async fn console_role_reads_are_not_stored_by_shared_or_browser_caches() {
    let app = Router::new()
        .route(
            "/settings/roles/admin/frontstage-routes",
            get(|| async { r#"{"data":{"checked_page_ids":["page"],"checked_tab_ids":["tab"]}}"# }),
        )
        .layer(map_response(prevent_implicit_console_caching));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/settings/roles/admin/frontstage-routes")
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
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        r#"{"data":{"checked_page_ids":["page"],"checked_tab_ids":["tab"]}}"#
    );
}

#[tokio::test]
async fn console_error_responses_are_not_implicitly_cached() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
    ] {
        let app = Router::new()
            .route("/roles", get(move || async move { (status, "denied") }))
            .layer(map_response(prevent_implicit_console_caching));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/roles")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            "denied"
        );
    }
}

#[tokio::test]
async fn explicit_private_revalidation_and_etag_are_preserved() {
    for status in [StatusCode::OK, StatusCode::NOT_MODIFIED] {
        let app = Router::new()
            .route(
                "/block-tree",
                get(move || async move {
                    (
                        status,
                        [
                            (header::CACHE_CONTROL, "private, no-cache"),
                            (header::ETAG, "\"revision-1\""),
                        ],
                    )
                }),
            )
            .layer(map_response(prevent_implicit_console_caching));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/block-tree")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-cache"
        );
        assert_eq!(response.headers()[header::ETAG], "\"revision-1\"");
    }
}
