use super::packages::{build_official_provider_package, official_upload_public_key};
use super::*;

#[derive(Clone, Default)]
pub(super) struct InMemoryOfficialPluginSource;

#[derive(Clone, Default)]
pub(super) struct InMemoryOfficialMcpBundleSource;

#[derive(Clone, Default)]
pub(super) struct InMemoryOfficialExtensionCatalogSource;

fn runtime_extension_catalog_entry() -> OfficialExtensionCatalogEntry {
    OfficialExtensionCatalogEntry {
        id: "runtime-extensions:taichuy/openai_compatible".to_string(),
        name: "OpenAI Compatible".to_string(),
        category: "runtime-extensions".to_string(),
        organization: "taichuy".to_string(),
        artifact: "openai_compatible".to_string(),
        version: "0.2.0".to_string(),
        description: "Official provider plugin".to_string(),
        host_version_requirement: ">=0.1.0".to_string(),
        source: OfficialExtensionCatalogEntrySource {
            kind: "runtime_extension_manifest".to_string(),
            locator: "runtime-extensions/@taichuy/openai_compatible/manifest.yaml".to_string(),
            metadata: std::collections::BTreeMap::from([
                (
                    "plugin_id".to_string(),
                    json!("1flowbase.openai_compatible"),
                ),
                ("plugin_type".to_string(), json!("model_provider")),
                ("provider_code".to_string(), json!("openai_compatible")),
                ("protocol".to_string(), json!("openai_compatible")),
                ("model_discovery_mode".to_string(), json!("hybrid")),
            ]),
        },
        signature: None,
        checksum: None,
        download_locator: json!({ "kind": "platform_release_assets" }),
        catalog_page: 1,
    }
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for InMemoryOfficialExtensionCatalogSource {
    async fn list_page(
        &self,
        category: &str,
        _cursor: Option<&str>,
    ) -> anyhow::Result<OfficialExtensionCatalogPage> {
        anyhow::ensure!(
            category == "runtime-extensions",
            "unexpected catalog category"
        );
        Ok(OfficialExtensionCatalogPage {
            source_kind: "official_repository".to_string(),
            category: category.to_string(),
            metadata: OfficialExtensionCatalogPageMetadata {
                page: 1,
                cursor: "start".to_string(),
                checksum: "sha256:test-runtime-extensions".to_string(),
                locator: "https://example.test/runtime-extensions/catalog/v1/pages/1.json"
                    .to_string(),
                next_cursor: None,
                page_size: 100,
                total_entries: 1,
                freshness: OfficialExtensionCatalogFreshness::Fresh,
            },
            entries: vec![runtime_extension_catalog_entry()],
        })
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> anyhow::Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        let entry = runtime_extension_catalog_entry();
        Ok(
            (category == entry.category && catalog_id == entry.id).then_some(
                LocatedOfficialExtensionCatalogEntry {
                    source_kind: "official_repository".to_string(),
                    entry,
                },
            ),
        )
    }

    fn resolve_artifact(
        &self,
        _entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<OfficialExtensionArtifactDescriptor> {
        let bytes = build_official_provider_package("0.2.0");
        Ok(OfficialExtensionArtifactDescriptor {
            locator_kind: "platform_release_asset".to_string(),
            locator: "https://example.test/openai_compatible-0.2.0.1flowbasepkg".to_string(),
            expected_checksum: Some(format!("sha256:{:x}", Sha256::digest(&bytes))),
            signature: None,
            platform: Some(OfficialExtensionArtifactPlatform {
                os: "linux".to_string(),
                arch: "amd64".to_string(),
                libc: Some("musl".to_string()),
                rust_target: "x86_64-unknown-linux-musl".to_string(),
            }),
        })
    }

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<DownloadedOfficialExtensionArtifact> {
        Ok(DownloadedOfficialExtensionArtifact {
            descriptor: self.resolve_artifact(entry)?,
            file_name: "openai_compatible-0.2.0.1flowbasepkg".to_string(),
            artifact_bytes: build_official_provider_package("0.2.0"),
        })
    }
}

#[async_trait]
impl OfficialMcpBundleSourcePort for InMemoryOfficialMcpBundleSource {
    async fn library_catalog(&self) -> anyhow::Result<McpBundleLibraryCatalog> {
        Ok(McpBundleLibraryCatalog {
            source: OfficialMcpBundleCatalogSource {
                source_kind: "official_registry".into(),
                source_label: "官方源".into(),
                catalog_url: "https://example.com/mcp/catalog.json".into(),
            },
            remote_available: true,
            remote_error: None,
            bundles: vec![McpBundleLibraryEntry {
                organization: "taichuy".into(),
                bundle_id: "test_bundle".into(),
                source_path: Some("mcp/@taichuy/test_bundle".into()),
                remote_versions: Vec::new(),
                local_versions: Vec::new(),
                current_bundle_version: None,
            }],
        })
    }

    async fn list_catalog(&self) -> anyhow::Result<OfficialMcpBundleCatalogSnapshot> {
        Ok(OfficialMcpBundleCatalogSnapshot {
            source: OfficialMcpBundleCatalogSource {
                source_kind: "official_registry".into(),
                source_label: "官方源".into(),
                catalog_url: "https://example.com/mcp/catalog.json".into(),
            },
            entries: vec![OfficialMcpBundleCatalogEntry {
                organization: "taichuy".into(),
                bundle_id: "test_bundle".into(),
                latest_version: "1.0.0".into(),
                locale: "zh_Hans".into(),
                minimum_host_version: "0.2.0".into(),
                exported_from_system_version: "0.1.0".into(),
                release_tag: "mcp-taichuy-test_bundle-v1.0.0".into(),
                download_url: "https://example.com/test-bundle.zip".into(),
                artifact_sha256: None,
            }],
        })
    }

    async fn download_bundle(
        &self,
        organization: &str,
        bundle_id: &str,
    ) -> anyhow::Result<DownloadedOfficialMcpBundle> {
        if organization != "taichuy" || bundle_id != "test_bundle" {
            return Err(anyhow::anyhow!("official MCP bundle not found"));
        }
        Ok(DownloadedOfficialMcpBundle {
            file_name: "taichuy-test_bundle-v1.0.0.zip".into(),
            package_bytes: build_official_mcp_bundle(),
        })
    }
}

fn build_official_mcp_bundle() -> Vec<u8> {
    let tool = serde_json::to_vec(&json!({
        "tool_id": "official_runtime_profile",
        "name": "Runtime profile",
        "short_description": "Runtime profile",
        "full_description": "Read runtime profile",
        "interface_id": "get_runtime_profile",
        "parameter_schema_snapshot": {},
        "result_schema_snapshot": {},
        "input_mapping": {},
        "output_mapping": {},
        "permission_code_snapshot": null,
        "risk_level_snapshot": "low",
        "status": "enabled"
    }))
    .unwrap();
    let manifest = serde_json::to_vec(&json!({
        "schema_version": "1flowbase.mcp.bundle/v1",
        "organization": "taichuy",
        "bundle_id": "test_bundle",
        "bundle_version": "1.0.0",
        "locale": "zh_Hans",
        "minimum_host_version": "0.2.0",
        "exported_from_system_version": "0.1.0",
        "exported_at": "2026-07-13T10:00:00Z",
        "files": [{
            "path": "tools/runtime-profile.json",
            "kind": "tool",
            "sha256": format!("sha256:{:x}", Sha256::digest(&tool))
        }]
    }))
    .unwrap();
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    archive.start_file("manifest.json", options).unwrap();
    archive.write_all(&manifest).unwrap();
    archive
        .start_file("tools/runtime-profile.json", options)
        .unwrap();
    archive.write_all(&tool).unwrap();
    archive.finish().unwrap().into_inner()
}

#[async_trait]
impl OfficialPluginSourcePort for InMemoryOfficialPluginSource {
    async fn list_official_catalog(&self) -> anyhow::Result<OfficialPluginCatalogSnapshot> {
        let package_bytes = build_official_provider_package("0.2.0");
        Ok(OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_kind: "mirror_registry".to_string(),
                source_label: "镜像源".to_string(),
                registry_url: "https://mirror.example.com/official-registry.json".to_string(),
            },
            freshness: control_plane::ports::OfficialPluginCatalogFreshness::Fresh,
            entries: vec![OfficialPluginSourceEntry {
                plugin_id: "1flowbase.openai_compatible".to_string(),
                plugin_type: "model_provider".to_string(),
                provider_code: "openai_compatible".to_string(),
                namespace: "plugin.openai_compatible".to_string(),
                protocol: "openai_compatible".to_string(),
                latest_version: "0.2.0".to_string(),
                minimum_host_version: "0.1.0".to_string(),
                icon: Some(
                    "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/runtime-extensions/model-providers/openai_compatible/_assets/icon.svg"
                        .to_string(),
                ),
                selected_artifact: OfficialPluginArtifact {
                    os: "linux".to_string(),
                    arch: "amd64".to_string(),
                    libc: Some("musl".to_string()),
                    rust_target: "x86_64-unknown-linux-musl".to_string(),
                    download_url: "https://example.com/openai-compatible.1flowbasepkg"
                        .to_string(),
                    checksum: format!("sha256:{:x}", Sha256::digest(&package_bytes)),
                    signature_algorithm: None,
                    signing_key_id: None,
                },
                i18n_summary: OfficialPluginI18nSummary {
                    default_locale: "en_US".to_string(),
                    available_locales: vec!["en_US".to_string(), "zh_Hans".to_string()],
                    bundles: std::collections::BTreeMap::from([
                        (
                            "en_US".to_string(),
                            json!({
                                "plugin": {
                                    "label": "OpenAI Compatible",
                                    "description": "Official provider plugin"
                                },
                                "provider": {
                                    "label": "OpenAI Compatible"
                                }
                            }),
                        ),
                        (
                            "zh_Hans".to_string(),
                            json!({
                                "plugin": {
                                    "label": "OpenAI Compatible",
                                    "description": "官方 Provider 插件"
                                },
                                "provider": {
                                    "label": "OpenAI Compatible"
                                }
                            }),
                        ),
                    ]),
                },
                release_tag: "openai_compatible-v0.2.0".to_string(),
                trust_mode: "allow_unsigned".to_string(),
                help_url: Some(
                    "https://github.com/taichuy/1flowbase-official-plugins/tree/main/models/openai_compatible"
                        .to_string(),
                ),
                model_discovery_mode: "hybrid".to_string(),
            }],
        })
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage> {
        Ok(DownloadedOfficialPluginPackage {
            file_name: "openai_compatible-0.2.0.1flowbasepkg".to_string(),
            package_bytes: build_official_provider_package("0.2.0"),
        })
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        vec![official_upload_public_key()]
    }
}
