use super::*;

pub(crate) fn write_test_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

pub(crate) fn write_provider_manifest_v2(
    root: &Path,
    provider_code: &str,
    display_name: &str,
    version: &str,
) {
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"manifest_version: 1
plugin_id: {provider_code}@{version}
version: {version}
publisher_namespace: taichuy
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

pub(crate) fn write_provider_runtime_script(path: &Path, model_id: &str, model_label: &str) {
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
    const lines = [
      {{ type: "text_delta", delta: "reply:" + query }},
      {{ type: "usage_snapshot", usage: {{ input_tokens: 5, output_tokens: 7, total_tokens: 12 }} }},
      {{ type: "finish", reason: "stop" }},
      {{
        type: "result",
        result: {{
          final_content: "reply:" + query,
          usage: {{ input_tokens: 5, output_tokens: 7, total_tokens: 12 }},
          finish_reason: "stop"
        }}
      }}
    ];
    process.stdout.write(lines.map((line) => JSON.stringify(line)).join("\n") + "\n");
    process.exit(0);
  }}
  default:
    result = {{}};
}}

process.stdout.write(JSON.stringify({{ ok: true, result }}));
"#
    );
    write_test_executable(path, &script);
}
fn create_official_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    write_provider_manifest_v2(root, "openai_compatible", "OpenAI Compatible", "0.2.0");
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

pub(super) fn official_upload_public_key() -> plugin_framework::TrustedPublicKey {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    plugin_framework::TrustedPublicKey {
        key_id: "official-key-2026-04".to_string(),
        algorithm: "ed25519".to_string(),
        public_key_pem: signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    }
}

pub(crate) fn build_official_provider_package(version: &str) -> Vec<u8> {
    let package_root =
        std::env::temp_dir().join(format!("official-plugin-route-package-{}", Uuid::now_v7()));
    create_official_provider_fixture(&package_root);
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

fn pack_tar_gz(root: &Path) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    append_dir_to_tar(&mut builder, root, root);
    builder.finish().unwrap();
    builder.into_inner().unwrap().finish().unwrap()
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
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
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
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(package_file_mode(&path));
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, relative, bytes.as_slice())
            .unwrap();
    }
}

#[cfg(unix)]
fn package_file_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn package_file_mode(_path: &Path) -> u32 {
    0o644
}

#[test]
fn official_provider_package_is_deterministic_across_file_timestamps() {
    let first = build_official_provider_package("0.2.0");
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let second = build_official_provider_package("0.2.0");

    assert_eq!(Sha256::digest(first), Sha256::digest(second));
}
