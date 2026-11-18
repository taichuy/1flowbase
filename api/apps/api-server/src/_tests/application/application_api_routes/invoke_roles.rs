use super::*;
use crate::_tests::support::{
    create_member, create_role, replace_member_roles, replace_role_permissions,
};

async fn request(
    app: &Router,
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
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), response_json(response).await)
}

async fn publish_roles(
    app: &Router,
    cookie: &str,
    csrf: &str,
    id: &str,
    roles: Value,
) -> (StatusCode, Value) {
    let (status, body) = request(
        app,
        "GET",
        &format!("/api/console/applications/{id}/api-mapping"),
        cookie,
        csrf,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut mapping = body["data"].clone();
    mapping["extension"]["invoke_role_codes"] = roles;
    request(
        app,
        "POST",
        &format!("/api/console/applications/{id}/api-publications"),
        cookie,
        csrf,
        json!({"mapping":mapping,"api_enabled":true}),
    )
    .await
}

async fn setup_workflow(app: &Router, slug: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let (status, body) = request(
        app,
        "POST",
        "/api/console/applications",
        &cookie,
        &csrf,
        json!({"application_type":"workflow","workflow_trigger_type":"extension",
            "workflow_trigger_config":{"subpath":slug,"http_method":"POST","response_mode":"async"},
            "name":slug,"description":"Isolated workflow invocation permission fixture"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["data"]["id"].as_str().unwrap().to_owned();
    let (status, state) = request(
        app,
        "GET",
        &format!("/api/console/applications/{id}/orchestration"),
        &cookie,
        &csrf,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let (status, saved) = request(app, "PUT", &format!("/api/console/applications/{id}/orchestration/draft"),
        &cookie, &csrf, json!({"document":state["data"]["draft"]["document"], "change_kind":"logical", "summary":"Save workflow permission fixture"})).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, published) = publish_roles(app, &cookie, &csrf, &id, json!([])).await;
    assert_eq!(status, StatusCode::CREATED, "{published}");
    id
}

#[tokio::test]
async fn published_workflow_role_grants_allow_only_selected_workflows_without_application_access() {
    let app = test_app().await;
    let id = setup_workflow(&app, "invoke-role-allowed").await;
    setup_workflow(&app, "invoke-role-unrelated").await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "workflow-invoker",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "workflow_invoker").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "workflow_invoker", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member,
        &["workflow_invoker"],
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "workflow-invoker", "temp-pass").await;
    let invoke = "/api/ex/invoke-role-allowed";
    let (status, denied) = request(&app, "POST", invoke, &cookie, &csrf, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, publication) = publish_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &id,
        json!(["workflow_invoker"]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{publication}");
    let (status, run) = request(&app, "POST", invoke, &cookie, &csrf, json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{run}");
    assert!(
        run["run_id"]
            .as_str()
            .is_some_and(|id| Uuid::parse_str(id).is_ok()),
        "{run}"
    );
    for path in ["/api/ex/invoke-role-unrelated"] {
        let (status, denied) = request(&app, "POST", path, &cookie, &csrf, json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    }
    for path in [
        format!("/api/console/applications/{id}"),
        format!("/api/console/applications/{id}/api-mapping"),
        "/api/console/applications".into(),
    ] {
        let (status, denied) = request(&app, "GET", &path, &cookie, &csrf, Value::Null).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {denied}");
    }
    let (status, invalid) = publish_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &id,
        json!(["workflow_invoker", "workflow_invoker"]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
    let (status, run) = request(&app, "POST", invoke, &cookie, &csrf, json!({})).await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "rejected configuration must retain the active grant: {run}"
    );
    let (status, revoked) = publish_roles(&app, &root_cookie, &root_csrf, &id, json!([])).await;
    assert_eq!(status, StatusCode::CREATED, "{revoked}");
    let (status, denied) = request(&app, "POST", invoke, &cookie, &csrf, json!({})).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "revocation must apply to the existing session: {denied}"
    );
}
