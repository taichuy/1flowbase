use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config, write_test_executable,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use control_plane::ports::{CreatePluginAssignmentInput, UpsertPluginInstallationInput};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use plugin_framework::compute_manifest_fingerprint;
use serde_json::{Value, json};
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
        let root = std::env::temp_dir().join(format!("api-server-data-source-routes-{nonce}"));
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

fn create_fixture_package() -> TempDataSourcePackage {
    let package = TempDataSourcePackage::new();
    fs::create_dir_all(package.path().join("bin")).unwrap();
    fs::create_dir_all(package.path().join("datasource")).unwrap();
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_data_source@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Fixture Data Source
description: Fixture Data Source
icon: icon.svg
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
  entry: bin/fixture_data_source
"#,
    );
    package.write(
        "datasource/fixture_data_source.yaml",
        r#"source_code: fixture_data_source
display_name: Fixture Data Source
auth_modes:
  - api_key
capabilities:
  - validate_config
  - test_connection
  - discover_catalog
  - describe_resource
  - preview_read
  - import_snapshot
supports_sync: true
supports_webhook: false
resource_kinds:
  - object
config_schema:
  - key: client_id
    label: Client ID
    type: string
    required: true
  - key: client_secret
    label: Client Secret
    type: string
    required: true
    send_mode: secret_ref
  - key: headers
    label: Headers
    type: json
    required: false
"#,
    );
    write_test_executable(
        &package.path().join("bin/fixture_data_source"),
        r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${payload}" in
  *'"method":"validate_config"'*)
    printf '%s' '{"ok":true,"result":{"ok":true,"echoed":"route-secret-echo","authorization":"Bearer route-secret-echo","nested":{"token":"route-secret-echo"}}}'
    ;;
  *'"method":"test_connection"'*)
    printf '%s' '{"ok":true,"result":{"status":"ok"}}'
    ;;
  *'"method":"discover_catalog"'*)
    printf '%s' '{"ok":true,"result":[{"resource_key":"contacts","display_name":"Contacts","resource_kind":"object","metadata":{"authorization":"Bearer route-secret-echo","nested":{"token":"route-secret-echo"}}}]}'
    ;;
  *'"method":"describe_resource"'*)
    if [[ "${payload}" == *'"client_secret":"route-secret-echo"'* ]]; then
      printf '%s' '{"ok":true,"result":{"resource_key":"contacts","primary_key":"contact_id","fields":[{"key":"contact_id","label":"Contact ID","type":"string","required":true},{"key":"properties.email","label":"Email route-secret-echo","type":"string","control":"input"}],"supports_preview_read":true,"supports_import_snapshot":false,"capabilities":{"supports_list":true,"supports_get":true,"supports_create":true,"supports_update":true,"supports_delete":true,"supports_filter":true,"supports_sort":true,"supports_pagination":true,"supports_owner_filter":false,"supports_scope_filter":true,"supports_write":true,"supports_transactions":false},"metadata":{"display_name":"Contacts route-secret-echo"}}}'
    else
      printf '%s' '{"ok":false,"error":{"message":"missing stored secret","provider_summary":null}}'
      exit 1
    fi
    ;;
  *'"method":"preview_read"'*)
    printf '%s' '{"ok":true,"result":{"rows":[{"id":"1","email":"person@example.com","token":"route-secret-echo","authorization":"Bearer route-secret-echo","nested":{"secret":"route-secret-echo"}}],"next_cursor":null}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"message":"unknown method","provider_summary":null}}'
    exit 1
    ;;
esac
"#,
    );
    package
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

async fn seed_data_source_installation(
    state: &crate::app_state::ApiState,
    package_root: &Path,
) -> String {
    let root = state
        .store
        .find_user_for_password_login(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID, "root")
        .await
        .unwrap()
        .unwrap();
    let scope =
        <storage_durable::MainDurableStore as control_plane::ports::AuthRepository>::default_scope_for_user(
            &state.store,
            root.id,
    )
        .await
        .unwrap();
    let installation_id = uuid::Uuid::now_v7();
    let installed_path = PathBuf::from(&state.provider_install_root)
        .join("installed")
        .join("fixture_data_source")
        .join("0.1.0");
    copy_dir_all(package_root, &installed_path);
    let manifest_fingerprint = compute_manifest_fingerprint(&installed_path.join("manifest.yaml"))
        .await
        .unwrap();
    fs::write(
        installed_path.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&json!({
            "plugin_id": "fixture_data_source@0.1.0",
            "version": "0.1.0",
            "checksum": null,
            "manifest_fingerprint": manifest_fingerprint,
        }))
        .unwrap(),
    )
    .unwrap();

    <storage_durable::MainDurableStore as control_plane::ports::PluginRepository>::upsert_installation(
        &state.store,
        &UpsertPluginInstallationInput {
            installation_id,
            provider_code: "fixture_data_source".into(),
            plugin_id: "fixture_data_source@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Fixture Data Source".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: installed_path.display().to_string(),
            checksum: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();

    <storage_durable::MainDurableStore as control_plane::ports::PluginRepository>::create_assignment(
        &state.store,
        &CreatePluginAssignmentInput {
            installation_id,
            workspace_id: scope.workspace_id,
            provider_code: "fixture_data_source".into(),
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();

    installation_id.to_string()
}

#[tokio::test]
async fn data_source_create_rejects_missing_required_package_config_field() {
    let package = create_fixture_package();
    let (state, _database_url) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = seed_data_source_installation(&state, package.path()).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Missing Required Field",
                        "installation_id": installation_id.clone(),
                        "config_json": {},
                        "secret_json": {}
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
async fn data_source_create_rejects_fields_not_declared_by_package_config_schema() {
    let package = create_fixture_package();
    let (state, _database_url) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = seed_data_source_installation(&state, package.path()).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Undeclared Field",
                        "installation_id": installation_id,
                        "config_json": {
                            "client_id": "abc",
                            "undeclared": "must-not-be-stored"
                        },
                        "secret_json": { "client_secret": "secret" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Undeclared Secret Field",
                        "installation_id": installation_id,
                        "config_json": { "client_id": "abc" },
                        "secret_json": {
                            "client_secret": "secret",
                            "undeclared_secret": "must-not-be-stored"
                        }
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
async fn data_source_create_classifies_fields_by_schema_and_encrypts_secrets_at_rest() {
    let package = create_fixture_package();
    let (state, database_url) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = seed_data_source_installation(&state, package.path()).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Schema Classified",
                        "installation_id": installation_id.clone(),
                        "config_json": { "client_secret": "secret-from-public-input" },
                        "secret_json": { "client_id": "public-from-secret-input" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["data"]["backend"]["config_json"]["client_id"],
        json!("public-from-secret-input")
    );
    assert!(
        payload["data"]["backend"]["config_json"]
            .get("client_secret")
            .is_none()
    );
    assert!(!payload.to_string().contains("secret-from-public-input"));

    let canonical_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Canonical Buckets",
                        "installation_id": installation_id,
                        "config_json": { "client_id": "public-from-secret-input" },
                        "secret_json": { "client_secret": "secret-from-public-input" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let canonical_status = canonical_response.status();
    let canonical_body = to_bytes(canonical_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        canonical_status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&canonical_body)
    );
    let canonical_payload: Value = serde_json::from_slice(&canonical_body).unwrap();
    assert_eq!(
        payload["data"]["backend"]["config_json"],
        canonical_payload["data"]["backend"]["config_json"]
    );

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let instance_id = uuid::Uuid::parse_str(payload["data"]["id"].as_str().unwrap()).unwrap();
    let stored_secret: Value = sqlx::query_scalar(
        "select encrypted_secret_json from data_source_secrets where data_source_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored_secret["algorithm"],
        json!("aead_xchacha20poly1305_v1")
    );
    assert!(
        !stored_secret
            .to_string()
            .contains("secret-from-public-input")
    );

    sqlx::query(
        "update data_source_secrets set encrypted_secret_json = $2 where data_source_instance_id = $1",
    )
    .bind(instance_id)
    .bind(json!({
        "algorithm": "aead_xchacha20poly1305_v1",
        "nonce": "00"
    }))
    .execute(&pool)
    .await
    .unwrap();
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/validate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::INTERNAL_SERVER_ERROR);

    sqlx::query(
        "update data_source_secrets set encrypted_secret_json = $2 where data_source_instance_id = $1",
    )
    .bind(instance_id)
    .bind(json!({ "client_secret": "legacy-plaintext-secret" }))
    .execute(&pool)
    .await
    .unwrap();
    let legacy_plaintext = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/validate"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_plaintext.status(), StatusCode::OK);
}

#[tokio::test]
async fn ac_001_003_data_source_routes_unify_main_and_runtime_extension_sources() {
    let package = create_fixture_package();
    let (state, database_url) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = seed_data_source_installation(&state, package.path()).await;

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/data-sources/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        catalog_payload["data"]["entries"][0]["source_code"].as_str(),
        Some("fixture_data_source")
    );

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Fixture Data Source",
                        "installation_id": installation_id,
                        "config_json": {
                            "client_id": "abc",
                            "headers": [
                                { "name": "Authorization", "value": "route-header-secret" },
                                { "name": "X-Trace", "value": "not-secret" }
                            ]
                        },
                        "secret_json": { "client_secret": "route-secret-echo" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let instance_id = create_payload["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(create_payload["data"]["status"].as_str(), Some("draft"));
    assert_eq!(
        create_payload["data"]["backend"]["kind"].as_str(),
        Some("runtime_extension")
    );
    assert_eq!(
        create_payload["data"]["default_data_model_status"].as_str(),
        Some("published")
    );
    assert!(
        !create_payload["data"]
            .as_object()
            .unwrap()
            .contains_key("default_api_exposure_status")
    );
    assert!(!create_payload.to_string().contains("route-header-secret"));
    assert!(!create_payload.to_string().contains("route-secret-echo"));
    assert_eq!(
        create_payload["data"]["backend"]["config_json"]["headers"][0]["value"]["secret_ref"],
        create_payload["data"]["backend"]["secret_ref"]
    );
    assert_eq!(
        create_payload["data"]["backend"]["config_json"]["headers"][1]["value"].as_str(),
        Some("not-secret")
    );

    let list_data_sources = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_data_sources.status(), StatusCode::OK);
    let list_payload: Value = serde_json::from_slice(
        &to_bytes(list_data_sources.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let listed_sources = list_payload["data"].as_array().unwrap();
    assert_eq!(listed_sources.len(), 2);
    let main_source = &listed_sources[0];
    assert_eq!(main_source["id"], json!("main"));
    assert_eq!(main_source["backend"]["kind"], json!("core"));
    assert_eq!(main_source["fixed"], json!(true));
    assert_eq!(main_source["enabled"], json!(true));
    for forbidden_field in [
        "installation_id",
        "source_code",
        "config_json",
        "secret_ref",
        "secret_version",
    ] {
        assert!(
            !main_source["backend"]
                .as_object()
                .unwrap()
                .contains_key(forbidden_field)
        );
    }
    assert!(listed_sources.iter().any(|source| {
        source["id"].as_str() == Some(&instance_id)
            && source["backend"]["kind"].as_str() == Some("runtime_extension")
            && source["backend"]["installation_id"].as_str() == Some(installation_id.as_str())
            && source["default_data_model_status"].as_str() == Some("published")
            && !source
                .as_object()
                .unwrap()
                .contains_key("default_api_exposure_status")
    }));

    let main_source_defaults = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/settings/data-models/data-sources/main/defaults")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "default_data_model_status": "draft" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(main_source_defaults.status(), StatusCode::OK);
    let main_source_defaults_payload: Value = serde_json::from_slice(
        &to_bytes(main_source_defaults.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        main_source_defaults_payload["data"]["id"].as_str(),
        Some("main")
    );
    assert_eq!(
        main_source_defaults_payload["data"]["default_data_model_status"].as_str(),
        Some("draft")
    );
    assert!(
        !main_source_defaults_payload["data"]
            .as_object()
            .unwrap()
            .contains_key("default_api_exposure_status")
    );

    let update_defaults = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/defaults"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "default_data_model_status": "draft" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_defaults.status(), StatusCode::OK);
    let defaults_payload: Value = serde_json::from_slice(
        &to_bytes(update_defaults.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        defaults_payload["data"]["default_data_model_status"].as_str(),
        Some("draft")
    );
    assert!(
        !defaults_payload["data"]
            .as_object()
            .unwrap()
            .contains_key("default_api_exposure_status")
    );

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/validate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);
    let validate_payload: Value =
        serde_json::from_slice(&to_bytes(validate.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        validate_payload["data"]["data_source"]["status"].as_str(),
        Some("ready")
    );
    assert!(!validate_payload.to_string().contains("route-secret-echo"));
    assert_eq!(
        validate_payload["data"]["output"]["echoed"].as_str(),
        Some("***")
    );
    assert_eq!(
        validate_payload["data"]["output"]["authorization"].as_str(),
        Some("Bearer ***")
    );
    assert!(
        validate_payload["data"]
            .as_object()
            .is_some_and(|data| !data.contains_key("catalog"))
    );

    let discover = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/resources/discover"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discover.status(), StatusCode::OK);
    let discover_payload: Value =
        serde_json::from_slice(&to_bytes(discover.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(discover_payload["data"]["refresh_status"], json!("ready"));
    assert_eq!(
        discover_payload["data"]["entries"][0]["resource_key"],
        json!("contacts")
    );
    assert_eq!(
        discover_payload["data"]["entries"][0]["metadata"]["authorization"],
        json!("Bearer ***")
    );
    assert!(!discover_payload.to_string().contains("route-secret-echo"));

    let list_resources = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/resources"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resources.status(), StatusCode::OK);
    let list_resources_payload: Value = serde_json::from_slice(
        &to_bytes(list_resources.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        list_resources_payload["data"]["entries"],
        discover_payload["data"]["entries"]
    );

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/preview-read"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "resource_key": "contacts",
                        "limit": 20,
                        "options_json": { "sample": true }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_payload: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        preview_payload["data"]["output"]["rows"][0]["email"].as_str(),
        Some("person@example.com")
    );
    assert!(!preview_payload.to_string().contains("route-secret-echo"));
    assert_eq!(
        preview_payload["data"]["output"]["rows"][0]["token"].as_str(),
        Some("***")
    );
    assert_eq!(
        preview_payload["data"]["output"]["rows"][0]["authorization"].as_str(),
        Some("Bearer ***")
    );

    let rotate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/data-sources/{instance_id}/secret/rotate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "secret_json": { "client_secret": "rotated-route-secret" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let rotate_status = rotate.status();
    let rotate_body = to_bytes(rotate.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        rotate_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&rotate_body)
    );
    let rotate_payload: Value = serde_json::from_slice(&rotate_body).unwrap();
    assert_eq!(
        rotate_payload["data"]["backend"]["secret_version"].as_i64(),
        Some(2)
    );
    assert!(
        rotate_payload["data"]["backend"]["secret_ref"]
            .as_str()
            .is_some()
    );
    assert!(!rotate_payload.to_string().contains("rotated-route-secret"));

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let stored_secret: Value = sqlx::query_scalar(
        "select encrypted_secret_json from data_source_secrets where data_source_instance_id = $1",
    )
    .bind(uuid::Uuid::parse_str(&instance_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored_secret["algorithm"],
        json!("aead_xchacha20poly1305_v1")
    );
    assert!(!stored_secret.to_string().contains("rotated-route-secret"));
}

#[tokio::test]
async fn data_source_routes_map_resource_to_model_returns_external_mapping_and_redacts_descriptor()
{
    let package = create_fixture_package();
    let (state, _database_url) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = seed_data_source_installation(&state, package.path()).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "fixture_data_source",
                        "display_name": "Fixture Data Source",
                        "installation_id": installation_id,
                        "config_json": { "client_id": "abc" },
                        "secret_json": { "client_secret": "route-secret-echo" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let instance_id = create_payload["data"]["id"].as_str().unwrap();

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/validate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);

    let map = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/data-sources/{instance_id}/resources/map-to-model"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "resource_key": "contacts" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = map.status();
    let body = to_bytes(map.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        payload["data"]["source_kind"].as_str(),
        Some("external_source")
    );
    assert_eq!(
        payload["data"]["data_source_id"].as_str(),
        Some(instance_id)
    );
    assert!(payload["data"].get("data_source_instance_id").is_none());
    assert_eq!(
        payload["data"]["external_resource_key"].as_str(),
        Some("contacts")
    );
    assert!(payload["data"]["external_table_id"].is_null());
    assert_eq!(payload["data"]["fields"].as_array().unwrap().len(), 2);
    assert_eq!(
        payload["data"]["fields"][1]["code"].as_str(),
        Some("properties_email")
    );
    assert_eq!(
        payload["data"]["fields"][1]["external_field_key"].as_str(),
        Some("properties.email")
    );
    assert!(!payload.to_string().contains("route-secret-echo"));
    assert_eq!(payload["data"]["title"].as_str(), Some("Contacts ***"));
    assert_eq!(
        payload["data"]["fields"][1]["title"].as_str(),
        Some("Email ***")
    );

    let list_mapped_models = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions?data_source_id={instance_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_mapped_models.status(), StatusCode::OK);
    let list_payload: Value = serde_json::from_slice(
        &to_bytes(list_mapped_models.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let models = list_payload["data"].as_array().unwrap();
    assert!(models.iter().any(|model| {
        model["id"].as_str() == payload["data"]["id"].as_str()
            && model["data_source_id"].as_str() == Some(instance_id)
    }));
    assert!(models.iter().all(|model| {
        model["data_source_id"].as_str() == Some(instance_id)
            && model["source_kind"].as_str() == Some("external_source")
    }));
}
