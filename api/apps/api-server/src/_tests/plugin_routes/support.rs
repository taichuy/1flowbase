use std::{fs, path::Path};

use crate::_tests::support::{write_provider_manifest_v2, write_provider_runtime_script};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use ed25519_dalek::{Signer, SigningKey};
use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tar::Builder;
use tower::ServiceExt;

pub(super) async fn create_member(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) -> String {
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": password,
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

pub(super) async fn create_role(app: &axum::Router, cookie: &str, csrf: &str, role_code: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": role_code,
                        "name": role_code,
                        "introduction": role_code
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

pub(super) async fn replace_role_permissions(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    role_code: &str,
    permission_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/roles/{role_code}/permissions"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "permission_codes": permission_codes }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

pub(super) async fn replace_member_roles(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    member_id: &str,
    role_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "role_codes": role_codes }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

pub(super) fn create_fixture_provider_package(root: &Path, version: &str) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    write_provider_manifest_v2(root, "fixture_provider", "Fixture Provider", version);
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
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    )
    .unwrap();
    fs::write(root.join("demo/index.html"), "<html></html>").unwrap();
    fs::write(root.join("scripts/demo.sh"), "echo demo").unwrap();
}

pub(super) fn create_openai_compatible_package(root: &Path, version: &str) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    write_provider_manifest_v2(root, "openai_compatible", "OpenAI Compatible", version);
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

pub(super) fn create_host_extension_package(root: &Path, version: &str) {
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

pub(super) fn build_signed_openai_upload_package(version: &str) -> Vec<u8> {
    let package_root = std::env::temp_dir().join(format!(
        "plugin-route-upload-openai-{}",
        uuid::Uuid::now_v7()
    ));
    create_openai_compatible_package(&package_root, version);

    let payload_sha256 = sha256_directory_tree(&package_root);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let release = serde_json::json!({
        "schema_version": 1,
        "plugin_id": format!("openai_compatible@{}", version),
        "provider_code": "openai_compatible",
        "version": version,
        "contract_version": "1flowbase.provider/v1",
        "artifact_sha256": "sha256:fixture-artifact",
        "payload_sha256": payload_sha256,
        "signature_algorithm": "ed25519",
        "signing_key_id": "official-key-2026-04",
        "issued_at": "2026-04-19T15:00:00Z"
    });
    let release_bytes = serde_json::to_vec(&release).unwrap();
    let signature = signing_key.sign(&release_bytes).to_bytes();
    fs::create_dir_all(package_root.join("_meta")).unwrap();
    fs::write(
        package_root.join("_meta/official-release.json"),
        release_bytes,
    )
    .unwrap();
    fs::write(package_root.join("_meta/official-release.sig"), signature).unwrap();

    let bytes = pack_tar_gz(&package_root);
    let _ = fs::remove_dir_all(&package_root);
    bytes
}

pub(super) fn build_upload_body(boundary: &str, file_name: &str, package_bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(package_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

pub(super) fn pack_tar_gz(root: &Path) -> Vec<u8> {
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
            builder.append_dir(relative, &path).unwrap();
            append_dir_to_tar(builder, root, &path);
            continue;
        }
        builder.append_path_with_name(&path, relative).unwrap();
    }
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
