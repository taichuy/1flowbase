use super::fixtures::build_openai_compatible_package_bytes;
use super::*;

#[derive(Clone, Default)]
pub(crate) struct MemoryProviderRuntime {
    loaded_installations: Arc<RwLock<Vec<Uuid>>>,
}

impl MemoryProviderRuntime {
    pub(crate) async fn loaded_installations(&self) -> Vec<Uuid> {
        self.loaded_installations.read().await.clone()
    }
}

#[derive(Clone)]
pub(crate) struct MemoryOfficialPluginSource {
    source_kind: String,
    source_label: String,
    trust_mode: String,
    include_signature: bool,
    minimum_host_version: String,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
}

impl Default for MemoryOfficialPluginSource {
    fn default() -> Self {
        Self {
            source_kind: "official_registry".to_string(),
            source_label: "官方源".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            include_signature: false,
            minimum_host_version: "0.1.0".to_string(),
            trusted_public_keys: Vec::new(),
        }
    }
}

pub(crate) fn sample_artifact(os: &str, arch: &str, libc: Option<&str>) -> OfficialPluginArtifact {
    OfficialPluginArtifact {
        os: os.into(),
        arch: arch.into(),
        libc: libc.map(str::to_string),
        rust_target: "x86_64-unknown-linux-musl".into(),
        download_url: "https://example.test/openai_compatible.1flowbasepkg".into(),
        checksum: format!("sha256:{}", "a".repeat(64)),
        signature_algorithm: Some("ed25519".into()),
        signing_key_id: Some("official-key".into()),
    }
}

pub(crate) fn sample_i18n_summary() -> OfficialPluginI18nSummary {
    OfficialPluginI18nSummary {
        default_locale: "en_US".into(),
        available_locales: vec!["en_US".into(), "zh_Hans".into()],
        bundles: BTreeMap::from([
            (
                "en_US".into(),
                serde_json::json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
            ),
            (
                "zh_Hans".into(),
                serde_json::json!({ "plugin": { "label": "OpenAI-Compatible API Provider" } }),
            ),
        ]),
    }
}

pub(crate) fn requested_locales() -> RequestedLocales {
    RequestedLocales::new("en_US", "en_US")
}

impl MemoryOfficialPluginSource {
    pub(crate) fn unsigned_required() -> Self {
        Self {
            trust_mode: "signature_required".to_string(),
            ..Self::default()
        }
    }

    pub(crate) fn with_trusted_public_keys(
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            trusted_public_keys,
            ..Self::default()
        }
    }

    pub(crate) fn with_minimum_host_version(minimum_host_version: impl Into<String>) -> Self {
        Self {
            minimum_host_version: minimum_host_version.into(),
            ..Self::default()
        }
    }
}

#[async_trait]
impl OfficialPluginSourcePort for MemoryOfficialPluginSource {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        let package_bytes = build_openai_compatible_package_bytes("0.1.0", self.include_signature);
        Ok(OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_kind: self.source_kind.clone(),
                source_label: self.source_label.clone(),
                registry_url: "https://example.com/official-registry.json".to_string(),
            },
            freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
            entries: vec![OfficialPluginSourceEntry {
                plugin_id: "1flowbase.openai_compatible".to_string(),
                plugin_type: "model_provider".to_string(),
                provider_code: "openai_compatible".to_string(),
                namespace: "plugin.openai_compatible".to_string(),
                protocol: "openai_compatible".to_string(),
                latest_version: "0.1.0".to_string(),
                minimum_host_version: self.minimum_host_version.clone(),
                icon: None,
                selected_artifact: OfficialPluginArtifact {
                    checksum: format!("sha256:{:x}", Sha256::digest(&package_bytes)),
                    ..sample_artifact("linux", "amd64", Some("musl"))
                },
                i18n_summary: sample_i18n_summary(),
                release_tag: "openai_compatible-v0.1.0".to_string(),
                trust_mode: self.trust_mode.clone(),
                help_url: Some(
                    "https://github.com/taichuy/1flowbase-official-plugins/tree/main/models/openai_compatible"
                        .to_string(),
                ),
                model_discovery_mode: "hybrid".to_string(),
            }],
        })
    }

    async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
        self.list_official_catalog().await.ok()
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        Ok(DownloadedOfficialPluginPackage {
            file_name: "openai_compatible-0.1.0.1flowbasepkg".to_string(),
            package_bytes: build_openai_compatible_package_bytes("0.1.0", self.include_signature),
        })
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        self.trusted_public_keys.clone()
    }
}

#[async_trait]
impl ProviderRuntimePort for MemoryProviderRuntime {
    async fn ensure_loaded(&self, installation: &PluginInstallationRecord) -> Result<()> {
        if !Path::new(&installation.installed_path).is_dir() {
            return Err(ControlPlaneError::NotFound("provider_install_path").into());
        }
        self.loaded_installations
            .write()
            .await
            .push(installation.id);
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        Ok(vec![ProviderModelDescriptor {
            model_id: "fixture_chat".to_string(),
            display_name: "Fixture Chat".to_string(),
            source: plugin_framework::provider_contract::ProviderModelSource::Dynamic,
            supports_streaming: true,
            supports_tool_call: false,
            supports_multimodal: false,
            context_window: Some(128000),
            max_output_tokens: Some(4096),
            provider_metadata: json!({}),
        }])
    }

    async fn invoke_stream(
        &self,
        _installation: &PluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderRuntimeInvocationOutput> {
        Ok(ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult::default(),
        })
    }
}
