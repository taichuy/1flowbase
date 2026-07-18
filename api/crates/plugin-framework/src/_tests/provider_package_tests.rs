use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::{
    installation::ProviderCatalogEntry,
    provider_contract::{ModelDiscoveryMode, ProviderModelSource},
    provider_package::ProviderPackage,
    PluginConsumptionKind,
};
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

fn make_package_fixture() -> TempProviderPackage {
    let fixture = TempProviderPackage::new();
    fixture.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: acme_openai_compatible
version: 1.2.3
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
  limits:
    timeout_ms: 30000
    memory_bytes: 268435456
node_contributions: []
"#,
    );
    fixture.write(
        "provider/acme_openai_compatible.yaml",
        r#"provider_code: acme_openai_compatible
display_name: Acme OpenAI Compatible
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
supports_model_fetch_without_credentials: true
parameter_form:
  schema_version: 1.0.0
  title: Provider Parameters
  fields:
    - key: temperature
      label: Temperature
      type: number
      send_mode: optional
      enabled_by_default: true
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
  - key: api_protocol
    type: enum
    required: false
    control: select
    default_value: openai_chat
    options:
      - label: OpenAI Chat Completions
        value: openai_chat
      - label: OpenAI Responses
        value: openai_responses
  - key: organization
    type: string
    required: false
    advanced: true
"#,
    );
    fixture.write(
        "bin/acme_openai_compatible-provider",
        "#!/usr/bin/env bash\nexit 0\n",
    );
    fixture.write("models/llm/_position.yaml", "items:\n  - acme_chat\n");
    fixture.write(
        "models/llm/acme_chat.yaml",
        r#"model: acme_chat
label: Acme Chat
family: llm
capabilities:
  - stream
  - tool_call
supports_multimodal: true
context_window: 128000
max_output_tokens: 4096
provider_metadata:
  tier: default
"#,
    );
    fixture.write(
        "i18n/en_US.json",
        r#"{
  "plugin": {
    "label": "Acme English",
    "description": "Acme provider"
  },
  "provider": {
    "label": "Acme Provider"
  },
  "fields": {
    "api_key": {
      "label": "API Key"
    }
  }
}
"#,
    );
    fixture.write(
        "i18n/zh_Hans.json",
        r#"{
  "plugin": {
    "label": "Acme 中文"
  }
}
"#,
    );
    fixture
}

#[test]
fn provider_package_loads_manifest_v1_runtime_entry_and_static_models() {
    let fixture = make_package_fixture();

    let package = ProviderPackage::load_from_dir(fixture.path()).unwrap();

    assert_eq!(package.identifier(), "acme_openai_compatible@1.2.3");
    assert_eq!(package.manifest.plugin_id, "acme_openai_compatible");
    assert_eq!(
        package.manifest.consumption_kind,
        PluginConsumptionKind::RuntimeExtension
    );
    assert_eq!(
        package.runtime_entry(),
        fixture.path().join("bin/acme_openai_compatible-provider")
    );
    assert_eq!(package.provider.provider_code, "acme_openai_compatible");
    assert_eq!(package.provider.protocol, "openai_compatible");
    assert_eq!(
        package.provider.model_discovery_mode,
        ModelDiscoveryMode::Hybrid
    );
    assert!(package.provider.supports_model_fetch_without_credentials);
    assert_eq!(
        package
            .provider
            .parameter_form
            .as_ref()
            .expect("provider parameter form should load")
            .fields[0]
            .key,
        "temperature"
    );
    assert_eq!(package.predefined_models.len(), 1);

    let model = &package.predefined_models[0];
    assert_eq!(model.model_id, "acme_chat");
    assert_eq!(model.display_name, "Acme Chat");
    assert_eq!(model.source, ProviderModelSource::Static);
    assert!(model.supports_streaming);
    assert!(model.supports_tool_call);
    assert!(model.supports_multimodal);
    assert_eq!(model.context_window, Some(128000));
    assert_eq!(model.max_output_tokens, Some(4096));
    assert!(
        serde_json::to_value(model)
            .unwrap()
            .get("parameter_form")
            .is_none(),
        "predefined models should stay metadata-only"
    );

    let catalog_entry = ProviderCatalogEntry::from_package(&package);
    assert_eq!(catalog_entry.plugin_id, "acme_openai_compatible@1.2.3");
    assert_eq!(catalog_entry.form_schema.len(), 4);
    assert_eq!(catalog_entry.form_schema[2].field_type, "enum");
    assert_eq!(
        catalog_entry.form_schema[2].control.as_deref(),
        Some("select")
    );
    assert_eq!(
        catalog_entry.form_schema[2].default_value.as_ref(),
        Some(&serde_json::json!("openai_chat"))
    );
    assert_eq!(catalog_entry.form_schema[2].options.len(), 2);
    assert_eq!(
        catalog_entry.form_schema[2].options[0].label,
        "OpenAI Chat Completions"
    );
    assert!(catalog_entry.form_schema[3].advanced);
    assert_eq!(catalog_entry.predefined_models.len(), 1);
    assert_eq!(catalog_entry.icon.as_deref(), Some("icon.svg"));
}

#[test]
fn provider_package_treats_vision_capability_as_multimodal() {
    let fixture = make_package_fixture();
    fixture.write(
        "models/llm/acme_vision.yaml",
        r#"model: acme_vision
label: Acme Vision
family: llm
capabilities:
  - stream
  - tool_call
  - vision
context_window: 128000
max_output_tokens: 4096
provider_metadata:
  tier: vision
"#,
    );

    let package = ProviderPackage::load_from_dir(fixture.path()).unwrap();
    let model = package
        .predefined_models
        .iter()
        .find(|model| model.model_id == "acme_vision")
        .expect("vision model should load");

    assert!(model.supports_multimodal);
}

#[test]
fn provider_package_falls_back_to_default_locale_for_missing_keys() {
    let fixture = make_package_fixture();

    let package = ProviderPackage::load_from_dir(fixture.path()).unwrap();

    assert_eq!(
        package.resolve_i18n_value(Some("zh_Hans"), "fields.api_key.label"),
        Some("API Key".to_string())
    );
    assert_eq!(
        package.resolve_i18n_value(Some("fr_FR"), "plugin.label"),
        Some("Acme English".to_string())
    );
}

#[test]
fn provider_package_requires_default_locale_bundle() {
    let fixture = make_package_fixture();
    fs::remove_file(fixture.path().join("i18n/en_US.json")).unwrap();

    let error = ProviderPackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(error.to_string().contains("en_US"));
}
