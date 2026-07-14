use std::{fs, path::PathBuf};

use api_server::host_extensions::builtin::{
    builtin_host_extension_manifest_paths, load_builtin_host_extension_manifests,
};
use plugin_framework::HostExtensionBootstrapPhase;

#[test]
fn builtin_manifest_paths_point_to_plugin_workspace_sources() {
    let paths = builtin_host_extension_manifest_paths();

    assert_eq!(
        paths,
        vec![
            "plugins/host-extensions/official.identity-host/manifest.yaml",
            "plugins/host-extensions/official.workspace-host/manifest.yaml",
            "plugins/host-extensions/official.plugin-host/manifest.yaml",
            "plugins/host-extensions/official.local-infra-host/manifest.yaml",
            "plugins/host-extensions/official.file-management-host/manifest.yaml",
            "plugins/host-extensions/official.runtime-orchestration-host/manifest.yaml",
        ]
    );
}

#[test]
fn builtin_source_does_not_embed_full_yaml_manifest_strings() {
    let source_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/host_extensions/builtin.rs");
    let source = fs::read_to_string(source_path).expect("builtin source should be readable");

    assert!(!source.contains("const IDENTITY_HOST"));
    assert!(!source.contains("manifest_version: 1\nextension_id:"));
    assert!(!source.contains("provides_contracts:"));
}

#[test]
fn builtin_manifests_load_from_plugin_workspace() {
    let workspace_root = api_workspace_root();
    let loaded = load_builtin_host_extension_manifests(&workspace_root)
        .expect("builtin manifests should load from plugin workspace");

    assert_eq!(loaded.len(), 6);
    assert!(loaded.iter().all(|(manifest, contribution)| {
        manifest.runtime.entry == "host-extension.yaml"
            && manifest.plugin_code().unwrap() == contribution.extension_id
    }));

    let local_infra = loaded
        .iter()
        .find(|(_, contribution)| contribution.extension_id == "official.local-infra-host")
        .expect("local infra builtin should be present");
    assert_eq!(
        local_infra.1.bootstrap_phase,
        HostExtensionBootstrapPhase::PreState
    );
    assert!(local_infra
        .1
        .infrastructure_providers
        .iter()
        .any(|provider| provider.contract == "storage-ephemeral"
            && provider.provider_code == "local"));
    assert!(local_infra
        .1
        .infrastructure_providers
        .iter()
        .any(|provider| provider.contract == "session-store" && provider.provider_code == "local"));
    assert!(local_infra
        .1
        .infrastructure_providers
        .iter()
        .any(|provider| provider.contract == "runtime-event-stream"
            && provider.provider_code == "local"));
}

#[test]
fn default_set_places_local_infra_before_optional_boot_hosts() {
    let default_set = fs::read_to_string(api_workspace_root().join("plugins/sets/default.yaml"))
        .expect("default plugin set should be readable");

    let local_infra = default_set
        .find("official.local-infra-host")
        .expect("default set should include local infra");
    let file_management = default_set
        .find("official.file-management-host")
        .expect("default set should include file management host");
    let runtime_orchestration = default_set
        .find("official.runtime-orchestration-host")
        .expect("default set should include runtime orchestration host");

    assert!(local_infra < file_management);
    assert!(local_infra < runtime_orchestration);
}

#[test]
fn missing_builtin_manifest_path_reports_clear_load_error() {
    let missing_root = api_workspace_root().join("missing-api-workspace-root");
    let error = load_builtin_host_extension_manifests(&missing_root)
        .expect_err("missing workspace root should fail");
    let message = format!("{error:#}");

    assert!(message.contains("failed to read builtin host extension manifest"));
    assert!(message.contains("plugins/host-extensions/official.identity-host/manifest.yaml"));
}

fn api_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
