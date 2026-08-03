use super::*;
use crate::_tests::support::seed_workspace;

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
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_model(app: &axum::Router, cookie: &str, csrf: &str, code: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": code,
                        "title": code
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn current_scope_grant_id(database_url: &str, model_id: &str, scope_id: &str) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let id: uuid::Uuid = sqlx::query_scalar(
        r#"
        select id
        from scope_data_model_grants
        where data_model_id = $1
          and scope_kind = 'workspace'
          and scope_id = $2
        "#,
    )
    .bind(uuid::Uuid::parse_str(model_id).unwrap())
    .bind(uuid::Uuid::parse_str(scope_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    id.to_string()
}

async fn request_scope_grant_create(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    body: serde_json::Value,
) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn request_scope_grant_update(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    grant_id: &str,
) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants/{grant_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": true, "permission_profile": "owner" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn model_definition_scope_grant_routes_restrict_non_root_to_current_workspace() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let root_workspace_id = current_workspace_id(&app, &root_cookie).await;
    let other_workspace_id = seed_workspace(&database_url, "Scope Grant Foreign Workspace").await;

    let model_id = create_model(
        &app,
        &root_cookie,
        &root_csrf,
        "scope_grant_acl_route_orders",
    )
    .await;
    let create_denied_model_id = create_model(
        &app,
        &root_cookie,
        &root_csrf,
        "scope_grant_create_acl_orders",
    )
    .await;
    let current_grant_id =
        current_scope_grant_id(&database_url, &model_id, &root_workspace_id).await;
    create_role(&app, &root_cookie, &root_csrf, "scope_grant_manager").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "scope_grant_manager",
        &["settings_feature.access.system.data-models"],
    )
    .await;
    let manager_member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "scope-grant-manager",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &manager_member_id,
        &["scope_grant_manager"],
    )
    .await;
    let (manager_cookie, manager_csrf) =
        login_and_capture_cookie(&app, "scope-grant-manager", "temp-pass").await;

    for body in [
        json!({
            "scope_kind": "system",
            "scope_id": domain::SYSTEM_SCOPE_ID,
            "enabled": true,
            "permission_profile": "scope_all"
        }),
        json!({
            "scope_kind": "workspace",
            "scope_id": other_workspace_id,
            "enabled": true,
            "permission_profile": "scope_all"
        }),
    ] {
        assert_eq!(
            request_scope_grant_create(
                &app,
                &manager_cookie,
                &manager_csrf,
                &create_denied_model_id,
                body,
            )
            .await,
            StatusCode::FORBIDDEN
        );
    }

    assert_eq!(
        request_scope_grant_update(
            &app,
            &manager_cookie,
            &manager_csrf,
            &model_id,
            &current_grant_id,
        )
        .await,
        StatusCode::OK
    );
}
