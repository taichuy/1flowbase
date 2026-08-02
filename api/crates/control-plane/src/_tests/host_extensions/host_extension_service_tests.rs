use std::{fs, path::Path, sync::Arc};

use flate2::{write::GzEncoder, Compression};
use tar::Builder;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    plugin_management::{InstallUploadedPluginCommand, PluginManagementService},
    ports::PluginRepository,
};
use domain::{ActorContext, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus};

use super::super::plugin_management::support::{
    actor_with_permissions, MemoryOfficialPluginSource, MemoryPluginManagementRepository,
    MemoryProviderRuntime,
};

fn create_host_extension_fixture(root: &Path, version: &str) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"manifest_version: 1
plugin_id: fixture_host_extension@{version}
version: {version}
vendor: 1flowbase tests
display_name: Fixture Host Extension
description: Fixture startup-only host extension
icon: icon.svg
source_kind: uploaded
trust_level: unverified
consumption_kind: host_extension
execution_mode: in_process
slot_codes:
  - host_bootstrap
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: bin/fixture_host_extension
  limits: {{}}
"#
        ),
    )
    .unwrap();
    fs::write(root.join("bin/fixture_host_extension"), "host extension").unwrap();
}

fn pack_tar_gz(root: &Path) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    append_dir_to_tar(&mut builder, root, root);
    builder.finish().unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn append_dir_to_tar(builder: &mut Builder<GzEncoder<Vec<u8>>>, root: &Path, current: &Path) {
    let mut entries = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let relative = path.strip_prefix(root).unwrap();
        if path.is_dir() {
            append_dir_to_tar(builder, root, &path);
            continue;
        }
        builder.append_path_with_name(&path, relative).unwrap();
    }
}

#[tokio::test]
async fn uploaded_host_extension_is_saved_as_pending_restart() {
    let workspace_id = Uuid::now_v7();
    let actor = ActorContext::root(Uuid::now_v7(), workspace_id, "root");
    let repository = MemoryPluginManagementRepository::new(actor);
    let install_root =
        std::env::temp_dir().join(format!("host-extension-install-{}", Uuid::now_v7()));
    let package_root =
        std::env::temp_dir().join(format!("host-extension-package-{}", Uuid::now_v7()));
    create_host_extension_fixture(&package_root, "0.1.0");

    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let result = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "fixture_host_extension-0.1.0.1flowbasepkg".to_string(),
            package_bytes: pack_tar_gz(&package_root),
        })
        .await
        .unwrap();

    assert_eq!(
        result.installation.desired_state,
        PluginDesiredState::PendingRestart
    );
    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            result.installation.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
    assert_eq!(
        artifact.availability_status,
        PluginAvailabilityStatus::PendingRestart
    );
    assert_eq!(
        result.task.status_message.as_deref(),
        Some("installed; restart required")
    );
    assert!(artifact.package_path.is_some());
    assert_eq!(
        repository
            .list_pending_restart_host_extensions()
            .await
            .unwrap()
            .len(),
        1
    );

    let _ = fs::remove_dir_all(package_root);
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn non_root_cannot_upload_host_extension() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("host-extension-install-denied-{}", Uuid::now_v7()));
    let package_root =
        std::env::temp_dir().join(format!("host-extension-package-denied-{}", Uuid::now_v7()));
    create_host_extension_fixture(&package_root, "0.1.0");

    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let error = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "fixture_host_extension-0.1.0.1flowbasepkg".to_string(),
            package_bytes: pack_tar_gz(&package_root),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(code))
            if code == &"host_extension_root_required"
    ));

    let _ = fs::remove_dir_all(package_root);
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn uploaded_host_extension_requires_feature_flag() {
    let workspace_id = Uuid::now_v7();
    let actor = ActorContext::root(Uuid::now_v7(), workspace_id, "root");
    let repository = MemoryPluginManagementRepository::new(actor);
    let install_root =
        std::env::temp_dir().join(format!("host-extension-install-flag-{}", Uuid::now_v7()));
    let package_root =
        std::env::temp_dir().join(format!("host-extension-package-flag-{}", Uuid::now_v7()));
    create_host_extension_fixture(&package_root, "0.1.0");

    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .with_allow_uploaded_host_extensions(false);

    let error = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "fixture_host_extension-0.1.0.1flowbasepkg".to_string(),
            package_bytes: pack_tar_gz(&package_root),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(code))
            if code == &"uploaded_host_extensions_disabled"
    ));

    let _ = fs::remove_dir_all(package_root);
    let _ = fs::remove_dir_all(install_root);
}
