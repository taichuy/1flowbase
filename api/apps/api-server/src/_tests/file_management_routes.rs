use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_app, test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const FILES_FEATURE_PERMISSION: &str = "settings_feature.access.system.files";

fn build_file_upload_body(
    boundary: &str,
    file_table_id: &str,
    file_name: &str,
    content_type: &str,
    bytes: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file_table_id\"\r\n\r\n{file_table_id}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
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
    response_json(response).await["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn model_id_for_file_table(database_url: &str, file_table_id: &str) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let model_id: Uuid =
        sqlx::query_scalar("select model_definition_id from file_tables where id = $1")
            .bind(Uuid::parse_str(file_table_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    model_id.to_string()
}

async fn revoke_model_grant(database_url: &str, model_id: &str, workspace_id: &str) {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    sqlx::query(
        "delete from scope_data_model_grants where scope_kind = 'workspace' and scope_id = $1 and data_model_id = $2",
    )
    .bind(Uuid::parse_str(workspace_id).unwrap())
    .bind(Uuid::parse_str(model_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
}

async fn register_files_feature_permission(database_url: &str) {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;
    store
        .upsert_permission_catalog(&[domain::PermissionDefinition {
            code: FILES_FEATURE_PERMISSION.to_string(),
            resource: "settings_feature".to_string(),
            action: "access".to_string(),
            scope: "system.files".to_string(),
            name: "settings_feature:access:system.files".to_string(),
        }])
        .await
        .expect("files feature permission should be seeded");
}

// AC-003/AC-004: system.files alone may list the current workspace's file tables,
// while system file-storage administration remains a root-only domain boundary.
#[tokio::test]
async fn settings_feature_files_route_keeps_storage_root_boundary() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_files_feature_permission(&database_url).await;
    create_role(&app, &root_cookie, &root_csrf, "files_feature_only").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "files_feature_only",
        &[FILES_FEATURE_PERMISSION],
    )
    .await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "files-feature-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["files_feature_only"],
    )
    .await;
    let (actor_cookie, actor_csrf) =
        login_and_capture_cookie(&app, "files-feature-actor", "temp-pass").await;

    let create_table = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "feature_owned_assets",
                        "title": "Feature owned assets"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table.status(), StatusCode::CREATED);
    let created_table = response_json(create_table).await;
    let current_table_id = created_table["data"]["id"].as_str().unwrap().to_string();
    let current_workspace = current_workspace_id(&app, &actor_cookie).await;
    assert_eq!(created_table["data"]["scope_kind"], "workspace");
    assert_eq!(created_table["data"]["scope_id"], current_workspace);

    let outside_workspace = seed_workspace(&database_url, "Outside files").await;
    let outside_table_id = Uuid::now_v7();
    let store = storage_durable::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    sqlx::query(
        r#"
        insert into file_tables (
            id, code, title, scope_kind, scope_id, model_definition_id,
            bound_storage_id, is_builtin, is_default, status, created_by, updated_by
        )
        select
            $1, $2, 'Outside assets', 'workspace', $3, model_definition_id,
            bound_storage_id, false, false, status, created_by, updated_by
        from file_tables
        where id = $4
        "#,
    )
    .bind(outside_table_id)
    .bind(format!("outside_assets_{}", outside_table_id.simple()))
    .bind(outside_workspace)
    .bind(Uuid::parse_str(&current_table_id).unwrap())
    .execute(store.pool())
    .await
    .unwrap();

    let tables = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/files/tables")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tables.status(), StatusCode::OK);
    let tables = response_json(tables).await;
    assert!(tables["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|table| table["id"] == current_table_id));
    assert!(!tables["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|table| table["id"] == outside_table_id.to_string()));

    let storages = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/files/storages")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages.status(), StatusCode::FORBIDDEN);

    let storage_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "workspace_forbidden",
                        "title": "Workspace forbidden",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": { "root_path": "/tmp/workspace-forbidden" },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storage_write.status(), StatusCode::FORBIDDEN);

    let binding_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/files/tables/{current_table_id}/binding"
                ))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "bound_storage_id": Uuid::now_v7() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(binding_write.status(), StatusCode::FORBIDDEN);

    let delete_table = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/files/tables/{current_table_id}"
                ))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_table.status(), StatusCode::FORBIDDEN);
}

// AC-002/AC-003/AC-011: legacy business actions do not authorize system.files,
// unregistered Settings routes fail closed, and the old HTTP contract is removed.
#[tokio::test]
async fn settings_feature_files_route_rejects_legacy_actions_and_removes_old_http_routes() {
    let (app, _) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "legacy_files_actions").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_files_actions",
        &[
            "file_storage.view.all",
            "file_storage.manage.all",
            "file_table.view.all",
            "file_table.view.own",
            "file_table.create.all",
            "file_table.delete.all",
            "file_table.delete.own",
            "file_table.bind.all",
        ],
    )
    .await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-files-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["legacy_files_actions"],
    )
    .await;
    let (actor_cookie, _) = login_and_capture_cookie(&app, "legacy-files-actor", "temp-pass").await;

    let action_only = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/files/tables")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action_only.status(), StatusCode::FORBIDDEN);

    let unregistered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/files/unregistered")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unregistered.status(), StatusCode::FORBIDDEN);

    for legacy_path in ["/api/console/file-storages", "/api/console/file-tables"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(legacy_path)
                    .header("cookie", &root_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{legacy_path}");
    }
}

#[tokio::test]
async fn file_management_routes_create_workspace_table_upload_and_read_by_storage_snapshot() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "file-admin", "change-me").await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["admin"]).await;
    let (admin_cookie, admin_csrf) =
        login_and_capture_cookie(&app, "file-admin", "change-me").await;

    let storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_response.status(), StatusCode::OK);
    let storages_payload = response_json(storages_response).await;
    let default_storage_id = storages_payload["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "project_assets",
                        "title": "Project Assets"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        table_payload["data"]["bound_storage_id"].as_str(),
        Some(default_storage_id.as_str())
    );
    assert_eq!(
        table_payload["data"]["bound_storage_title"].as_str(),
        storages_payload["data"][0]["title"].as_str()
    );

    let backup_root =
        std::env::temp_dir().join(format!("file-management-routes-{}", Uuid::now_v7()));
    let create_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "backup_local",
                        "title": "Backup Local",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": {
                            "root_path": backup_root.display().to_string(),
                            "public_base_url": null
                        },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_storage_response.status(), StatusCode::CREATED);
    let create_storage_payload = response_json(create_storage_response).await;
    let backup_storage_id = create_storage_payload["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let boundary = "----1flowbase-file-upload";
    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/files/upload")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(build_file_upload_body(
                    boundary,
                    &file_table_id,
                    "demo.txt",
                    "text/plain",
                    b"hello file-management",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let (upload_parts, upload_body) = upload_response.into_parts();
    let upload_body = to_bytes(upload_body, usize::MAX).await.unwrap();
    if upload_parts.status != StatusCode::CREATED {
        panic!(
            "upload failed: status={}, body={}",
            upload_parts.status,
            String::from_utf8_lossy(&upload_body)
        );
    }
    let upload_payload: Value = serde_json::from_slice(&upload_body).unwrap();
    assert_eq!(
        upload_payload["data"]["storage_id"].as_str(),
        Some(default_storage_id.as_str())
    );
    let record_id = upload_payload["data"]["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let bind_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}/binding"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "bound_storage_id": backup_storage_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bind_response.status(), StatusCode::OK);

    let second_upload_boundary = "----1flowbase-file-upload-after-bind";
    let second_upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/files/upload")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={second_upload_boundary}"),
                )
                .body(Body::from(build_file_upload_body(
                    second_upload_boundary,
                    &file_table_id,
                    "demo-after-bind.txt",
                    "text/plain",
                    b"hello backup storage",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_upload_response.status(), StatusCode::CREATED);
    let second_upload_payload = response_json(second_upload_response).await;
    assert_eq!(
        second_upload_payload["data"]["storage_id"].as_str(),
        Some(backup_storage_id.as_str())
    );

    let content_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/files/{file_table_id}/records/{record_id}/content"
                ))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    assert_eq!(
        content_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain")
    );
    assert_eq!(
        to_bytes(content_response.into_body(), usize::MAX)
            .await
            .unwrap(),
        &b"hello file-management"[..]
    );

    let _ = std::fs::remove_dir_all(backup_root);
}

#[tokio::test]
async fn file_routes_reject_upload_and_read_without_persisted_scope_grant() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;

    let storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_response.status(), StatusCode::OK);

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "blocked_assets",
                        "title": "Blocked Assets"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();
    let file_model_id = model_id_for_file_table(&database_url, &file_table_id).await;

    let allowed_boundary = "----1flowbase-file-upload-allowed";
    let allowed_upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/files/upload")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={allowed_boundary}"),
                )
                .body(Body::from(build_file_upload_body(
                    allowed_boundary,
                    &file_table_id,
                    "allowed.txt",
                    "text/plain",
                    b"allowed",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_upload.status(), StatusCode::CREATED);
    let upload_payload = response_json(allowed_upload).await;
    let record_id = upload_payload["data"]["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    revoke_model_grant(&database_url, &file_model_id, &workspace_id).await;
    let blocked_upload_boundary = "----1flowbase-file-upload-blocked";
    let blocked_upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/files/upload")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={blocked_upload_boundary}"),
                )
                .body(Body::from(build_file_upload_body(
                    blocked_upload_boundary,
                    &file_table_id,
                    "blocked.txt",
                    "text/plain",
                    b"blocked",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_upload.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(blocked_upload).await["code"],
        json!("data_model_scope_not_granted")
    );

    let blocked_read = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/files/{file_table_id}/records/{record_id}/content"
                ))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_read.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(blocked_read).await["code"],
        json!("data_model_scope_not_granted")
    );
}

#[tokio::test]
async fn file_upload_requires_workspace_session_context() {
    let app = test_app().await;
    let boundary = "----1flowbase-file-upload-no-session";

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(build_file_upload_body(
                    boundary,
                    &Uuid::now_v7().to_string(),
                    "demo.txt",
                    "text/plain",
                    b"hello",
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn file_management_settings_routes_enforce_root_only_storage_and_binding_rules() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "file-admin", "change-me").await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["admin"]).await;
    let (admin_cookie, admin_csrf) =
        login_and_capture_cookie(&app, "file-admin", "change-me").await;

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "workspace_docs",
                        "title": "Workspace Docs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();

    let root_storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_storages_response.status(), StatusCode::OK);
    let root_storages_payload = response_json(root_storages_response).await;
    let default_storage_title = root_storages_payload["data"][0]["title"]
        .as_str()
        .unwrap()
        .to_string();

    let list_tables_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tables_response.status(), StatusCode::OK);
    let list_tables_payload = response_json(list_tables_response).await;
    assert_eq!(
        list_tables_payload["data"][0]["bound_storage_title"].as_str(),
        Some(default_storage_title.as_str())
    );

    let list_storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_storages_response.status(), StatusCode::FORBIDDEN);

    let create_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "admin_local",
                        "title": "Admin Local",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": {
                            "root_path": std::env::temp_dir().join(format!("file-management-admin-{}", Uuid::now_v7())).display().to_string(),
                            "public_base_url": null
                        },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_storage_response.status(), StatusCode::FORBIDDEN);

    let bind_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}/binding"
                ))
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "bound_storage_id": Uuid::now_v7() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bind_response.status(), StatusCode::FORBIDDEN);

    let update_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/files/storages/00000000-0000-0000-0000-000000000001")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"Admin Local Updated","enabled":true,"is_default":false,"config_json":{"root_path":"/tmp/admin-local"},"rule_json":{}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_storage_response.status(), StatusCode::FORBIDDEN);

    let delete_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/files/storages/00000000-0000-0000-0000-000000000001")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_storage_response.status(), StatusCode::FORBIDDEN);

    let delete_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}"
                ))
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_table_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn file_management_settings_routes_allow_root_to_update_and_delete_storage_and_delete_table()
{
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_response.status(), StatusCode::OK);
    let storages_payload = response_json(storages_response).await;
    let default_storage_id = storages_payload["data"][0]["id"].as_str().unwrap();

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "root_cleanup_docs",
                        "title": "Root Cleanup Docs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();

    let storage_root =
        std::env::temp_dir().join(format!("file-management-update-{}", Uuid::now_v7()));
    let create_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "cleanup_local",
                        "title": "Cleanup Local",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": {
                            "root_path": storage_root.display().to_string()
                        },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_storage_response.status(), StatusCode::CREATED);
    let storage_payload = response_json(create_storage_response).await;
    let storage_id = storage_payload["data"]["id"].as_str().unwrap().to_string();

    let update_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/files/storages/{storage_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Cleanup Archive",
                        "enabled": false,
                        "is_default": false,
                        "config_json": {
                            "root_path": storage_root.display().to_string(),
                            "public_base_url": "https://files.example.com"
                        },
                        "rule_json": {
                            "description": "archive"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_storage_response.status(), StatusCode::OK);
    let updated_storage_payload = response_json(update_storage_response).await;
    assert_eq!(
        updated_storage_payload["data"]["title"].as_str(),
        Some("Cleanup Archive")
    );
    assert_eq!(
        updated_storage_payload["data"]["enabled"].as_bool(),
        Some(false)
    );
    assert_eq!(
        updated_storage_payload["data"]["config_json"]["public_base_url"].as_str(),
        Some("https://files.example.com")
    );

    let delete_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_table_response.status(), StatusCode::NO_CONTENT);

    let list_tables_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tables_response.status(), StatusCode::OK);
    let list_tables_payload = response_json(list_tables_response).await;
    let table_records = list_tables_payload["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(table_records
        .iter()
        .all(|record| record["id"].as_str() != Some(file_table_id.as_str())));

    let delete_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/settings/files/storages/{storage_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_storage_response.status(), StatusCode::NO_CONTENT);

    let storages_after_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_after_delete_response.status(), StatusCode::OK);
    let storages_after_delete_payload = response_json(storages_after_delete_response).await;
    let storage_records = storages_after_delete_payload["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(storage_records
        .iter()
        .any(|record| record["id"].as_str() == Some(default_storage_id)));
    assert!(storage_records
        .iter()
        .all(|record| record["id"].as_str() != Some(storage_id.as_str())));

    let _ = std::fs::remove_dir_all(storage_root);
}
