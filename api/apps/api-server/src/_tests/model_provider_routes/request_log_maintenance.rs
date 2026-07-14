use super::*;
use crate::_tests::support::{
    create_member, create_role, replace_member_roles, replace_role_permissions, seed_workspace,
    test_app_with_database_url,
};
use uuid::Uuid;

#[tokio::test]
async fn request_log_maintenance_routes_enforce_csrf_permission_and_openapi_contract() {
    // AC-008: both commands use the authenticated console security boundary.
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/model-providers/request-logs")
                .header("cookie", &root_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "attempt_ids": [Uuid::now_v7()] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "provider-log-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "provider_log_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "provider_log_viewer",
        &["state_model.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["provider_log_viewer"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "provider-log-viewer", "temp-pass").await;
    let forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", member_cookie)
                .header("x-csrf-token", member_csrf)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let selected_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/model-providers/request-logs")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "attempt_ids": [Uuid::now_v7()] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selected_delete.status(), StatusCode::OK);
    let selected_payload: Value = serde_json::from_slice(
        &to_bytes(selected_delete.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(selected_payload["data"]["deleted_count"], 0);

    let clear = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear.status(), StatusCode::OK);
    let clear_payload: Value =
        serde_json::from_slice(&to_bytes(clear.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(clear_payload["data"]["deleted_count"], 0);
    assert_eq!(clear_payload["data"]["has_more"], false);
    assert!(clear_payload["data"]["continuation_token"].is_string());
    assert!(clear_payload["data"]
        .get("snapshot_created_before")
        .is_none());

    let openapi = openapi_payload().await;
    assert!(
        openapi["paths"]["/api/console/settings/model-providers/request-logs"]["delete"]
            .is_object()
    );
    assert!(
        openapi["paths"]["/api/console/settings/model-providers/request-logs/clear"]["post"]
            .is_object()
    );
}

#[tokio::test]
async fn clear_request_log_continuation_is_opaque_tamper_proof_and_workspace_bound() {
    // AC-005/AC-007: callers can only continue the backend-frozen workspace snapshot.
    let (app, database_url) = test_app_with_database_url().await;
    let other_workspace_id = seed_workspace(&database_url, "Clear token target").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let plaintext_snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "snapshot_created_before": "2099-01-01T00:00:00Z" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(plaintext_snapshot.status(), StatusCode::BAD_REQUEST);

    let initial = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_payload: Value =
        serde_json::from_slice(&to_bytes(initial.into_body(), usize::MAX).await.unwrap()).unwrap();
    let continuation_token = initial_payload["data"]["continuation_token"]
        .as_str()
        .expect("opaque continuation token")
        .to_string();
    assert!(initial_payload["data"]
        .get("snapshot_created_before")
        .is_none());

    let retry = post_clear(
        &app,
        &cookie,
        &csrf,
        json!({
            "continuation_token": continuation_token
        }),
    )
    .await;
    assert_eq!(retry.status(), StatusCode::OK);
    let retry_payload: Value =
        serde_json::from_slice(&to_bytes(retry.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        retry_payload["data"]["continuation_token"],
        initial_payload["data"]["continuation_token"]
    );

    let mut tampered = continuation_token.clone();
    tampered.push('x');
    assert_eq!(
        post_clear(
            &app,
            &cookie,
            &csrf,
            json!({ "continuation_token": tampered })
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post_clear(
            &app,
            &cookie,
            &csrf,
            json!({ "continuation_token": "not-a-token" })
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let wrong_version = continuation_token.replacen("v1.", "v2.", 1);
    assert_eq!(
        post_clear(
            &app,
            &cookie,
            &csrf,
            json!({ "continuation_token": wrong_version })
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let switched = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "workspace_id": other_workspace_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switched.status(), StatusCode::OK);
    let switched_payload: Value =
        serde_json::from_slice(&to_bytes(switched.into_body(), usize::MAX).await.unwrap()).unwrap();
    let switched_csrf = switched_payload["data"]["csrf_token"]
        .as_str()
        .expect("rotated csrf token");
    assert_eq!(
        post_clear(
            &app,
            &cookie,
            switched_csrf,
            json!({ "continuation_token": continuation_token })
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn clear_request_log_continuation_keeps_late_arrivals_outside_the_frozen_snapshot() {
    // AC-005/AC-007: the signed continuation keeps T0 even when a later request runs at T1.
    let (app, database_url) = test_app_with_database_url().await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("select id from workspaces limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let flow_run_id = Uuid::now_v7();
    let attempt_ids = (0..501).map(|_| Uuid::now_v7()).collect::<Vec<_>>();
    sqlx::query(
        r#"
        insert into model_provider_request_logs (
            id, scope_id, attempt_id, flow_run_id, application_name, attempt_index,
            provider_code, protocol, upstream_model_id, status,
            failed_after_first_token, started_at, created_at
        )
        select attempt_id, $1, attempt_id, $2, 'Clear fixture', 1,
               'fixture', 'openai_chat', 'fixture-model', 'succeeded',
               false, statement_timestamp() - interval '1 second',
               statement_timestamp() - interval '1 second'
        from unnest($3::uuid[]) as attempt_id
        "#,
    )
    .bind(workspace_id)
    .bind(flow_run_id)
    .bind(&attempt_ids)
    .execute(&pool)
    .await
    .unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let first = post_clear(&app, &cookie, &csrf, json!({})).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload: Value =
        serde_json::from_slice(&to_bytes(first.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(first_payload["data"]["deleted_count"], 500);
    assert_eq!(first_payload["data"]["has_more"], true);
    let continuation_token = first_payload["data"]["continuation_token"]
        .as_str()
        .expect("continuation token")
        .to_string();

    let late_attempt_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into model_provider_request_logs (
            id, scope_id, attempt_id, flow_run_id, application_name, attempt_index,
            provider_code, protocol, upstream_model_id, status,
            failed_after_first_token, started_at, created_at
        ) values ($1, $2, $1, $3, 'Late arrival', 1, 'fixture', 'openai_chat',
                  'fixture-model', 'succeeded', false,
                  statement_timestamp() - interval '1 day', statement_timestamp())
        "#,
    )
    .bind(late_attempt_id)
    .bind(workspace_id)
    .bind(flow_run_id)
    .execute(&pool)
    .await
    .unwrap();

    let second = post_clear(
        &app,
        &cookie,
        &csrf,
        json!({ "continuation_token": continuation_token }),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_payload: Value =
        serde_json::from_slice(&to_bytes(second.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(second_payload["data"]["deleted_count"], 1);
    assert_eq!(second_payload["data"]["has_more"], false);
    assert_eq!(
        second_payload["data"]["continuation_token"],
        first_payload["data"]["continuation_token"]
    );
    let remaining_attempt_ids = sqlx::query_scalar::<_, Uuid>(
        "select attempt_id from model_provider_request_logs where scope_id = $1",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(remaining_attempt_ids, vec![late_attempt_id]);
}

async fn post_clear(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
