use super::*;

#[test]
fn local_command_contract_accepts_only_command_and_keeps_content_private() {
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
}

#[test]
fn local_command_needs_no_deployment_configuration() {
    assert!(crate::config::ApiConfig::from_env_map(&[
        ("API_DATABASE_URL", "postgres://unused"),
        // Removed settings must not remain a hidden activation gate or parser error.
        ("API_LOCAL_COMMAND_TIMEOUT_MS", "not-a-number"),
        ("API_LOCAL_COMMAND_MAX_OUTPUT_BYTES", "not-a-number"),
    ])
    .is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn local_command_http_grant_revoke_full_custom_and_audit() {
    use crate::_tests::support::{
        create_member, create_role, login_and_capture_cookie, replace_member_roles,
        test_api_state_with_database_url, test_config,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;
    let (state, database_url) = test_api_state_with_database_url().await;
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
            .body(Body::from(json!({"command":command}).to_string()))
            .unwrap()
    };
    for (auth, token, status) in [
        (None, None, StatusCode::UNAUTHORIZED),
        (Some(cookie.as_str()), None, StatusCode::UNAUTHORIZED),
        (
            Some(cookie.as_str()),
            Some("wrong-token"),
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(auth, token, &command))
                .await
                .unwrap()
                .status(),
            status
        );
        assert!(!marker.exists());
    }
    assert_eq!(
        app.clone()
            .oneshot(request(Some(&cookie), Some(&csrf), " "))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(request(Some(&cookie), Some(&csrf), "printf root"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let member_id = create_member(&app, &cookie, &csrf, "command-member", "temp-pass").await;
    create_role(&app, &cookie, &csrf, "command_member_role").await;
    replace_member_roles(&app, &cookie, &csrf, &member_id, &["command_member_role"]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "command-member", "temp-pass").await;
    let denied = app
        .clone()
        .oneshot(request(Some(&member_cookie), Some(&member_csrf), &command))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert!(!marker.exists());

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/console-policy-catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    let group = catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["group_id"] == "other.local-commands")
        .expect("selectable other permission group");
    let operation = group["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["route"]["path"] == "/api/console/local-commands/execute")
        .expect("selectable command operation");
    let operation_id = operation["operation_id"].as_str().unwrap();
    assert_eq!(
        operation["full_profile"],
        json!({"kind":"simple","enabled":true})
    );
    let permission = json!({"kind":"simple","operation_id":operation_id,"enabled":true});
    for (enabled, strategy, operations, allowed) in [
        (true, "custom", vec![permission.clone()], true),
        (false, "custom", vec![permission.clone()], false),
        (true, "full", vec![], true),
        (true, "custom", vec![], false),
    ] {
        let update = app.clone().oneshot(Request::builder().method("PUT").uri("/api/console/settings/roles/command_member_role/console-policy")
            .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
            .body(Body::from(json!({"groups":[{"kind":"other","group_id":"other.local-commands","enabled":enabled,"strategy":strategy,"operations":operations}]}).to_string())).unwrap()).await.unwrap();
        let status = update.status();
        let bytes = to_bytes(update.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let response = app
            .clone()
            .oneshot(request(Some(&member_cookie), Some(&member_csrf), &command))
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        if allowed {
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["data"]["stdout"], "output");
            assert_eq!(body["data"]["stderr"], "problem");
            assert_eq!(body["data"]["exit_code"], 9);
            assert_eq!(body["data"]["cleanup_complete"], true);
            assert_eq!(body["data"]["truncated"], false);
            assert_eq!(std::fs::read(&marker).unwrap(), b"done");
            std::fs::remove_file(&marker).unwrap();
        } else {
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert!(!marker.exists());
        }
        if !enabled {
            let saved = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/console/settings/roles/command_member_role/console-policy")
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let saved: Value =
                serde_json::from_slice(&to_bytes(saved.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            let group = saved["data"]["groups"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["group_id"] == "other.local-commands")
                .unwrap();
            assert_eq!(
                group["operations"].as_array().unwrap().len(),
                1,
                "disabling must retain custom selections"
            );
        }
    }
    let store = storage_durable_postgres::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    let audits: Vec<Value> =
        sqlx::query_scalar("SELECT payload FROM audit_logs WHERE target_type = 'local_command'")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(audits.len(), 6); // root + two explicitly allowed member calls, each started/finished.
    assert!(!serde_json::to_string(&audits).unwrap().contains(&command));
    assert!(audits
        .iter()
        .all(|p| p.get("stdout").is_none() && p.get("stderr").is_none()));
}

#[cfg(unix)]
#[tokio::test]
async fn local_command_http_response_stream_preserves_all_output() {
    use axum::body::to_bytes;
    let result = tokio::task::spawn_blocking(|| {
        crate::local_command::process::execute("head -c 2097152 /dev/zero", &Default::default())
    })
    .await
    .unwrap()
    .unwrap();
    let response = super::response::response(result).unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["data"]["stdout"].as_str().unwrap().len(), 2097152);
    assert_eq!(body["data"]["truncated"], false);
}

#[test]
fn local_command_route_openapi_and_operation_agree() {
    use utoipa::OpenApi;
    let document = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let operation = &document["paths"]["/api/console/local-commands/execute"]["post"];
    assert!(operation["requestBody"].is_object());
    assert_eq!(operation["operationId"], "execute_local_command");
    assert_eq!(route_assembly().bindings().len(), 1);
}
