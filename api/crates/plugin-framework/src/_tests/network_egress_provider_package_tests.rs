use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::{
    parse_plugin_manifest, NetworkEgressProviderPackage, PluginExecutionMode,
    NETWORK_EGRESS_PROVIDER_CONTRACT,
};
use uuid::Uuid;

struct ThirdPartyEgressPackageFixture {
    root: PathBuf,
}

impl ThirdPartyEgressPackageFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "plugin-framework-network-egress-tests-{}",
            Uuid::now_v7()
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

impl Drop for ThirdPartyEgressPackageFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn third_party_manifest() -> String {
    format!(
        r#"manifest_version: 1
plugin_id: acme_network_egress@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: Acme Networks
display_name: Acme Network Egress
description: Third-party stateful network egress runtime extension
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes:
  - network_egress_provider
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: {NETWORK_EGRESS_PROVIDER_CONTRACT}
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/acme-network-egress
"#
    )
}

fn third_party_package_fixture() -> ThirdPartyEgressPackageFixture {
    let fixture = ThirdPartyEgressPackageFixture::new();
    fixture.write("manifest.yaml", &third_party_manifest());
    fixture.write("bin/acme-network-egress", "#!/usr/bin/env bash\nexit 0\n");
    fixture.write(
        "provider/egress-provider.yaml",
        r#"provider_code: acme-network-egress
display_name: Acme Network Egress
form_schema:
  schema_version: 1flowbase.plugin.form/v1
  fields:
    - key: subscription_url
      label: Subscription URL
      type: string
      control: url
      required: true
      send_mode: secret
"#,
    );
    fixture
}

#[test]
fn ac_002_network_egress_slot_is_public_and_independent_of_provider_v2() {
    let manifest = parse_plugin_manifest(&third_party_manifest()).unwrap();

    assert_eq!(manifest.slot_codes, vec!["network_egress_provider"]);
    assert_eq!(
        manifest.contract_version,
        "1flowbase.network_egress_provider/v1"
    );
    assert_ne!(manifest.contract_version, "1flowbase.provider/v2");
    assert_eq!(
        manifest.execution_mode,
        PluginExecutionMode::StatefulRuntimeWorker
    );
}

#[test]
fn ac_003_third_party_egress_package_conforms_at_manifest_and_package_boundaries() {
    let fixture = third_party_package_fixture();

    let package = NetworkEgressProviderPackage::load_from_dir(fixture.path()).unwrap();

    assert_eq!(package.identifier(), "acme_network_egress@0.1.0");
    assert_eq!(
        package.manifest_path(),
        fixture.path().join("manifest.yaml")
    );
    assert_eq!(
        package.runtime_entry(),
        fixture.path().join("bin/acme-network-egress")
    );
    assert_eq!(package.provider.provider_code, "acme-network-egress");
    assert_eq!(
        package.provider.form_schema.fields[0].key,
        "subscription_url"
    );
}

#[test]
fn qf_002_egress_package_declares_the_plugin_specific_instance_form() {
    let fixture = third_party_package_fixture();
    fixture.write(
        "provider/egress-provider.yaml",
        "provider_code: acme\ndisplay_name: Acme\n",
    );

    let error = NetworkEgressProviderPackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(error.to_string().contains("form_schema"));
}

#[test]
fn ac_004_egress_contract_rejects_provider_v2_and_model_provider_coupling() {
    let provider_v2 =
        third_party_manifest().replace(NETWORK_EGRESS_PROVIDER_CONTRACT, "1flowbase.provider/v2");
    let provider_v2_error = parse_plugin_manifest(&provider_v2).unwrap_err();
    assert!(provider_v2_error
        .to_string()
        .contains("network_egress_provider"));

    let coupled_slot = third_party_manifest().replace(
        "  - network_egress_provider",
        "  - network_egress_provider\n  - model_provider",
    );
    let coupled_slot_error = parse_plugin_manifest(&coupled_slot).unwrap_err();
    assert!(coupled_slot_error.to_string().contains("only slot"));
}

#[test]
fn ac_016_egress_contract_requires_a_stateful_runtime_worker_and_runtime_entry() {
    let wrong_execution = third_party_manifest().replace(
        "execution_mode: stateful_runtime_worker",
        "execution_mode: process_per_call",
    );
    let execution_error = parse_plugin_manifest(&wrong_execution).unwrap_err();
    assert!(execution_error
        .to_string()
        .contains("stdio_json_worker runtime.protocol"));

    let fixture = ThirdPartyEgressPackageFixture::new();
    fixture.write("manifest.yaml", &third_party_manifest());
    let entry_error = NetworkEgressProviderPackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(entry_error
        .to_string()
        .contains("runtime entry does not exist"));
}
