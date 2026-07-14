use axum::{
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    Json, Router,
};
use control_plane::ports::OfficialPluginSourcePort;
use plugin_framework::RuntimeTarget;
use serde_json::json;

use crate::config::ResolvedOfficialPluginSourceConfig;
use crate::official_plugin_registry::{
    rewrite_github_raw_url, select_artifact_for_host, ApiOfficialPluginRegistry,
    OfficialRegistryArtifact, OfficialRegistryEntry, OfficialRegistryI18nSummary,
};

const RAW_REGISTRY_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/official-registry.json";
const RAW_ICON_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/runtime-extensions/model-providers/openai_compatible/_assets/icon.svg";
const RAW_PACKAGE_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/dist/openai-compatible.1flowbasepkg";

#[test]
fn select_artifact_prefers_exact_linux_match() {
    let host = RuntimeTarget {
        rust_target_triple: "x86_64-unknown-linux-gnu".into(),
        os: "linux".into(),
        arch: "amd64".into(),
        libc: Some("gnu".into()),
    };
    let entry = OfficialRegistryEntry {
        plugin_id: "1flowbase.openai_compatible".into(),
        plugin_type: "model_provider".into(),
        provider_code: "openai_compatible".into(),
        display_name: "OpenAI-Compatible API Provider".into(),
        protocol: "openai_compatible".into(),
        latest_version: "0.2.1".into(),
        minimum_host_version: "0.1.0".into(),
        icon: None,
        help_url: None,
        model_discovery_mode: "hybrid".into(),
        i18n_summary: OfficialRegistryI18nSummary {
            default_locale: "en_US".into(),
            available_locales: vec!["en_US".into(), "zh_Hans".into()],
            bundles: std::collections::BTreeMap::from([
                (
                    "en_US".into(),
                    json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
                ),
                (
                    "zh_Hans".into(),
                    json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
                ),
            ]),
        },
        artifacts: vec![
            OfficialRegistryArtifact {
                os: "linux".into(),
                arch: "amd64".into(),
                libc: Some("musl".into()),
                rust_target: "x86_64-unknown-linux-musl".into(),
                download_url: "https://example.com/linux-amd64.1flowbasepkg".into(),
                checksum: "sha256:linux-amd64".into(),
                signature_algorithm: Some("ed25519".into()),
                signing_key_id: Some("official-key-2026-04".into()),
            },
            OfficialRegistryArtifact {
                os: "linux".into(),
                arch: "arm64".into(),
                libc: Some("musl".into()),
                rust_target: "aarch64-unknown-linux-musl".into(),
                download_url: "https://example.com/linux-arm64.1flowbasepkg".into(),
                checksum: "sha256:linux-arm64".into(),
                signature_algorithm: Some("ed25519".into()),
                signing_key_id: Some("official-key-2026-04".into()),
            },
        ],
    };

    let selected = select_artifact_for_host(&entry, &host).unwrap();
    assert_eq!(
        selected.download_url,
        "https://example.com/linux-amd64.1flowbasepkg"
    );
}

#[test]
fn select_artifact_returns_none_when_no_platform_matches() {
    let host = RuntimeTarget {
        rust_target_triple: "aarch64-apple-darwin".into(),
        os: "macos".into(),
        arch: "arm64".into(),
        libc: None,
    };
    let entry = OfficialRegistryEntry {
        plugin_id: "1flowbase.openai_compatible".into(),
        plugin_type: "model_provider".into(),
        provider_code: "openai_compatible".into(),
        display_name: "OpenAI-Compatible API Provider".into(),
        protocol: "openai_compatible".into(),
        latest_version: "0.2.1".into(),
        minimum_host_version: "0.1.0".into(),
        icon: None,
        help_url: None,
        model_discovery_mode: "hybrid".into(),
        i18n_summary: OfficialRegistryI18nSummary {
            default_locale: "en_US".into(),
            available_locales: vec!["en_US".into(), "zh_Hans".into()],
            bundles: std::collections::BTreeMap::from([
                (
                    "en_US".into(),
                    json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
                ),
                (
                    "zh_Hans".into(),
                    json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
                ),
            ]),
        },
        artifacts: vec![OfficialRegistryArtifact {
            os: "linux".into(),
            arch: "amd64".into(),
            libc: Some("musl".into()),
            rust_target: "x86_64-unknown-linux-musl".into(),
            download_url: "https://example.com/linux-amd64.1flowbasepkg".into(),
            checksum: "sha256:linux-amd64".into(),
            signature_algorithm: None,
            signing_key_id: None,
        }],
    };

    assert!(select_artifact_for_host(&entry, &host).is_none());
}

#[test]
fn rewrite_github_raw_url_adds_proxy_prefix_for_raw_githubusercontent() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg",
            Some("https://gh-proxy.com/")
        ),
        "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg"
    );
}

#[test]
fn rewrite_github_raw_url_normalizes_proxy_prefix_without_trailing_slash() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg",
            Some("https://gh-proxy.com")
        ),
        "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg"
    );
}

#[test]
fn rewrite_github_raw_url_keeps_non_raw_urls_unchanged() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://example.com/openai-compatible.1flowbasepkg",
            Some("https://gh-proxy.com/")
        ),
        "https://example.com/openai-compatible.1flowbasepkg"
    );
}

#[test]
fn rewrite_github_raw_url_does_not_duplicate_existing_proxy_prefix() {
    let proxied = "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg";

    assert_eq!(
        rewrite_github_raw_url(proxied, Some("https://gh-proxy.com/")),
        proxied
    );
}

#[tokio::test]
async fn api_official_plugin_registry_uses_proxy_for_catalog_urls_and_package_download() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}/proxy", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(proxy_response))
            .await
            .unwrap();
    });
    let registry = ApiOfficialPluginRegistry::new(
        ResolvedOfficialPluginSourceConfig {
            source_kind: "official_registry".to_string(),
            source_label: "官方源".to_string(),
            registry_url: RAW_REGISTRY_URL.to_string(),
            github_proxy_url: Some(proxy_url.clone()),
            trust_mode: "allow_unsigned".to_string(),
        },
        Vec::new(),
    );

    let catalog = registry.list_official_catalog().await.unwrap();
    let entry = catalog.entries.first().unwrap();
    let proxied_registry_url = format!("{proxy_url}/{RAW_REGISTRY_URL}");
    let proxied_icon_url = format!("{proxy_url}/{RAW_ICON_URL}");
    let proxied_package_url = format!("{proxy_url}/{RAW_PACKAGE_URL}");

    assert_eq!(catalog.source.registry_url, proxied_registry_url);
    assert_eq!(entry.icon.as_deref(), Some(proxied_icon_url.as_str()));
    assert_eq!(entry.selected_artifact.download_url, proxied_package_url);
    assert_eq!(entry.trust_mode, "allow_unsigned");

    let downloaded = registry.download_plugin(entry).await.unwrap();
    assert_eq!(downloaded.package_bytes, b"package-via-proxy");

    server.abort();
}

async fn proxy_response(uri: Uri) -> Response {
    let path = uri.path();
    if path.ends_with("official-registry.json") {
        return Json(json!({
            "version": 1,
            "plugins": [{
                "plugin_id": "1flowbase.openai_compatible",
                "plugin_type": "model_provider",
                "provider_code": "openai_compatible",
                "display_name": "OpenAI Compatible",
                "protocol": "openai_compatible",
                "latest_version": "0.2.0",
                "icon": RAW_ICON_URL,
                "help_url": null,
                "model_discovery_mode": "hybrid",
                "artifacts": [{
                    "os": "linux",
                    "arch": "amd64",
                    "rust_target": "x86_64-unknown-linux-musl",
                    "download_url": RAW_PACKAGE_URL,
                    "checksum": "sha256:test-package",
                    "signature_algorithm": null,
                    "signing_key_id": null
                }]
            }]
        }))
        .into_response();
    }
    if path.ends_with("openai-compatible.1flowbasepkg") {
        return b"package-via-proxy".to_vec().into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
