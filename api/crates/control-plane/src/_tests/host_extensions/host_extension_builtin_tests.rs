use std::{fs, path::PathBuf};

use control_plane::host_extension_boot::register_builtin_host_extension_contributions;
use plugin_framework::{
    parse_host_extension_contribution_manifest, parse_plugin_manifest, HostExtensionBootstrapPhase,
    HostExtensionContributionManifest, PluginManifestV1,
};

#[test]
fn contribution_backed_builtins_populate_registry() {
    let manifests = load_builtin_manifest_pairs();
    let registry =
        register_builtin_host_extension_contributions(&manifests).expect("registry should build");

    let local = registry
        .extension("official.local-infra-host")
        .expect("local infra extension should be registered");
    assert_eq!(local.bootstrap_phase, HostExtensionBootstrapPhase::PreState);
    assert!(local.infrastructure_providers.iter().any(|provider| {
        provider.contract == "storage-ephemeral" && provider.provider_code == "local"
    }));
    assert!(registry
        .infrastructure_provider("cache-store", "local")
        .is_some());
    assert!(registry
        .infrastructure_provider("provider-transport-store", "local")
        .is_some());

    let identity = registry
        .extension("official.identity-host")
        .expect("identity extension should be registered");
    assert_eq!(identity.owned_resources, vec!["identity"]);

    let workspace = registry
        .extension("official.workspace-host")
        .expect("workspace extension should be registered");
    assert_eq!(workspace.owned_resources, vec!["workspace"]);
    assert_eq!(workspace.extends_resources, vec!["identity"]);
}

fn load_builtin_manifest_pairs() -> Vec<(PluginManifestV1, HostExtensionContributionManifest)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    [
        "official.identity-host",
        "official.workspace-host",
        "official.plugin-host",
        "official.local-infra-host",
        "official.file-management-host",
        "official.runtime-orchestration-host",
    ]
    .into_iter()
    .map(|extension_id| {
        let manifest_path = root
            .join("plugins/host-extensions")
            .join(extension_id)
            .join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path).unwrap();
        let manifest = parse_plugin_manifest(&manifest_raw).unwrap();
        let contribution_path = manifest_path
            .parent()
            .unwrap()
            .join(&manifest.runtime.entry);
        let contribution_raw = fs::read_to_string(contribution_path).unwrap();
        let contribution = parse_host_extension_contribution_manifest(&contribution_raw).unwrap();
        (manifest, contribution)
    })
    .collect()
}
