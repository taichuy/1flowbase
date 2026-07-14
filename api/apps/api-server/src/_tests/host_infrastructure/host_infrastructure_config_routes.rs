use std::{fs, path::Path};

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{AuthRepository, PluginRepository, UpsertPluginInstallationInput};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn write_host_extension_fixture(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: redis-infra-host@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Redis Infra Host
description: Redis host infrastructure fixture
source_kind: uploaded
trust_level: unverified
consumption_kind: host_extension
execution_mode: in_process
slot_codes: [host_bootstrap]
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: host_managed
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: host-extension.yaml
"#,
    )
    .unwrap();
    fs::write(
        root.join("host-extension.yaml"),
        r#"schema_version: 1flowbase.host-extension/v1
extension_id: redis-infra-host
version: 0.1.0
bootstrap_phase: pre_state
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://redis-infra-host
  entry_symbol: redis_infra_host
owned_resources: []
extends_resources: []
infrastructure_providers:
  - contract: storage-ephemeral
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
      - key: port
        label: Port
        type: number
        required: true
  - contract: cache-store
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
      - key: port
        label: Port
        type: number
        required: true
routes: []
workers: []
migrations: []
"#,
    )
    .unwrap();
}

#[tokio::test]
async fn host_infrastructure_config_routes_list_inactive_provider_and_save_pending_restart() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    let root = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();
    let install_root =
        std::env::temp_dir().join(format!("host-infra-config-route-{}", Uuid::now_v7()));
    write_host_extension_fixture(&install_root);
    let installation = PluginRepository::upsert_installation(
        &state.store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "redis-infra-host".into(),
            plugin_id: "redis-infra-host@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.host_extension/v1".into(),
            protocol: "native_host".into(),
            display_name: "Redis Infra Host".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::Disabled,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Disabled,
            package_path: None,
            installed_path: install_root.display().to_string(),
            checksum: None,
            manifest_fingerprint: None,
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

    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/providers")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload = response_json(list_response).await;
    assert_eq!(list_payload["data"][0]["display_name"], "Redis");
    assert_eq!(list_payload["data"][0]["runtime_status"], "inactive");
    assert_eq!(list_payload["data"][0]["desired_state"], "disabled");
    assert_eq!(
        list_payload["data"][0]["contracts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(list_payload["data"][0]["config_schema"][0]["key"], "host");

    let save_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/host-infrastructure/providers/{}/redis/config",
                    installation.id
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled_contracts": ["storage-ephemeral"],
                        "config_json": { "host": "localhost", "port": 6379 }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save_response.status(), StatusCode::OK);
    let save_payload = response_json(save_response).await;
    assert_eq!(save_payload["data"]["restart_required"], true);
    assert_eq!(
        save_payload["data"]["installation_desired_state"],
        "pending_restart"
    );
    assert_eq!(
        save_payload["data"]["provider_config_status"],
        "pending_restart"
    );

    let refreshed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/providers")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed_response.status(), StatusCode::OK);
    let refreshed_payload = response_json(refreshed_response).await;
    assert_eq!(
        refreshed_payload["data"][0]["desired_state"],
        "pending_restart"
    );
    assert_eq!(refreshed_payload["data"][0]["runtime_status"], "inactive");
    assert_eq!(refreshed_payload["data"][0]["restart_required"], true);
    assert_eq!(
        refreshed_payload["data"][0]["config_json"],
        json!({ "host": "localhost", "port": 6379 })
    );

    let _ = fs::remove_dir_all(install_root);
}
