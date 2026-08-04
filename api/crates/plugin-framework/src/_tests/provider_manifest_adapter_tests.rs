use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::provider_package::ProviderPackage;
use uuid::Uuid;

struct TempProviderPackage {
    root: PathBuf,
}

impl TempProviderPackage {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("plugin-framework-tests-{}", Uuid::now_v7()));
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

impl Drop for TempProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn make_package_fixture(
    raw_provider_code: &str,
    slot_codes: &[&str],
    execution_mode: &str,
    runtime_protocol: &str,
) -> TempProviderPackage {
    let fixture = TempProviderPackage::new();
    fixture.write(
        "manifest.yaml",
        &format!(
            r#"manifest_version: 1
plugin_id: acme_openai_compatible@1.2.3
version: 1.2.3
publisher_namespace: taichuy
vendor: taichuy
display_name: Acme OpenAI Compatible
description: Acme provider package
icon: icon.svg
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: {execution_mode}
slot_codes:
{slot_codes}
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: {runtime_protocol}
  entry: bin/acme_openai_compatible-provider
node_contributions: []
"#,
            execution_mode = execution_mode,
            runtime_protocol = runtime_protocol,
            slot_codes = slot_codes
                .iter()
                .map(|value| format!("  - {value}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    );
    fixture.write(
        "provider/acme_openai_compatible.yaml",
        &format!(
            r#"provider_code: {raw_provider_code}
display_name: Acme OpenAI Compatible
protocol: openai_compatible
model_discovery: hybrid
config_schema:
  - key: api_key
    type: string
    required: true
"#
        ),
    );
    fixture.write(
        "bin/acme_openai_compatible-provider",
        "#!/usr/bin/env bash\nexit 0\n",
    );
    fixture.write(
        "i18n/en_US.json",
        "{ \"plugin\": { \"label\": \"Acme\" } }\n",
    );
    fixture
}

#[test]
fn provider_package_rejects_runtime_extension_without_model_provider_slot() {
    let fixture = make_package_fixture(
        "acme_openai_compatible",
        &["node_contribution"],
        "process_per_call",
        "stdio_json",
    );

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();

    assert!(error.to_string().contains("model_provider"));
}

#[test]
fn provider_package_rejects_provider_code_prefix_mismatch() {
    let fixture = make_package_fixture(
        "different_provider",
        &["model_provider"],
        "process_per_call",
        "stdio_json",
    );

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();

    assert!(error.to_string().contains(
        "provider_code different_provider does not match plugin_id prefix acme_openai_compatible"
    ));
}

#[test]
fn provider_package_rejects_plugin_id_version_mismatch() {
    let fixture = make_package_fixture(
        "acme_openai_compatible",
        &["model_provider"],
        "process_per_call",
        "stdio_json",
    );
    fixture.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: acme_openai_compatible@9.9.9
version: 1.2.3
publisher_namespace: taichuy
vendor: taichuy
display_name: Acme OpenAI Compatible
description: Acme provider package
icon: icon.svg
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/acme_openai_compatible-provider
node_contributions: []
"#,
    );

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();

    assert!(error
        .to_string()
        .contains("plugin_id version suffix must match version"));
}

#[test]
fn provider_package_accepts_stateful_provider_worker_runtime() {
    let fixture = make_package_fixture(
        "acme_openai_compatible",
        &["model_provider"],
        "stateful_provider_worker",
        "stdio_json_worker",
    );

    let package = ProviderPackage::load_from_dir(fixture.path()).unwrap();

    assert_eq!(package.provider.provider_code, "acme_openai_compatible");
    assert_eq!(
        package.manifest.execution_mode.as_str(),
        "stateful_provider_worker"
    );
    assert_eq!(package.manifest.runtime.protocol, "stdio_json_worker");
}

#[test]
fn provider_package_rejects_non_process_per_call_execution_mode() {
    let fixture = make_package_fixture(
        "acme_openai_compatible",
        &["model_provider"],
        "in_process",
        "stdio_json",
    );

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();

    assert!(error
        .to_string()
        .contains("model provider package must declare execution_mode=process_per_call"));
}

#[test]
fn provider_package_rejects_non_stdio_runtime_protocol() {
    let fixture = make_package_fixture(
        "acme_openai_compatible",
        &["model_provider"],
        "process_per_call",
        "native_host",
    );

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();

    assert!(error
        .to_string()
        .contains("model provider package must declare execution_mode=process_per_call"));
}
