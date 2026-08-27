use std::{fs, path::Path, sync::Arc};

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
};
use control_plane::ports::{AuthRepository, PluginRepository, UpsertPluginInstallationInput};
use domain::{
    PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
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
publisher_namespace: 1flowbase-tests
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
    let (mut state, _database_url) = test_api_state_with_database_url().await;
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
            category: domain::ExtensionCategory::HostExtensions,
            organization: "test".into(),
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
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    PluginRepository::upsert_artifact_instance(
        &state.store,
        &control_plane::ports::UpsertPluginArtifactInstanceInput {
            node_id: state.api_node_id.clone(),
            installation_id: installation.id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some(install_root.display().to_string()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Disabled,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: false,
        },
    )
    .await
    .unwrap();

    let extension_assembly = crate::extension_bus::assemble_extension_graph_input(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    let extension_snapshot = Arc::new(
        crate::extension_bus::ExtensionBootSnapshot::compile(
            Arc::new(extension_assembly.compile_graph().unwrap()),
            extension_assembly.interface_operations(),
        )
        .unwrap(),
    );
    let interface_snapshot = extension_snapshot.interface_registry().unwrap().snapshot();
    let activated_route_assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        Some(interface_snapshot.as_ref()),
    );
    let activated_registry =
        crate::routes::console_route_assembly::compile_migrated_core_console_operation_registry(
            &state.settings_feature_registry,
            activated_route_assembly.bindings(),
        )
        .unwrap();
    let mutable_state = Arc::get_mut(&mut state).unwrap();
    mutable_state.extension_boot_snapshot = Some(Arc::clone(&extension_snapshot));
    mutable_state.console_operation_registry = Arc::new(activated_registry);
    let typed_payload = serde_json::to_value(
        crate::routes::host_infrastructure::interface_operation::invoke_providers_view(
            Arc::clone(&state),
            domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root"),
            interface_runtime::InterfaceProtocol::Internal,
        )
        .await
        .unwrap()
        .0
        .into_providers(),
    )
    .unwrap();

    let app = crate::app_with_state_and_config(Arc::clone(&state), &test_config());
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
    assert_eq!(list_payload["data"], typed_payload);
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

    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    let capability = crate::openapi_interface::build_openapi_capability_catalog(
        state.as_ref(),
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap()
    .into_iter()
    .find(|entry| {
        entry.interface.operation_id
            == crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    })
    .unwrap();
    let mcp_interface =
        crate::routes::mcp_management::mcp_interface_entry_from_capability(capability);
    let mcp_result = match crate::routes::mcp_management::debug_execute::execute_with_server_bindings(
        Arc::clone(&state),
        HeaderMap::new(),
        actor,
        mcp_interface,
        crate::routes::mcp_management::McpDebugExecuteBody {
            interface_id: crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID.to_string(),
            debug_response_mode:
                crate::routes::mcp_management::debug_execute::McpDebugResponseMode::ToolResult,
            mcp_arguments: json!({}),
            input_mapping: json!({"mappings": []}),
            output_mapping: json!({}),
        },
        crate::routes::mcp_management::debug_execute::McpServerBoundInputs {
            workspace_id: state.bootstrap_workspace_id,
        },
    )
    .await {
        Ok(value) => value,
        Err(_) => panic!("activated interface MCP wrapper must invoke the typed Kernel"),
    };
    assert_eq!(mcp_result, typed_payload);

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
