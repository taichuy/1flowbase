//! Real activation consumer against retained package bytes and an isolated PostgreSQL schema.
use super::*;
use control_plane::{
    plugin_management::{ManagedPluginActivationPort, ManagedPluginActivationService},
    ports::UpsertPluginArtifactInstanceInput,
};
use std::path::PathBuf;

struct PackageRoot(PathBuf);
impl Drop for PackageRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn retained_package() -> (PackageRoot, plugin_framework::PluginManifestV1, Vec<u8>) {
    let root = PackageRoot(
        std::env::temp_dir().join(format!("plugin-template-assets-{}", Uuid::now_v7())),
    );
    let directory = root.0.join("_assets/mcp");
    tokio::fs::create_dir_all(directory.join("tools"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(directory.join("instances"))
        .await
        .unwrap();
    let (manifest, mut package) = native_template_fixture("1.0.0");
    let mut first_tool_bytes = vec![];
    for (index, tool) in package.tools.iter().enumerate() {
        let relative = format!("tools/{index}.json");
        let path = directory.join(&relative);
        let bytes = serde_json::to_vec(tool).unwrap();
        if index == 0 {
            first_tool_bytes = bytes.clone();
        }
        tokio::fs::write(&path, bytes).await.unwrap();
        package.manifest.files.push(domain::McpBundleFile {
            path: relative,
            kind: domain::McpBundleFileKind::Tool,
            sha256: plugin_framework::compute_manifest_fingerprint(&path)
                .await
                .unwrap(),
        });
    }
    let path = directory.join("instances/owned.json");
    tokio::fs::write(&path, serde_json::to_vec(&package.instances[0]).unwrap())
        .await
        .unwrap();
    package.manifest.files.push(domain::McpBundleFile {
        path: "instances/owned.json".into(),
        kind: domain::McpBundleFileKind::Instance,
        sha256: plugin_framework::compute_manifest_fingerprint(&path)
            .await
            .unwrap(),
    });
    let path = directory.join("manifest.json");
    tokio::fs::write(&path, serde_json::to_vec(&package.manifest).unwrap())
        .await
        .unwrap();
    let mut document = manifest_document("1.0.0");
    document["managed_service"]["mcp_instance"] = serde_json::to_value(
        manifest
            .managed_service
            .as_ref()
            .unwrap()
            .mcp_instance
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    document["managed_service"]["mcp_instance"]["template"]["sha256"] =
        json!(plugin_framework::compute_manifest_fingerprint(&path)
            .await
            .unwrap());
    let bytes = document.to_string();
    tokio::fs::write(root.0.join("manifest.yaml"), &bytes)
        .await
        .unwrap();
    let manifest = plugin_framework::parse_plugin_manifest(&bytes).unwrap();
    (root, manifest, first_tool_bytes)
}

#[tokio::test]
async fn plugin_template_installed_assets_are_verified_before_atomic_activation_and_reset() {
    let (store, workspace, user) = seed_store().await;
    let actor = domain::ActorContext::root(user.id, workspace.id, "root");
    let (root, manifest, first_tool_bytes) = retained_package().await;
    let installation = install(&store, user.id, &manifest).await;
    store
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "plugin-template-node".into(),
            installation_id: installation,
            local_version: Some(manifest.version.clone()),
            local_checksum: None,
            local_path: Some(root.0.display().to_string()),
            package_path: None,
            manifest_fingerprint: Some(
                plugin_framework::compute_manifest_fingerprint(&root.0.join("manifest.yaml"))
                    .await
                    .unwrap(),
            ),
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
        })
        .await
        .unwrap();
    let service = ManagedPluginActivationService::new(
        store.clone(),
        "plugin-template-node".into(),
        root.0.clone(),
    );
    let tool_path = root.0.join("_assets/mcp/tools/0.json");
    tokio::fs::write(&tool_path, b"{}").await.unwrap();
    assert!(
        service.prepare(&actor, installation).await.is_err(),
        "tampered shard must fail before catalog mutation"
    );
    assert!(catalog(&store, user.id, workspace.id)
        .await
        .tools
        .is_empty());
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, installation)
        .await
        .unwrap());
    tokio::fs::write(&tool_path, &first_tool_bytes)
        .await
        .unwrap();
    service.prepare(&actor, installation).await.unwrap();
    let seeded = catalog(&store, user.id, workspace.id).await;
    assert_eq!(seeded.tools.len(), 2);
    assert_eq!(seeded.instances.len(), 1);
    sqlx::query("update mcp_instances set name='Manual edit' where id=$1")
        .bind(seeded.instances[0].id)
        .execute(store.pool())
        .await
        .unwrap();
    service.prepare(&actor, installation).await.unwrap();
    let restored = catalog(&store, user.id, workspace.id).await;
    assert_eq!(restored.instances[0].id, seeded.instances[0].id);
    assert_eq!(restored.instances[0].name, seeded.instances[0].name);
    let manifest_path = root.0.join("_assets/mcp/manifest.json");
    let original_manifest = tokio::fs::read(&manifest_path).await.unwrap();
    tokio::fs::write(&manifest_path, b"{}").await.unwrap();
    assert!(
        service.prepare(&actor, installation).await.is_err(),
        "bundle manifest is bound by signed plugin digest"
    );
    assert_eq!(catalog(&store, user.id, workspace.id).await, restored);
    tokio::fs::write(&manifest_path, original_manifest)
        .await
        .unwrap();
    let plugin_path = root.0.join("manifest.yaml");
    let original_plugin = tokio::fs::read(&plugin_path).await.unwrap();
    tokio::fs::write(&plugin_path, b"{}").await.unwrap();
    assert!(
        service.prepare(&actor, installation).await.is_err(),
        "installed manifest fingerprint must match"
    );
    assert_eq!(catalog(&store, user.id, workspace.id).await, restored);
    tokio::fs::write(&plugin_path, original_plugin)
        .await
        .unwrap();
    #[cfg(unix)]
    {
        let linked_tool = root.0.join("linked-tool.json");
        tokio::fs::rename(&tool_path, &linked_tool).await.unwrap();
        std::os::unix::fs::symlink(&linked_tool, &tool_path).unwrap();
        assert!(
            service.prepare(&actor, installation).await.is_err(),
            "even an internal symlink is rejected"
        );
        assert_eq!(catalog(&store, user.id, workspace.id).await, restored);
        tokio::fs::remove_file(&tool_path).await.unwrap();
        tokio::fs::rename(&linked_tool, &tool_path).await.unwrap();
        let tools_directory = root.0.join("_assets/mcp/tools");
        let linked_directory = root.0.join("linked-tools");
        tokio::fs::rename(&tools_directory, &linked_directory)
            .await
            .unwrap();
        std::os::unix::fs::symlink(&linked_directory, &tools_directory).unwrap();
        assert!(
            service.prepare(&actor, installation).await.is_err(),
            "parent-directory symlinks are rejected"
        );
        assert_eq!(catalog(&store, user.id, workspace.id).await, restored);
    }
}
