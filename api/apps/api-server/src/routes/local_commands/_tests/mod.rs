use super::*;

#[test]
fn local_command_contract_accepts_only_a_command_and_keeps_content_private() {
    let body: LocalCommandBody =
        serde_json::from_value(serde_json::json!({"command":"printf secret"})).unwrap();
    assert_eq!(
        body.project_for_managed_hook().unwrap(),
        serde_json::json!({"command_bytes":13})
    );
    assert!(serde_json::from_value::<LocalCommandBody>(
        serde_json::json!({"command":"pwd", "program":"sh"})
    )
    .is_err());
    assert!(serde_json::from_value::<LocalCommandBody>(serde_json::json!({"command":42})).is_err());
    assert!(serde_json::from_value::<LocalCommandBody>(serde_json::json!({})).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn local_command_http_auth_csrf_root_and_execution_contract() {
    use crate::_tests::support::{
        create_member, create_role, login_and_capture_cookie, replace_member_roles,
        test_api_state_with_database_url, test_config,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (mut state, database_url) = test_api_state_with_database_url().await;
    Arc::get_mut(&mut state).unwrap().local_command_policy =
        Some(control_plane::local_command::LocalCommandPolicy {
            timeout: std::time::Duration::from_secs(3),
            max_output_bytes: 1024,
        });
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let marker = std::env::temp_dir().join(format!("local-command-auth-{}", uuid::Uuid::now_v7()));
    let command = format!(
        "printf done > '{}'; printf output; printf problem >&2; exit 9",
        marker.display()
    );
    let request = |cookie: Option<&str>, csrf: Option<&str>, command: &str| {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/console/local-commands/execute")
            .header("content-type", "application/json");
        if let Some(value) = cookie {
            builder = builder.header("cookie", value);
        }
        if let Some(value) = csrf {
            builder = builder.header("x-csrf-token", value);
        }
        builder
            .body(Body::from(
                serde_json::json!({"command":command}).to_string(),
            ))
            .unwrap()
    };
    let response = app
        .clone()
        .oneshot(request(None, None, &command))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!marker.exists());
    let response = app
        .clone()
        .oneshot(request(Some(&cookie), None, &command))
        .await
        .unwrap();
    // Existing CSRF contract: absent header is 401; mismatched token is 403.
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!marker.exists());
    let response = app
        .clone()
        .oneshot(request(Some(&cookie), Some("wrong-token"), &command))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!marker.exists());
    let member_id = create_member(&app, &cookie, &csrf, "command-member", "temp-pass").await;
    create_role(&app, &cookie, &csrf, "command_member_role").await;
    replace_member_roles(&app, &cookie, &csrf, &member_id, &["command_member_role"]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "command-member", "temp-pass").await;
    let response = app
        .clone()
        .oneshot(request(Some(&member_cookie), Some(&member_csrf), &command))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let denied: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(denied["code"], "local_command_root_required");
    assert!(!marker.exists());
    let response = app
        .clone()
        .oneshot(request(Some(&cookie), Some(&csrf), " "))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!marker.exists());
    let response = app
        .clone()
        .oneshot(request(Some(&cookie), Some(&csrf), &command))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["data"]["stdout"], "output");
    assert_eq!(body["data"]["stderr"], "problem");
    assert_eq!(body["data"]["exit_code"], 9);
    assert_eq!(body["data"]["cleanup_complete"], true);
    assert_eq!(std::fs::read(&marker).unwrap(), b"done");
    std::fs::remove_file(marker).unwrap();
    let store = storage_durable_postgres::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    let audits: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM audit_logs WHERE target_type = 'local_command' ORDER BY created_at",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(audits.len(), 2);
    assert!(!serde_json::to_string(&audits).unwrap().contains(&command));
    assert!(audits
        .iter()
        .all(|payload| payload.get("stdout").is_none() && payload.get("stderr").is_none()));
}

#[tokio::test]
async fn local_command_http_disabled_has_no_side_effect() {
    use crate::_tests::support::{
        login_and_capture_cookie, test_api_state_with_database_url, test_config,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let marker =
        std::env::temp_dir().join(format!("local-command-disabled-{}", uuid::Uuid::now_v7()));
    let request = Request::builder()
        .method("POST")
        .uri("/api/console/local-commands/execute")
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"command":format!("echo unexpected > '{}'", marker.display())})
                .to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert!(!marker.exists());
}

#[test]
fn local_command_route_openapi_and_operation_agree() {
    use utoipa::OpenApi;
    let document = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let operation = &document["paths"]["/api/console/local-commands/execute"]["post"];
    assert!(operation["requestBody"].is_object());
    assert_eq!(operation["operationId"], "execute_local_command");
    let bindings = route_assembly().bindings().to_vec();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].route.method, "POST");
    assert_eq!(
        bindings[0].ownership,
        access_control::ConsoleRouteOwnership::ConsoleOperation("local_commands.execute".into())
    );
}
