use crate::_tests::support::{login_and_capture_cookie, test_app, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use sqlx::Row;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

struct TempDataSourcePackage {
    root: PathBuf,
}

impl TempDataSourcePackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "api-runtime-model-data-source-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
}

impl Drop for TempDataSourcePackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_external_runtime_package(package: &TempDataSourcePackage) {
    let list_output = json!({
        "ok": true,
        "result": {
            "rows": [{
                "id": "contact-1",
                "email_address": "list@example.com",
                "secret_echo": "Bearer route-runtime-secret"
            }],
            "total_count": 1,
            "metadata": {}
        }
    })
    .to_string();
    let get_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-1",
                "email_address": "get@example.com",
                "secret_echo": "Bearer route-runtime-secret"
            },
            "metadata": {}
        }
    })
    .to_string();
    let create_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-created",
                "email_address": "created@example.com",
                "secret_echo": "Bearer route-runtime-secret"
            },
            "metadata": {}
        }
    })
    .to_string();
    let update_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-1",
                "email_address": "updated@example.com",
                "secret_echo": "Bearer route-runtime-secret"
            },
            "metadata": {}
        }
    })
    .to_string();
    let delete_output = json!({
        "ok": true,
        "result": {
            "deleted": true,
            "metadata": {}
        }
    })
    .to_string();
    let error_output = json!({
        "ok": false,
        "error": {
            "message": "runtime CRUD request missing connection secret or unsupported method",
            "provider_summary": null
        }
    })
    .to_string();

    package.write(
        "bin/fixture_external_data_source",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${{payload}}" in
  *'"method":"list_records"'*)
    if [[ "${{payload}}" == *'"client_secret":"route-runtime-secret"'* && "${{payload}}" == *'"resource_key":"contacts"'* ]]; then
      printf '%s' '{list_output}'
    else
      printf '%s' '{error_output}'
      exit 1
    fi
    ;;
  *'"method":"get_record"'*)
    if [[ "${{payload}}" == *'"client_secret":"route-runtime-secret"'* && "${{payload}}" == *'"record_id":"contact-1"'* ]]; then
      printf '%s' '{get_output}'
    else
      printf '%s' '{error_output}'
      exit 1
    fi
    ;;
  *'"method":"create_record"'*)
    if [[ "${{payload}}" == *'"client_secret":"route-runtime-secret"'* && "${{payload}}" == *'"transaction_id":null'* ]]; then
      printf '%s' '{create_output}'
    else
      printf '%s' '{error_output}'
      exit 1
    fi
    ;;
  *'"method":"update_record"'*)
    if [[ "${{payload}}" == *'"client_secret":"route-runtime-secret"'* && "${{payload}}" == *'"transaction_id":null'* ]]; then
      printf '%s' '{update_output}'
    else
      printf '%s' '{error_output}'
      exit 1
    fi
    ;;
  *'"method":"delete_record"'*)
    if [[ "${{payload}}" == *'"client_secret":"route-runtime-secret"'* && "${{payload}}" == *'"transaction_id":null'* ]]; then
      printf '%s' '{delete_output}'
    else
      printf '%s' '{error_output}'
      exit 1
    fi
    ;;
  *)
    printf '%s' '{error_output}'
    exit 1
    ;;
esac
"#
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_external_data_source");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_external_data_source@0.1.0
version: 0.1.0
vendor: taichuy
display_name: Fixture External Data Source
description: Fixture External Data Source
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - data_source
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.data_source/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_external_data_source
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
node_contributions: []
"#,
    );
    package.write(
        "datasource/fixture_external_data_source.yaml",
        r#"source_code: fixture_external_data_source
display_name: Fixture External Data Source
auth_modes:
  - api_key
capabilities:
  - list_records
  - get_record
  - create_record
  - update_record
  - delete_record
supports_sync: false
supports_webhook: false
resource_kinds:
  - object
config_schema:
  - key: client_id
    label: Client ID
    type: string
    required: true
"#,
    );
}

async fn revoke_model_grant(database_url: &str, model_id: &str) {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    sqlx::query(
        r#"
        delete from scope_data_model_grants
        where data_model_id = $1
        "#,
    )
    .bind(uuid::Uuid::parse_str(model_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
}

async fn seed_runtime_data_source_instance(
    database_url: &str,
    package: &TempDataSourcePackage,
) -> String {
    seed_runtime_data_source_instance_with_options(
        database_url,
        package,
        RuntimeDataSourceSeedOptions::default(),
    )
    .await
}

struct RuntimeDataSourceSeedOptions<'a> {
    provider_code: &'a str,
    source_code: &'a str,
    contract_version: &'a str,
    desired_state: &'a str,
    artifact_status: &'a str,
    runtime_status: &'a str,
    availability_status: &'a str,
    instance_status: &'a str,
    installed_path: Option<&'a str>,
    assign: bool,
}

impl Default for RuntimeDataSourceSeedOptions<'_> {
    fn default() -> Self {
        Self {
            provider_code: "fixture_external_data_source",
            source_code: "fixture_external_data_source",
            contract_version: "1flowbase.data_source/v1",
            desired_state: "active_requested",
            artifact_status: "ready",
            runtime_status: "active",
            availability_status: "available",
            instance_status: "ready",
            installed_path: None,
            assign: true,
        }
    }
}

async fn seed_runtime_data_source_instance_with_options(
    database_url: &str,
    package: &TempDataSourcePackage,
    options: RuntimeDataSourceSeedOptions<'_>,
) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let actor = sqlx::query(
        r#"
        select users.id as user_id, workspace_memberships.workspace_id as workspace_id
        from users
        join workspace_memberships on workspace_memberships.user_id = users.id
        where users.account = 'root'
        order by workspace_memberships.created_at asc
        limit 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_user_id: uuid::Uuid = actor.get("user_id");
    let workspace_id: uuid::Uuid = actor.get("workspace_id");
    let installation_id = uuid::Uuid::now_v7();
    let assignment_id = uuid::Uuid::now_v7();
    let data_source_instance_id = uuid::Uuid::now_v7();
    let installed_path = options
        .installed_path
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| package.path().display().to_string());

    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, metadata_json, created_by
        ) values (
            $1, 'capability-plugins', 'test', $2, '0.1.0',
            'fixture_external_data_source@0.1.0', $3, 'stdio_json',
            'Fixture External Data Source', 'uploaded', 'unverified', 'valid',
            $4, 'missing', '{}', $5
        )
        "#,
    )
    .bind(installation_id)
    .bind(options.provider_code)
    .bind(options.contract_version)
    .bind(options.desired_state)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into extension_artifact_instances (
            node_id, installation_id, local_version, local_path,
            artifact_status, runtime_status, availability_status
        ) values ($1, $2, '0.1.0', $3, $4, $5, $6)
        "#,
    )
    .bind(crate::_tests::support::test_config().api_node_id)
    .bind(installation_id)
    .bind(installed_path)
    .bind(options.artifact_status)
    .bind(options.runtime_status)
    .bind(options.availability_status)
    .execute(&pool)
    .await
    .unwrap();

    if options.assign {
        sqlx::query(
            r#"
            insert into plugin_assignments (
                id, installation_id, workspace_id, provider_code, assigned_by
            ) values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(assignment_id)
        .bind(installation_id)
        .bind(workspace_id)
        .bind(options.provider_code)
        .bind(actor_user_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    sqlx::query(
        r#"
        insert into data_source_instances (
            id, workspace_id, installation_id, source_code, display_name, status,
            config_json, metadata_json, default_data_model_status,
            created_by
        ) values (
            $1, $2, $3, $4, 'Fixture External Data Source',
            $5, '{"client_id":"route-runtime-client"}', '{}',
            'published', $6
        )
        "#,
    )
    .bind(data_source_instance_id)
    .bind(workspace_id)
    .bind(installation_id)
    .bind(options.source_code)
    .bind(options.instance_status)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into data_source_secrets (
            id, scope_id, data_source_instance_id, encrypted_secret_json,
            secret_version, created_by, updated_by
        ) values (
            $1, (select scope_id from data_source_instances where id = $2),
            $2, '{"client_secret":"route-runtime-secret"}', 1, $3, $3
        )
        "#,
    )
    .bind(uuid::Uuid::now_v7())
    .bind(data_source_instance_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    data_source_instance_id.to_string()
}

async fn seed_external_runtime_model(
    database_url: &str,
    data_source_instance_id: &str,
    code: &str,
    title: &str,
) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let actor = sqlx::query(
        r#"
        select users.id as user_id, workspace_memberships.workspace_id as workspace_id
        from users
        join workspace_memberships on workspace_memberships.user_id = users.id
        where users.account = 'root'
        order by workspace_memberships.created_at asc
        limit 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_user_id: uuid::Uuid = actor.get("user_id");
    let workspace_id: uuid::Uuid = actor.get("workspace_id");
    let model_id = uuid::Uuid::now_v7();

    sqlx::query(
        r#"
        insert into model_definitions (
            id, scope_kind, scope_id, data_source_instance_id, source_kind,
            external_resource_key, external_capability_snapshot,
            code, title, physical_table_name, acl_namespace, audit_namespace,
            availability_status, status, owner_kind, is_protected, created_by, updated_by
        ) values (
            $1, 'system', $2, $3, 'external_source', 'contacts', '{}',
            $4, $5, $6, $7, $8,
            'available', 'published', 'core', false, $9, $9
        )
        "#,
    )
    .bind(model_id)
    .bind(domain::SYSTEM_SCOPE_ID)
    .bind(uuid::Uuid::parse_str(data_source_instance_id).unwrap())
    .bind(code)
    .bind(title)
    .bind(format!("external_{}", model_id.simple()))
    .bind(format!("data_model.{code}"))
    .bind(format!("data_model.{code}"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into scope_data_model_grants (
            id, scope_kind, scope_id, data_model_id, enabled, permission_profile, created_by
        ) values ($1, 'workspace', $2, $3, true, 'scope_all', $4)
        "#,
    )
    .bind(uuid::Uuid::now_v7())
    .bind(workspace_id)
    .bind(model_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    model_id.to_string()
}

async fn create_user_api_key(app: &axum::Router, cookie: &str, csrf: &str, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/user-api-keys")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": name,
                        "expiration_policy": "never"
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
    payload["data"]["token"].as_str().unwrap().to_string()
}

async fn list_records_with_api_key(
    app: &axum::Router,
    model_code: &str,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/{model_code}/list"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, payload)
}

async fn create_member(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) -> String {
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": password,
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_member: serde_json::Value = serde_json::from_slice(&body).unwrap();
    created_member["data"]["id"].as_str().unwrap().to_string()
}

async fn replace_member_roles(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    member_id: &str,
    role_codes: &[&str],
) {
    let replace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": role_codes
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(replace_response.status(), StatusCode::NO_CONTENT);
}

async fn create_orders_model(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    create_model_with_status(app, cookie, csrf, "orders", None).await
}

async fn create_model_with_status(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
    status: Option<&str>,
) -> String {
    let mut body = json!({
        "scope_kind": "workspace",
        "code": code,
        "title": code
    });
    if let Some(status) = status {
        body["status"] = json!(status);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["scope_kind"], json!("system"));
    assert_eq!(
        payload["data"]["scope_id"],
        json!(domain::SYSTEM_SCOPE_ID.to_string())
    );
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn update_model_status(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    status: &str,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": status }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_runtime_record(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_code: &str,
    title: &str,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/runtime/models/{model_code}/create"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": title }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn assert_runtime_crud_blocked(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_code: &str,
    expected_status: StatusCode,
    expected_code: &str,
) {
    let record_id = uuid::Uuid::now_v7();
    let requests = [
        Request::builder()
            .method("GET")
            .uri(format!("/api/runtime/models/{model_code}/list"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("GET")
            .uri(format!("/api/runtime/models/{model_code}/get/{record_id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/runtime/models/{model_code}/create"))
            .header("cookie", cookie)
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "title": "blocked" }).to_string()))
            .unwrap(),
        Request::builder()
            .method("PATCH")
            .uri(format!(
                "/api/runtime/models/{model_code}/update/{record_id}"
            ))
            .header("cookie", cookie)
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "title": "blocked" }).to_string()))
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!(
                "/api/runtime/models/{model_code}/delete/{record_id}"
            ))
            .header("cookie", cookie)
            .header("x-csrf-token", csrf)
            .body(Body::empty())
            .unwrap(),
    ];

    for request in requests {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected_status);
        let payload: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["code"], json!(expected_code));
    }
}

async fn create_text_field(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    code: &str,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "title": code,
                        "field_kind": "text",
                        "is_required": true,
                        "is_unique": false,
                        "display_options": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn create_text_field_with_external_key(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    code: &str,
    external_field_key: &str,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "title": code,
                        "external_field_key": external_field_key,
                        "field_kind": "text",
                        "is_required": false,
                        "is_unique": false,
                        "display_options": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn create_enum_field(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    code: &str,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "title": code,
                        "field_kind": "enum",
                        "is_required": true,
                        "is_unique": false,
                        "display_interface": "select",
                        "display_options": { "options": ["draft", "paid"] }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn drop_runtime_table(database_url: &str, model_id: &str) {
    let durable = storage_durable::build_main_durable_postgres(database_url)
        .await
        .unwrap();
    let pool = durable.store;
    let model_id = uuid::Uuid::parse_str(model_id).unwrap();
    let physical_table_name: String =
        sqlx::query_scalar("select physical_table_name from model_definitions where id = $1")
            .bind(model_id)
            .fetch_one(pool.pool())
            .await
            .unwrap();
    let statement = format!("drop table if exists \"{physical_table_name}\"");
    sqlx::query(&statement).execute(pool.pool()).await.unwrap();
}

async fn update_runtime_record_title_directly(
    database_url: &str,
    model_id: &str,
    record_id: &str,
    title: &str,
) {
    let durable = storage_durable::build_main_durable_postgres(database_url)
        .await
        .unwrap();
    let pool = durable.store;
    let model_id = uuid::Uuid::parse_str(model_id).unwrap();
    let row = sqlx::query(
        r#"
        select model_definitions.physical_table_name, model_fields.physical_column_name
        from model_definitions
        join model_fields on model_fields.data_model_id = model_definitions.id
        where model_definitions.id = $1
          and model_fields.code = 'title'
        "#,
    )
    .bind(model_id)
    .fetch_one(pool.pool())
    .await
    .unwrap();
    let physical_table_name: String = row.get("physical_table_name");
    let physical_column_name: String = row.get("physical_column_name");
    let statement = format!(
        "update {} set {} = $1 where id = $2",
        quote_test_identifier(&physical_table_name),
        quote_test_identifier(&physical_column_name)
    );

    sqlx::query(&statement)
        .bind(title)
        .bind(uuid::Uuid::parse_str(record_id).unwrap())
        .execute(pool.pool())
        .await
        .unwrap();
}

fn quote_test_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

mod api_key_access;
mod crud_dispatch;
mod status_scope;
