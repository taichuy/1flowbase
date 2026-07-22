use super::repository::MemoryPluginManagementRepository;
use super::*;
use flate2::GzBuilder;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tar::{EntryType, Header};

#[derive(Serialize)]
struct OfficialReleaseDocument<'a> {
    schema_version: u32,
    plugin_id: String,
    provider_code: &'a str,
    version: &'a str,
    contract_version: &'static str,
    artifact_sha256: &'a str,
    payload_sha256: String,
    signature_algorithm: &'static str,
    signing_key_id: &'static str,
    issued_at: &'static str,
}

pub(crate) struct SignedUploadPackageFixture {
    pub(crate) package_bytes: Vec<u8>,
    pub(crate) public_key: plugin_framework::TrustedPublicKey,
}

pub(crate) fn actor_with_permissions(workspace_id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext::scoped(
        Uuid::now_v7(),
        workspace_id,
        "member",
        permissions.iter().map(|value| value.to_string()),
    )
}

fn write_test_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn write_provider_manifest_v2(root: &Path, provider_code: &str, display_name: &str, version: &str) {
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"manifest_version: 1
plugin_id: {provider_code}@{version}
version: {version}
vendor: 1flowbase tests
display_name: {display_name}
description: {display_name}
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
  entry: bin/{provider_code}-provider
"#
        ),
    )
    .unwrap();
}

fn write_provider_runtime_script(path: &Path, model_id: &str, model_label: &str) {
    let script = format!(
        r#"#!/usr/bin/env node
const fs = require('node:fs');

const request = JSON.parse(fs.readFileSync(0, 'utf8') || '{{}}');
const listModels = [{{
  model_id: "{model_id}",
  display_name: "{model_label}",
  source: "dynamic",
  supports_streaming: true,
  supports_tool_call: false,
  supports_multimodal: false,
  provider_metadata: {{}}
}}];

let result = {{}};
switch (request.method) {{
  case 'validate':
    result = {{
      sanitized: {{
        api_key: request.input?.api_key ? "***" : null
      }}
    }};
    break;
  case 'list_models':
    result = listModels;
    break;
  case 'invoke': {{
    const query = request.input?.messages?.[0]?.content ?? "";
    result = {{
      events: [
        {{ type: "text_delta", delta: "reply:" + query }},
        {{ type: "usage_snapshot", usage: {{ input_tokens: 5, output_tokens: 7, total_tokens: 12 }} }},
        {{ type: "finish", reason: "stop" }}
      ],
      result: {{
        final_content: "reply:" + query,
        usage: {{ input_tokens: 5, output_tokens: 7, total_tokens: 12 }},
        finish_reason: "stop"
      }}
    }};
    break;
  }}
  default:
    result = {{}};
}}

process.stdout.write(JSON.stringify({{ ok: true, result }}));
"#
    );
    write_test_executable(path, &script);
}

pub(crate) fn create_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    write_provider_manifest_v2(root, "fixture_provider", "Fixture Provider", "0.1.0");
    fs::write(
        root.join("provider/fixture_provider.yaml"),
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
parameter_form:
  schema_version: 1.0.0
  title: LLM Parameters
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
"#,
    )
    .unwrap();
    write_provider_runtime_script(
        &root.join("bin/fixture_provider-provider"),
        "fixture_chat",
        "Fixture Chat",
    );
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .unwrap();
    fs::write(
        root.join("models/llm/fixture_chat.yaml"),
        r#"model: fixture_chat
label: Fixture Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{
  "plugin": {
    "label": "Fixture Provider",
    "description": "Fixture provider plugin"
  },
  "provider": {
    "label": "Fixture Provider",
    "description": "Fixture provider"
  },
  "models": {
    "fixture_chat": {
      "label": "Fixture Chat",
      "description": "Fixture chat model"
    }
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/zh_Hans.json"),
        r#"{
  "plugin": {
    "label": "示例供应商插件",
    "description": "示例供应商插件"
  },
  "provider": {
    "label": "示例供应商",
    "description": "示例供应商"
  },
  "models": {
    "fixture_chat": {
      "label": "示例聊天模型",
      "description": "示例聊天模型"
    }
  }
}"#,
    )
    .unwrap();
    fs::write(root.join("demo/index.html"), "<html></html>").unwrap();
    fs::write(root.join("scripts/demo.sh"), "echo demo").unwrap();
}

pub(crate) fn create_capability_plugin_fixture(root: &Path) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: fixture_capability@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Fixture Capability
description: Fixture capability plugin
icon: icon.svg
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - node_contribution
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture-capability
node_contributions:
  - contribution_code: fixture_action
    node_shell: action
    category: automation
    title: Fixture Action
    description: Fixture capability node
    icon: puzzle
    schema_ui:
      sections:
        - blocks:
            - kind: field
              renderer: text
              path: config.prompt
              label: Prompt
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: external_read
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap();
    fs::write(root.join("bin/fixture-capability"), "echo fixture").unwrap();
}

pub(crate) fn create_js_dependency_pack_fixture(root: &Path, alias: &str, package: &str) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("artifacts")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"manifest_version: 1
plugin_id: fixture_js_dependency_pack@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Fixture JS Dependency Pack
description: Fixture JS dependency pack
icon: icon.svg
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - js_dependency_pack
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture-js-dependency-pack
js_dependencies:
  - alias: {alias}
    package: {package}
    version: 1.2.3
    targets:
      - backend_code
    artifacts:
      backend_code: artifacts/{alias}.backend.mjs
    integrity: sha256-{alias}
    permissions:
      network: outbound_only
      filesystem: deny
      env: deny
    native_addon: false
    lifecycle_scripts: false
"#
        ),
    )
    .unwrap();
    fs::write(root.join("bin/fixture-js-dependency-pack"), "echo fixture").unwrap();
    fs::write(
        root.join(format!("artifacts/{alias}.backend.mjs")),
        "export default {};",
    )
    .unwrap();
}

pub(crate) fn create_frontend_block_fixture(root: &Path) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("blocks/hero")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: fixture_frontend_blocks@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Fixture Frontend Blocks
description: Fixture frontend block catalog
icon: icon.svg
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture-frontend-blocks
block_contributions:
  - contribution_code: hero_banner
    title: Hero Banner
    runtime: iframe
    entry: blocks/hero/index.html
    code_template: |
      import { defineBlock } from '@1flowbase/block-sdk';
      export default defineBlock({ render() { return { primitive: 'Text' }; } });
    code_template_version: 1.0.0
    code_template_language: tsx
    code_modules:
      - source: "@1flowbase/block-sdk"
        type_declarations: "export declare function defineBlock(input: unknown): unknown;"
    context_contract:
      primitives:
        - text
        - image
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - responsive
      - configurable
"#,
    )
    .unwrap();
    fs::write(root.join("bin/fixture-frontend-blocks"), "echo fixture").unwrap();
    fs::write(root.join("blocks/hero/index.html"), "<div>hero</div>").unwrap();
}

pub(crate) fn create_provider_fixture_with_node_contribution(root: &Path) {
    create_provider_fixture(root);
    let manifest_path = root.join("manifest.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        manifest_path,
        format!(
            r#"{manifest}node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui: {{}}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: external_read
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#
        ),
    )
    .unwrap();
}

fn create_openai_compatible_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    write_provider_manifest_v2(root, "openai_compatible", "OpenAI Compatible", "0.1.0");
    fs::write(
        root.join("provider/openai_compatible.yaml"),
        r#"provider_code: openai_compatible
display_name: OpenAI Compatible
protocol: openai_compatible
help_url: https://platform.openai.com/docs/api-reference
default_base_url: https://api.openai.com/v1
model_discovery: hybrid
parameter_form:
  schema_version: 1.0.0
  title: LLM Parameters
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
"#,
    )
    .unwrap();
    write_provider_runtime_script(
        &root.join("bin/openai_compatible-provider"),
        "openai_compatible_chat",
        "OpenAI Compatible Chat",
    );
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - openai_compatible_chat\n",
    )
    .unwrap();
    fs::write(
        root.join("models/llm/openai_compatible_chat.yaml"),
        r#"model: openai_compatible_chat
label: OpenAI Compatible Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "OpenAI Compatible" } }"#,
    )
    .unwrap();
}

pub(crate) fn build_openai_compatible_package_bytes(
    version: &str,
    _include_signature: bool,
) -> Vec<u8> {
    let package_root =
        std::env::temp_dir().join(format!("official-plugin-source-{}", Uuid::now_v7()));
    create_openai_compatible_fixture(&package_root);
    write_provider_manifest_v2(
        &package_root,
        "openai_compatible",
        "OpenAI Compatible",
        version,
    );
    let bytes = pack_tar_gz(&package_root);
    let _ = fs::remove_dir_all(&package_root);
    bytes
}

pub(crate) fn build_capability_plugin_package_bytes() -> Vec<u8> {
    let package_root =
        std::env::temp_dir().join(format!("capability-plugin-source-{}", Uuid::now_v7()));
    create_capability_plugin_fixture(&package_root);
    let bytes = pack_tar_gz(&package_root);
    let _ = fs::remove_dir_all(&package_root);
    bytes
}

pub(crate) fn build_signed_openai_upload_package(version: &str) -> SignedUploadPackageFixture {
    let package_root =
        std::env::temp_dir().join(format!("uploaded-plugin-source-{}", Uuid::now_v7()));
    create_openai_compatible_fixture(&package_root);
    write_provider_manifest_v2(
        &package_root,
        "openai_compatible",
        "OpenAI Compatible",
        version,
    );

    let payload_sha256 = sha256_directory_tree(&package_root);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let public_key = plugin_framework::TrustedPublicKey {
        key_id: "official-key-2026-04".to_string(),
        algorithm: "ed25519".to_string(),
        public_key_pem: signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    };
    let release = OfficialReleaseDocument {
        schema_version: 1,
        plugin_id: format!("openai_compatible@{}", version),
        provider_code: "openai_compatible",
        version,
        contract_version: "1flowbase.provider/v2",
        artifact_sha256: "sha256:fixture-artifact",
        payload_sha256,
        signature_algorithm: "ed25519",
        signing_key_id: "official-key-2026-04",
        issued_at: "2026-04-19T15:00:00Z",
    };
    let release_bytes = serde_json::to_vec(&release).unwrap();
    let signature = signing_key.sign(&release_bytes).to_bytes();
    fs::create_dir_all(package_root.join("_meta")).unwrap();
    fs::write(
        package_root.join("_meta/official-release.json"),
        release_bytes,
    )
    .unwrap();
    fs::write(package_root.join("_meta/official-release.sig"), signature).unwrap();

    let package_bytes = pack_tar_gz(&package_root);
    let _ = fs::remove_dir_all(&package_root);

    SignedUploadPackageFixture {
        package_bytes,
        public_key,
    }
}

fn pack_tar_gz(root: &Path) -> Vec<u8> {
    let encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    append_dir_to_tar(&mut builder, root, root);
    builder.finish().unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn sha256_directory_tree(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hash_dir_recursive(root, root, &mut hasher);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_dir_recursive(root: &Path, current: &Path, hasher: &mut Sha256) {
    let mut children = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());
    for entry in children {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with("_meta/") {
            continue;
        }
        if path.is_dir() {
            hash_dir_recursive(root, &path, hasher);
            continue;
        }
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(&path).unwrap());
        hasher.update([0]);
    }
}

fn append_dir_to_tar(builder: &mut Builder<GzEncoder<Vec<u8>>>, root: &Path, current: &Path) {
    let mut children = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());
    for entry in children {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap();
        if path.is_dir() {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            builder
                .append_data(&mut header, relative, std::io::empty())
                .unwrap();
            append_dir_to_tar(builder, root, &path);
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let mut header = Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(file_mode(&path));
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, relative, Cursor::new(bytes))
            .unwrap();
    }
}

#[cfg(unix)]
fn file_mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_path: &Path) -> u32 {
    0o644
}

pub(crate) async fn seed_test_installation(
    repository: &MemoryPluginManagementRepository,
    install_root: &Path,
    provider_code: &str,
    plugin_version: &str,
    desired_state: PluginDesiredState,
) -> Uuid {
    let package_root = install_root
        .join("installed")
        .join(provider_code)
        .join(plugin_version);
    fs::create_dir_all(package_root.join("provider")).unwrap();
    fs::create_dir_all(package_root.join("bin")).unwrap();
    fs::create_dir_all(package_root.join("models/llm")).unwrap();
    fs::create_dir_all(package_root.join("i18n")).unwrap();
    write_provider_manifest_v2(
        &package_root,
        provider_code,
        "Fixture Provider",
        plugin_version,
    );
    fs::write(
        package_root.join(format!("provider/{provider_code}.yaml")),
        format!(
            "provider_code: {provider_code}\ndisplay_name: Fixture Provider\nprotocol: openai_compatible\nhelp_url: https://example.com/help\ndefault_base_url: https://api.example.com\nmodel_discovery: hybrid\nparameter_form:\n  schema_version: 1.0.0\n  title: LLM Parameters\n  fields:\n    - key: temperature\n      label: Temperature\n      type: number\n      send_mode: optional\n      enabled_by_default: true\nconfig_schema:\n  - key: base_url\n    type: string\n    required: true\n  - key: api_key\n    type: secret\n    required: true\n"
        ),
    )
    .unwrap();
    write_provider_runtime_script(
        &package_root.join(format!("bin/{provider_code}-provider")),
        "fixture_chat",
        "Fixture Chat",
    );
    fs::write(
        package_root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .unwrap();
    fs::write(
        package_root.join("models/llm/fixture_chat.yaml"),
        r#"model: fixture_chat
label: Fixture Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        package_root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    )
    .unwrap();
    let manifest_fingerprint =
        plugin_framework::compute_manifest_fingerprint(&package_root.join("manifest.yaml"))
            .await
            .unwrap();
    fs::write(
        package_root.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&json!({
            "plugin_id": format!("{provider_code}@{plugin_version}"),
            "version": plugin_version,
            "checksum": null,
            "manifest_fingerprint": manifest_fingerprint,
        }))
        .unwrap(),
    )
    .unwrap();

    let installation = repository
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: provider_code.into(),
            plugin_id: format!("{provider_code}@{plugin_version}"),
            plugin_version: plugin_version.into(),
            contract_version: "1flowbase.provider/v2".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "official_registry".into(),
            trust_level: "checksum_only".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: if matches!(desired_state, PluginDesiredState::Disabled) {
                PluginAvailabilityStatus::Disabled
            } else {
                PluginAvailabilityStatus::InstallIncomplete
            },
            package_path: None,
            installed_path: package_root.display().to_string(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({
                "help_url": "https://example.com/help",
                "default_base_url": "https://api.example.com",
                "model_discovery_mode": "hybrid",
                "supported_model_types": ["llm"],
            }),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    repository
        .upsert_plugin_package_catalog_projection(&UpsertPluginPackageCatalogProjectionInput {
            installation_id: installation.id,
            package_code: provider_code.into(),
            package_version: plugin_version.into(),
            catalog_snapshot_json: json!({
                "manifest": {
                    "icon": null,
                },
                "provider": {
                    "display_name": "Fixture Provider",
                    "help_url": "https://example.com/help",
                    "default_base_url": "https://api.example.com",
                    "model_discovery_mode": "hybrid",
                    "supports_model_fetch_without_credentials": false,
                    "form_schema": [
                        {
                            "key": "base_url",
                            "type": "string",
                            "required": true
                        },
                        {
                            "key": "api_key",
                            "type": "secret",
                            "required": true
                        }
                    ],
                    "parameter_form": {
                        "schema_version": "1.0.0",
                        "title": "LLM Parameters",
                        "fields": []
                    },
                    "predefined_models": [
                        {
                            "model_id": "fixture_chat",
                            "display_name": "Fixture Chat",
                            "source": "static",
                            "supports_streaming": true,
                            "supports_tool_call": false,
                            "supports_multimodal": false,
                            "context_window": null,
                            "max_output_tokens": null,
                            "provider_metadata": {}
                        }
                    ]
                },
                "i18n": {
                    "default_locale": "en_US",
                    "bundles": {
                        "en_US": {
                            "plugin": {
                                "label": "Fixture Provider"
                            }
                        }
                    }
                }
            }),
            projection_status: PluginPackageCatalogProjectionStatus::Ok,
            last_error_message: None,
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    installation.id
}
