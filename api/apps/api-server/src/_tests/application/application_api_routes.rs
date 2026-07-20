use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_app, test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_application_with_type(
    app: &Router,
    cookie: &str,
    csrf: &str,
    name: &str,
    application_type: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": application_type,
                        "name": name,
                        "description": "application public api test",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn create_application(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
    create_application_with_type(app, cookie, csrf, name, "agent_flow").await
}

async fn create_extension_workflow(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "workflow",
                        "workflow_trigger_type": "extension",
                        "workflow_trigger_config": {
                            "subpath": "orders/create",
                            "http_method": "POST",
                            "access_policy": "user_api_key",
                            "response_mode": "sync"
                        },
                        "name": name,
                        "description": "extension workflow test",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_member_with_permissions(
    app: &Router,
    root_cookie: &str,
    root_csrf: &str,
    permissions: &[&str],
) -> (String, String) {
    let suffix = Uuid::now_v7().to_string().replace('-', "");
    let account = format!("app_api_member_{}", &suffix[..16]);
    let role_code = format!("app_api_role_{}", &suffix[16..32]);
    let member_id = create_member(app, root_cookie, root_csrf, &account, "temp-pass").await;
    create_role(app, root_cookie, root_csrf, &role_code).await;
    replace_role_permissions(app, root_cookie, root_csrf, &role_code, permissions).await;
    replace_member_roles(app, root_cookie, root_csrf, &member_id, &[&role_code]).await;
    login_and_capture_cookie(app, &account, "temp-pass").await
}

async fn seed_js_dependency_pack(database_url: &str, version: &str) -> Uuid {
    let pool = PgPool::connect(database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into plugin_installations (
            id, provider_code, plugin_id, plugin_version, contract_version, protocol,
            display_name, source_kind, trust_level, verification_status, desired_state,
            artifact_status, runtime_status, availability_status, package_path, installed_path,
            checksum, manifest_fingerprint, signature_status, signature_algorithm, signing_key_id,
            last_load_error, metadata_json, created_by
        ) values (
            $1, $2, $3, $4, '1flowbase.capability/v1', 'stdio_json',
            'Fixture JS Dependency Pack', 'uploaded', 'checksum_only', 'valid', 'active_requested',
            'ready', 'inactive', 'available', null, $5, null, null, 'unsigned', null, null,
            null, $6, $7
        )
        "#,
    )
    .bind(installation_id)
    .bind(format!("fixture_js_dependency_pack_{version}"))
    .bind(format!("fixture_js_dependency_pack@{version}"))
    .bind(version)
    .bind(format!("/tmp/plugins/fixture_js_dependency_pack/{version}"))
    .bind(json!({}))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into js_dependency_registry (
            id, installation_id, provider_code, plugin_id, plugin_version, alias, package,
            version, target, artifact_path, integrity, permission_network,
            permission_filesystem, permission_env
        ) values ($1, $2, $3, $4, $5, 'zod', 'zod', $6, 'backend_code', $7, $8,
            'outbound_only', 'deny', 'deny')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(format!("fixture_js_dependency_pack_{version}"))
    .bind(format!("fixture_js_dependency_pack@{version}"))
    .bind(version)
    .bind(version)
    .bind(format!("artifacts/zod-{version}.backend.mjs"))
    .bind(format!("sha256-zod-{version}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into plugin_assignments (
            id, installation_id, workspace_id, provider_code, assigned_by
        ) values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(workspace_id)
    .bind(format!("fixture_js_dependency_pack_{version}"))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();

    installation_id
}

#[tokio::test]
async fn application_api_key_routes_create_list_hide_token_filter_and_revoke() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_application(&app, &root_cookie, &root_csrf, "API Key App").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Server key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload = response_json(create).await;
    let key_id = create_payload["data"]["id"].as_str().unwrap().to_string();
    let token = create_payload["data"]["token"].as_str().unwrap();
    let token_prefix = create_payload["data"]["token_prefix"].as_str().unwrap();
    assert!(token.starts_with("sk-"));
    assert!(token_prefix.starts_with("sk-"));
    assert_eq!(token.len(), 56);
    assert_eq!(token_prefix.len(), 15);
    assert_eq!(token.matches('-').count(), 2);
    assert!(token.starts_with(token_prefix));

    let duplicate_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Server key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(duplicate_create.status(), StatusCode::CREATED);
    let duplicate_payload = response_json(duplicate_create).await;
    assert!(duplicate_payload["data"]["token"]
        .as_str()
        .unwrap()
        .starts_with("sk-"));
    assert_eq!(
        duplicate_payload["data"]["token"].as_str().unwrap().len(),
        56
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_payload = response_json(list).await;
    let listed_keys = list_payload["data"].as_array().unwrap();
    assert_eq!(listed_keys.len(), 2);
    assert!(listed_keys.iter().any(
        |key| key["token_prefix"].as_str() == Some(token_prefix) && key.get("token").is_none()
    ));
    assert!(listed_keys
        .iter()
        .all(|key| key["name"].as_str() == Some("Server key")));

    let (member_cookie, _) =
        create_member_with_permissions(&app, &root_cookie, &root_csrf, &["application.view.all"])
            .await;
    let foreign_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_list.status(), StatusCode::OK);
    let foreign_payload = response_json(foreign_list).await;
    assert_eq!(foreign_payload["data"].as_array().unwrap().len(), 0);

    let (foreign_editor_cookie, foreign_editor_csrf) = create_member_with_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        &["application.view.all", "application.edit.all"],
    )
    .await;
    let foreign_revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys/{key_id}"
                ))
                .header("cookie", &foreign_editor_cookie)
                .header("x-csrf-token", &foreign_editor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_revoke.status(), StatusCode::NOT_FOUND);

    let revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys/{key_id}"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);

    let list_after_revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_after_revoke.status(), StatusCode::OK);
    let list_after_revoke_payload = response_json(list_after_revoke).await;
    assert_eq!(
        list_after_revoke_payload["data"].as_array().unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ac_004_application_api_key_routes_fail_closed_for_workflows() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workflow_id =
        create_extension_workflow(&app, &cookie, &csrf, "Workflow without app keys").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/applications/{workflow_id}/api-keys"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Unsupported", "expires_at": null}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(create).await["code"],
        json!("application_api_key_application_type")
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{workflow_id}/api-keys"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(list).await["code"],
        json!("application_api_key_application_type")
    );

    let revoke = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{workflow_id}/api-keys/{}",
                    Uuid::now_v7()
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(revoke).await["code"],
        json!("application_api_key_application_type")
    );
}

#[tokio::test]
async fn application_api_mapping_routes_get_and_replace_nullable_model_target() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_application(&app, &cookie, &csrf, "Mapping App").await;

    let default_mapping = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(default_mapping.status(), StatusCode::OK);
    let default_payload = response_json(default_mapping).await;
    assert_eq!(
        default_payload["data"]["input"]["query_target"].as_str(),
        Some("node-start.query")
    );
    assert_eq!(
        default_payload["data"]["input"]["model_target"].as_str(),
        Some("node-start.model")
    );
    assert!(default_payload["data"]["operation_bindings"]["generate"].is_null());
    assert!(default_payload["data"]["operation_bindings"]["count_tokens"].is_null());

    let replace = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input": {
                            "query_target": "start.prompt",
                            "model_target": null,
                            "inputs_target": "node-start",
                            "history_target": null,
                            "attachments_target": "node-start.files"
                        },
                        "output": {
                            "answer_selector": "end.answer",
                            "usage_selector": null,
                            "files_selector": "end.files",
                            "error_selector": "end.error"
                        },
                        "operation_bindings": {
                            "generate": null,
                            "count_tokens": null,
                            "compact": {
                                "responses_compact": null,
                                "responses_compaction_v2": null
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::OK);
    let replace_payload = response_json(replace).await;
    assert_eq!(
        replace_payload["data"]["input"]["query_target"].as_str(),
        Some("start.prompt")
    );
    assert!(replace_payload["data"]["input"]["model_target"].is_null());
    assert_eq!(
        replace_payload["data"]["output"]["answer_selector"].as_str(),
        Some("end.answer")
    );
    assert!(replace_payload["data"]["operation_bindings"]["generate"].is_null());
}

#[tokio::test]
async fn ac_007_extension_registration_fields_are_immutable_after_creation() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_extension_workflow(&app, &cookie, &csrf, "Immutable Extension").await;

    let replace = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input": {
                            "query_target": "node-start.query",
                            "model_target": "node-start.model",
                            "inputs_target": "node-start",
                            "history_target": "node-start.history",
                            "attachments_target": "node-start.files"
                        },
                        "output": {
                            "answer_selector": null,
                            "usage_selector": null,
                            "files_selector": null,
                            "error_selector": null
                        },
                        "extension": {
                            "slug": "orders/changed",
                            "method": "PUT",
                            "access_policy": "public",
                            "response_mode": "async"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(replace.status(), StatusCode::CONFLICT);
    let conflict = response_json(replace).await;
    assert_eq!(
        conflict["code"],
        json!("workflow_extension_registration_immutable")
    );

    let current = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    let current = response_json(current).await;
    assert_eq!(current["data"]["extension"]["slug"], json!("orders/create"));
    assert_eq!(current["data"]["extension"]["method"], json!("POST"));
    assert_eq!(
        current["data"]["extension"]["access_policy"],
        json!("user_api_key")
    );
    assert_eq!(current["data"]["extension"]["response_mode"], json!("sync"));
}

#[tokio::test]
async fn workflow_schedule_trigger_routes_get_replace_and_validate_config() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workflow_id =
        create_application_with_type(&app, &cookie, &csrf, "Scheduled Workflow", "workflow").await;

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{workflow_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::OK);
    let missing_payload = response_json(missing).await;
    assert!(missing_payload["data"].is_null());

    let replace = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{workflow_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "cron": "0 9 * * *",
                        "timezone": "Asia/Shanghai",
                        "input_payload": {
                            "node-workflow-start": {
                                "customer_id": "C-42"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::OK);
    let replace_payload = response_json(replace).await;
    assert_eq!(
        replace_payload["data"]["application_id"].as_str(),
        Some(workflow_id.as_str())
    );
    assert_eq!(replace_payload["data"]["enabled"].as_bool(), Some(true));
    assert_eq!(replace_payload["data"]["cron"].as_str(), Some("0 9 * * *"));
    assert_eq!(
        replace_payload["data"]["timezone"].as_str(),
        Some("Asia/Shanghai")
    );
    assert_eq!(
        replace_payload["data"]["input_payload"]["node-workflow-start"]["customer_id"].as_str(),
        Some("C-42")
    );

    let loaded = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{workflow_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(loaded.status(), StatusCode::OK);
    let loaded_payload = response_json(loaded).await;
    assert_eq!(loaded_payload["data"]["cron"].as_str(), Some("0 9 * * *"));

    let invalid_cron = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{workflow_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "cron": "bad cron",
                        "timezone": "Asia/Shanghai",
                        "input_payload": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_cron.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn workflow_schedule_trigger_route_rejects_non_workflow_application() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let agent_flow_id = create_application(&app, &cookie, &csrf, "Agent Flow Schedule").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{agent_flow_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "cron": "0 9 * * *",
                        "timezone": "UTC",
                        "input_payload": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn application_api_publication_routes_publish_and_patch_api_enabled_state() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_application(&app, &cookie, &csrf, "Publication App").await;

    let mapping = json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": null,
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        },
        "operation_bindings": {
            "generate": null,
            "count_tokens": null,
            "compact": {
                "responses_compact": null,
                "responses_compaction_v2": null
            }
        }
    });

    let create_key = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Publication key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_key.status(), StatusCode::CREATED);

    let replace_mapping = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(mapping.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_mapping.status(), StatusCode::OK);

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": mapping.clone(),
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish.status(), StatusCode::CREATED);
    let publish_payload = response_json(publish).await;
    assert_eq!(
        publish_payload["data"]["application_id"].as_str(),
        Some(application_id.as_str())
    );
    assert_eq!(
        publish_payload["data"]["version_sequence"].as_i64(),
        Some(1)
    );
    assert_eq!(publish_payload["data"]["active"].as_bool(), Some(true));
    assert_eq!(publish_payload["data"]["api_enabled"].as_bool(), Some(true));
    assert_eq!(
        publish_payload["data"]["public_url"].as_str(),
        Some("/api/agent/v1/runs")
    );
    assert!(publish_payload["data"]["operation_bindings"]["generate"].is_null());

    let mut invalid_binding_mapping = mapping.clone();
    invalid_binding_mapping["operation_bindings"]["generate"] = json!({
        "target_node_id": "missing-llm-node"
    });
    let invalid_publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": invalid_binding_mapping,
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_publish.status(), StatusCode::BAD_REQUEST);

    let patch_status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-status"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "api_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_status.status(), StatusCode::OK);
    let patch_status_payload = response_json(patch_status).await;
    assert_eq!(
        patch_status_payload["data"]["api_enabled"].as_bool(),
        Some(false)
    );

    let active_publication = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publication"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(active_publication.status(), StatusCode::OK);
    let active_payload = response_json(active_publication).await;
    assert_eq!(active_payload["data"]["api_enabled"].as_bool(), Some(false));
    assert_eq!(
        active_payload["data"]["public_url"].as_str(),
        Some("/api/agent/v1/runs")
    );
    assert!(active_payload["data"]["mapping_snapshot"]["input"]["model_target"].is_null());

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{application_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_payload = response_json(detail).await;
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["status"].as_str(),
        Some("active")
    );
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["invoke_path_template"].as_str(),
        Some("/api/agent/v1/runs")
    );
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["api_capability_status"].as_str(),
        Some("disabled")
    );
}

#[tokio::test]
async fn application_api_publication_route_unpublish_reverts_to_draft_and_allows_republish() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_application(&app, &cookie, &csrf, "Unpublish App").await;

    let mapping = json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": null,
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        }
    });

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": mapping,
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish.status(), StatusCode::CREATED);

    let unpublish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publication"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unpublish.status(), StatusCode::NO_CONTENT);

    let active_publication = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publication"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(active_publication.status(), StatusCode::NOT_FOUND);

    let unpublish_draft = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publication"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unpublish_draft.status(), StatusCode::NOT_FOUND);

    let republish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": mapping,
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(republish.status(), StatusCode::CREATED);
    let republish_payload = response_json(republish).await;
    assert_eq!(republish_payload["data"]["active"].as_bool(), Some(true));
    assert_eq!(
        republish_payload["data"]["api_enabled"].as_bool(),
        Some(true)
    );
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_is_empty_without_selection_on_publish_response(
) {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_application(&app, &cookie, &csrf, "Publication Empty Dependency App").await;
    let mapping = json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": null,
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        }
    });

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": mapping,
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(publish.status(), StatusCode::CREATED);
    let payload = response_json(publish).await;
    assert_eq!(
        payload["data"]["dependency_snapshot"].as_array().unwrap(),
        &Vec::<Value>::new()
    );
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_is_returned_on_publish_response() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_application(&app, &cookie, &csrf, "Publication Dependency App").await;
    let zod_v3 = seed_js_dependency_pack(&database_url, "3.24.0").await;
    let zod_v4 = seed_js_dependency_pack(&database_url, "4.0.0").await;
    let mapping = json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": null,
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        }
    });

    for installation_id in [zod_v3, zod_v4] {
        let replace_selection = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/api/console/applications/{application_id}/js-dependencies"
                    ))
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "installation_id": installation_id.to_string(),
                            "alias": "zod",
                            "target": "backend_code"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replace_selection.status(), StatusCode::OK);

        let publish = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/console/applications/{application_id}/api-publications"
                    ))
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "mapping": mapping,
                            "api_enabled": true
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(publish.status(), StatusCode::CREATED);
        let payload = response_json(publish).await;
        let snapshot = payload["data"]["dependency_snapshot"].as_array().unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0]["alias"].as_str(), Some("zod"));
        assert_eq!(snapshot[0]["package"].as_str(), Some("zod"));
        assert_eq!(
            snapshot[0]["version"].as_str(),
            Some(if installation_id == zod_v3 {
                "3.24.0"
            } else {
                "4.0.0"
            })
        );
        assert_eq!(
            snapshot[0]["permissions"]["network"].as_str(),
            Some("outbound_only")
        );
    }
}

/// Root #1366 B2b: the console gets one server-owned binding projection rather
/// than reconstructing target capability, publication support, or editability.
#[tokio::test]
async fn application_operation_binding_projection_route_returns_exact_consumer_dto() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_application(&app, &root_cookie, &root_csrf, "Binding DTO App").await;

    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-operation-bindings"
                ))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_response.status(), StatusCode::OK);
    let root_payload = response_json(root_response).await;
    assert_eq!(root_payload["data"]["editable"], json!(true));
    assert!(root_payload["data"]["draft"]["operation_bindings"]["generate"].is_null());
    assert_eq!(
        root_payload["data"]["draft"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["operation"].as_str())
            .collect::<Vec<_>>(),
        vec![
            Some("generate"),
            Some("count_tokens"),
            Some("compact.responses_compact"),
            Some("compact.responses_compaction_v2"),
        ]
    );
    assert!(root_payload["data"]["published"].is_null());

    let (viewer_cookie, _) =
        create_member_with_permissions(&app, &root_cookie, &root_csrf, &["application.view.all"])
            .await;
    let viewer_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-operation-bindings"
                ))
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(viewer_response).await["data"]["editable"],
        json!(false)
    );
}
