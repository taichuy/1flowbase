use std::collections::BTreeSet;

use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_app, test_app_with_database_url,
};
use access_control::ConsoleRouteOwnership;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serde_json::Value;
use tower::ServiceExt;
use utoipa::OpenApi;

#[test]
fn frontstage_route_assembly_marks_every_console_route_as_authenticated() {
    let assembly = crate::routes::frontstage::route_assembly();
    let routes = assembly
        .bindings()
        .iter()
        .map(|binding| (binding.route.method.as_str(), binding.route.path.as_str()))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        routes,
        BTreeSet::from([
            (
                "GET",
                "/api/console/frontstage/:workspace_id/component-capabilities",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/component-capabilities/:component_id",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/component-module-assets/:sha256",
            ),
            ("GET", "/api/console/frontstage/:workspace_id/data-capabilities"),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/interface-capabilities",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/interface-capabilities/:interface_id",
            ),
            ("GET", "/api/console/frontstage/:workspace_id/pages"),
            ("POST", "/api/console/frontstage/:workspace_id/pages"),
            ("POST", "/api/console/frontstage/:workspace_id/pages/groups",),
            (
                "PATCH",
                "/api/console/frontstage/:workspace_id/pages/:page_id",
            ),
            (
                "DELETE",
                "/api/console/frontstage/:workspace_id/pages/:page_id",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/move",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_reference",
            ),
            (
                "PATCH",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_reference",
            ),
            (
                "DELETE",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_reference",
            ),
            (
                "PUT",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/document",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/blocks",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/queries/dispatch",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/actions/dispatch",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/callable-interfaces/dispatch",
            ),
            (
                "POST",
                "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/callable-interfaces/write-grants",
            ),
            (
                "GET",
                "/api/console/frontstage/:workspace_id/pages/:page_id/block-codes/:code_ref",
            ),
            (
                "PUT",
                "/api/console/frontstage/:workspace_id/pages/:page_id/block-codes/:code_ref",
            ),
        ])
    );
    assert!(assembly
        .bindings()
        .iter()
        .all(|binding| { binding.ownership == ConsoleRouteOwnership::Authenticated }));
}

async fn current_workspace_id(app: &axum::Router, cookie: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();

    payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_group(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    title: Option<&str>,
    rank: &str,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/groups"),
        cookie,
        csrf,
        json!({
            "title": title,
            "rank": rank
        }),
    )
    .await
}

async fn create_page(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    title: Option<&str>,
    parent_id: Option<&str>,
    rank: &str,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages"),
        cookie,
        csrf,
        json!({
            "title": title,
            "parent_id": parent_id,
            "rank": rank
        }),
    )
    .await
}

async fn send_json(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
            .unwrap_or_else(|_| json!({}));
    (status, payload)
}

async fn delete_node(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/pages/{page_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn get_json(app: &axum::Router, path: &str, cookie: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
            .unwrap_or_else(|_| json!({}));
    (status, payload)
}

#[allow(clippy::too_many_arguments)]
async fn save_page_content(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    tab_id: &str,
    document_payload: Value,
) -> (StatusCode, Value) {
    send_json(
        app,
        "PUT",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/document"),
        cookie,
        csrf,
        json!({ "payload": document_payload }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_capability(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    tab_id: &str,
    kind: &str,
    capability_id_field: &str,
    capability_id: &str,
    params: Value,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/{kind}/dispatch"
        ),
        cookie,
        csrf,
        json!({ capability_id_field: capability_id, "params": params }),
    )
    .await
}

mod authoring;
mod content;
mod listing;
mod placement;
mod presentation;
