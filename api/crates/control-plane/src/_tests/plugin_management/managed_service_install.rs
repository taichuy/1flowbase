use super::support::{
    actor_with_permissions, MemoryOfficialPluginSource, MemoryPluginManagementRepository,
    MemoryProviderRuntime,
};
use crate::{
    plugin_management::{
        ExtensionCatalogCategory, InstallExtensionNodePluginCommand, PluginManagementService,
    },
    ports::{PluginRepository, UpdatePluginDesiredStateInput},
};
use domain::PluginDesiredState;
use std::sync::Arc;
use uuid::Uuid;

fn package(version: &str) -> Vec<u8> {
    let manifest = format!(
        r#"manifest_version: 2
plugin_id: managed_fixture
version: {version}
publisher_namespace: acme
vendor: acme
display_name: Managed fixture
description: Managed installation lifecycle fixture
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes: []
binding_targets: [system]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.extension-bus/v1
schema_version: 1flowbase.plugin.manifest/v2
permissions: {{network: none, secrets: none, storage: none, mcp: none, subprocess: deny}}
runtime: {{protocol: stdio_json_multiplex_v1, entry: bin/fixture}}
managed:
  module:
    bus_version: v1
    module_id: managed_fixture
    module_version: {version}
    module_kind: runtime
    contributions:
      - contribution_id: managed_fixture.list
        contributor_module_id: managed_fixture
        point_id: 1flowbase.managed-service.operation
        contract_version: "1"
        mode: append
        required_permissions: [service.execute]
  execution_bindings:
    - contribution_id: managed_fixture.list
      execution_mode: process_per_call
      runtime: {{protocol: stdio_json_multiplex_v1, entry: bin/fixture}}
      handler: list
managed_service:
  scope: system
  feature: {{feature_id: managed_fixture.settings, label: Fixture, description: Manage fixture, route_id: fixture, path: /settings/fixture}}
  operations:
    - interface_id: managed_fixture.list
      contribution_id: managed_fixture.list
      method: GET
      path: /api/console/managed-services/managed_fixture/items
      summary: List items
      description: List shared items.
      input_schema: {{type: object}}
      output_schema: {{type: object}}
"#
    );
    let encoder = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for (name, bytes, mode) in [
        ("manifest.yaml", manifest.as_bytes(), 0o644),
        ("bin/fixture", b"#!/bin/sh\nexit 0\n".as_slice(), 0o755),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

#[tokio::test]
async fn managed_service_install_and_reupload_preserve_explicit_activation_boundary() {
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(Uuid::now_v7(), &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.extension-center").unwrap(),
            "extension_center.install.upload",
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    let install_root = std::env::temp_dir().join(format!("managed-install-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .for_extension_center_console_operation("extension_center.install.upload");
    let command = |version: &str| InstallExtensionNodePluginCommand {
        actor_user_id: repository.actor.user_id,
        category: ExtensionCatalogCategory::RuntimeExtensions,
        file_name: "managed.1flowbasepkg".into(),
        package_bytes: package(version),
        source_kind: "uploaded".into(),
    };
    let first = service
        .install_extension_node_plugin(command("0.1.0"))
        .await
        .unwrap();
    assert_eq!(
        first.installation.desired_state,
        PluginDesiredState::Disabled
    );
    // Re-upload of both an intentionally disabled and a previously active installation is state-neutral.
    for state in [
        PluginDesiredState::Disabled,
        PluginDesiredState::ActiveRequested,
    ] {
        repository
            .update_desired_state(&UpdatePluginDesiredStateInput {
                installation_id: first.installation.id,
                actor_user_id: repository.actor.user_id,
                desired_state: state,
            })
            .await
            .unwrap();
        let retry = service
            .install_extension_node_plugin(command("0.1.0"))
            .await
            .unwrap();
        assert_eq!(retry.installation.id, first.installation.id);
        assert_eq!(retry.installation.desired_state, state);
    }
    let upgrade = service
        .install_extension_node_plugin(command("0.2.0"))
        .await
        .unwrap();
    assert_eq!(
        upgrade.installation.desired_state,
        PluginDesiredState::Disabled
    );
    assert_eq!(
        repository
            .get_installation(first.installation.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
    assert!(runtime.loaded_installations().await.is_empty());
    assert!(repository
        .list_tasks()
        .await
        .unwrap()
        .iter()
        .all(|task| task.task_kind == domain::PluginTaskKind::Install));
    assert!(repository
        .audit_events()
        .await
        .iter()
        .all(|event| event == "plugin.installed"));
    std::fs::remove_dir_all(install_root).unwrap();
}
