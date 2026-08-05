use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::{
    host_extension_dropin::{
        scan_host_extension_dropins, scan_host_extension_dropins_with_policy,
        HostExtensionDropinPolicy,
    },
    PluginConsumptionKind,
};
use uuid::Uuid;

struct TempDropinFixture {
    root: PathBuf,
}

impl TempDropinFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("plugin-framework-host-dropin-{}", Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_manifest(&self, relative_dir: &str, trust_level: &str) {
        let package_root = self.root.join(relative_dir);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(
            package_root.join("manifest.yaml"),
            format!(
                r#"manifest_version: 1
plugin_id: fixture_host_extension@0.1.0
version: 0.1.0
publisher_namespace: fixture
vendor: fixture
display_name: Fixture Host Extension
description: Fixture host extension
icon: icon.svg
source_kind: filesystem_dropin
trust_level: {trust_level}
consumption_kind: host_extension
execution_mode: in_process
slot_codes: []
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: none
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: bin/fixture-host-extension
node_contributions: []
"#
            ),
        )
        .unwrap();
    }
}

impl Drop for TempDropinFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn scan_dropins_accepts_signed_host_extension_package() {
    let fixture = TempDropinFixture::new();
    fixture.write_manifest("host-extensions/signed_fixture", "verified_official");

    let result = scan_host_extension_dropins(fixture.path()).unwrap();

    assert_eq!(result.installations.len(), 1);
    assert_eq!(result.installations[0].source_kind, "filesystem_dropin");
    assert_eq!(
        result.installations[0].manifest.consumption_kind,
        PluginConsumptionKind::HostExtension
    );
    assert!(result.warnings.is_empty());
}

#[test]
fn scan_dropins_rejects_unverified_package_when_policy_disallows_it() {
    let fixture = TempDropinFixture::new();
    fixture.write_manifest("host-extensions/unverified_fixture", "unverified");

    let error = scan_host_extension_dropins_with_policy(
        fixture.path(),
        HostExtensionDropinPolicy {
            allow_unverified_filesystem_dropins: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("filesystem_dropin"));
}
