use super::*;

#[derive(Clone)]
pub struct ApiOfficialRuntimeExtensionSource {
    catalog: Arc<dyn OfficialExtensionCatalogSourcePort>,
    trust_mode: String,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    projection_cache: Arc<Mutex<RuntimeExtensionProjectionCache>>,
}

#[derive(Default)]
struct RuntimeExtensionProjectionCache {
    entries: HashMap<String, ProjectedRuntimeExtensionEntry>,
}

#[derive(Clone)]
struct ProjectedRuntimeExtensionEntry {
    catalog_entry: OfficialExtensionCatalogEntry,
    plugin_entry: OfficialPluginSourceEntry,
}

impl ApiOfficialRuntimeExtensionSource {
    pub fn new(
        catalog: Arc<dyn OfficialExtensionCatalogSourcePort>,
        trust_mode: String,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            catalog,
            trust_mode,
            trusted_public_keys,
            projection_cache: Arc::new(Mutex::new(RuntimeExtensionProjectionCache::default())),
        }
    }

    async fn runtime_extension_snapshot(
        &self,
        workspace_id: Option<Uuid>,
    ) -> Result<OfficialPluginCatalogSnapshot> {
        let mut cursor = None;
        let mut visited_cursors = HashSet::new();
        let mut projected_entries = HashMap::new();
        let mut entries = Vec::new();
        let mut source_kind = None;
        let mut registry_url = None;
        let mut freshness = OfficialPluginCatalogFreshness::Fresh;

        loop {
            let page = match workspace_id {
                Some(workspace_id) => {
                    self.catalog
                        .list_page_for_workspace(
                            workspace_id,
                            "runtime-extensions",
                            cursor.as_deref(),
                        )
                        .await?
                }
                None => {
                    self.catalog
                        .list_page("runtime-extensions", cursor.as_deref())
                        .await?
                }
            };
            match source_kind.as_deref() {
                Some(expected) if expected != page.source_kind => {
                    bail!("runtime extension catalog source changed while paging")
                }
                None => source_kind = Some(page.source_kind.clone()),
                _ => {}
            }
            registry_url.get_or_insert_with(|| page.metadata.locator.clone());
            if page.metadata.freshness == OfficialExtensionCatalogFreshness::Stale {
                freshness = OfficialPluginCatalogFreshness::Stale;
            }
            for catalog_entry in page.entries {
                // Client distributions share catalog category, never the runtime plugin registry.
                if catalog_entry.source.client_collector()?.is_some() {
                    continue;
                }
                let plugin_entry = project_runtime_extension_entry(
                    &*self.catalog,
                    &catalog_entry,
                    &self.trust_mode,
                )?;
                if projected_entries
                    .insert(
                        plugin_entry.plugin_id.clone(),
                        ProjectedRuntimeExtensionEntry {
                            catalog_entry,
                            plugin_entry: plugin_entry.clone(),
                        },
                    )
                    .is_some()
                {
                    bail!(
                        "runtime extension catalog contains duplicate plugin_id: {}",
                        plugin_entry.plugin_id
                    );
                }
                entries.push(plugin_entry);
            }
            let Some(next_cursor) = page.metadata.next_cursor else {
                break;
            };
            if !visited_cursors.insert(next_cursor.clone()) {
                bail!("runtime extension catalog contains a cursor cycle");
            }
            cursor = Some(next_cursor);
        }

        let source_kind = source_kind.unwrap_or_else(|| "official_repository".to_string());
        let snapshot = OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_label: if source_kind == "official_repository" {
                    "Official source".to_string()
                } else {
                    "Mirror source".to_string()
                },
                source_kind,
                registry_url: registry_url.unwrap_or_default(),
            },
            freshness,
            entries,
        };
        let mut cache = self
            .projection_cache
            .lock()
            .map_err(|_| anyhow!("runtime extension projection cache is poisoned"))?;
        cache.entries = projected_entries;
        Ok(snapshot)
    }
}

#[async_trait]
impl OfficialPluginSourcePort for ApiOfficialRuntimeExtensionSource {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        self.runtime_extension_snapshot(None).await
    }

    async fn list_official_catalog_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<OfficialPluginCatalogSnapshot> {
        self.runtime_extension_snapshot(Some(workspace_id)).await
    }

    async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
        let cached = self
            .catalog
            .cached_verified_entries("runtime-extensions", Some("model_provider"))?;
        let entries = cached
            .entries
            .iter()
            .map(|entry| project_runtime_extension_entry(&*self.catalog, entry, &self.trust_mode))
            .collect::<Result<Vec<_>>>()
            .ok()?;
        Some(OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_label: "Verified catalog snapshot".to_string(),
                source_kind: cached.source_kind,
                registry_url: cached.snapshot_locator,
            },
            freshness: OfficialPluginCatalogFreshness::Stale,
            entries,
        })
    }

    async fn download_plugin(
        &self,
        entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        let catalog_entry = {
            let cache = self
                .projection_cache
                .lock()
                .map_err(|_| anyhow!("runtime extension projection cache is poisoned"))?;
            let projected = cache.entries.get(&entry.plugin_id).ok_or_else(|| {
                anyhow!("runtime extension entry was not projected by the catalog")
            })?;
            if projected.plugin_entry != *entry {
                bail!("runtime extension entry does not match its catalog projection");
            }
            projected.catalog_entry.clone()
        };
        let downloaded = self.catalog.download_artifact(&catalog_entry).await?;
        Ok(DownloadedOfficialPluginPackage {
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
        })
    }

    async fn download_plugin_for_workspace(
        &self,
        workspace_id: Uuid,
        entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        let catalog_entry = {
            let cache = self
                .projection_cache
                .lock()
                .map_err(|_| anyhow!("runtime extension projection cache is poisoned"))?;
            let projected = cache.entries.get(&entry.plugin_id).ok_or_else(|| {
                anyhow!("runtime extension entry was not projected by the catalog")
            })?;
            if projected.plugin_entry != *entry {
                bail!("runtime extension entry does not match its catalog projection");
            }
            projected.catalog_entry.clone()
        };
        let downloaded = self
            .catalog
            .download_artifact_for_workspace(workspace_id, &catalog_entry)
            .await?;
        Ok(DownloadedOfficialPluginPackage {
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
        })
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        self.trusted_public_keys.clone()
    }
}

fn project_runtime_extension_entry(
    catalog: &dyn OfficialExtensionCatalogSourcePort,
    entry: &OfficialExtensionCatalogEntry,
    trust_mode: &str,
) -> Result<OfficialPluginSourceEntry> {
    if entry.category != "runtime-extensions" {
        bail!("runtime extension catalog returned an entry from another category");
    }
    let plugin_id = required_runtime_extension_metadata(entry, "plugin_id")?;
    let plugin_type = required_runtime_extension_metadata(entry, "plugin_type")?;
    let provider_code = required_runtime_extension_metadata(entry, "provider_code")?;
    let protocol = required_runtime_extension_metadata(entry, "protocol")?;
    let model_discovery_mode = required_runtime_extension_metadata(entry, "model_discovery_mode")?;
    let descriptor = catalog.resolve_artifact(entry)?;
    let platform = descriptor.platform.unwrap_or_else(|| {
        let host = plugin_framework::RuntimeTarget::current_host().unwrap_or_else(|_| {
            plugin_framework::RuntimeTarget::from_rust_target_triple("x86_64-unknown-linux-musl")
                .expect("fallback runtime extension target must be supported")
        });
        OfficialExtensionArtifactPlatform {
            os: host.os,
            arch: host.arch,
            libc: host.libc,
            rust_target: host.rust_target_triple,
        }
    });
    let checksum = descriptor
        .expected_checksum
        .ok_or_else(|| anyhow!("runtime extension catalog entry is missing checksum"))?;
    let signature_algorithm = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("algorithm"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let signing_key_id = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let i18n_summary = runtime_extension_i18n_summary(entry)?;
    Ok(OfficialPluginSourceEntry {
        plugin_id,
        plugin_type,
        provider_code: provider_code.clone(),
        namespace: optional_runtime_extension_metadata(entry, "namespace")
            .unwrap_or_else(|| format!("plugin.{provider_code}")),
        protocol,
        latest_version: entry.version.clone(),
        minimum_host_version: entry.host_version_requirement.clone(),
        icon: optional_runtime_extension_metadata(entry, "icon"),
        selected_artifact: OfficialPluginArtifact {
            os: platform.os,
            arch: platform.arch,
            libc: platform.libc,
            rust_target: platform.rust_target,
            download_url: descriptor.locator,
            checksum,
            signature_algorithm,
            signing_key_id,
        },
        i18n_summary,
        release_tag: optional_runtime_extension_metadata(entry, "release_tag")
            .unwrap_or_else(|| format!("{provider_code}-v{}", entry.version)),
        trust_mode: trust_mode.to_string(),
        help_url: optional_runtime_extension_metadata(entry, "help_url"),
        model_discovery_mode,
    })
}

fn required_runtime_extension_metadata(
    entry: &OfficialExtensionCatalogEntry,
    field: &'static str,
) -> Result<String> {
    optional_runtime_extension_metadata(entry, field)
        .ok_or_else(|| anyhow!("runtime extension catalog entry is missing {field}"))
}

fn optional_runtime_extension_metadata(
    entry: &OfficialExtensionCatalogEntry,
    field: &str,
) -> Option<String> {
    entry
        .source
        .metadata
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_extension_i18n_summary(
    entry: &OfficialExtensionCatalogEntry,
) -> Result<OfficialPluginI18nSummary> {
    let value = entry.source.metadata.get("i18n_summary");
    let default_locale = value
        .and_then(|value| value.get("default_locale"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en_US")
        .to_string();
    let mut bundles = value
        .and_then(|value| value.get("bundles"))
        .cloned()
        .map(serde_json::from_value::<BTreeMap<String, Value>>)
        .transpose()
        .context("runtime extension catalog i18n bundles must be an object")?
        .unwrap_or_default();
    if bundles.is_empty() {
        bundles.insert(
            default_locale.clone(),
            serde_json::json!({
                "plugin": { "label": entry.name },
                "provider": { "label": entry.name },
            }),
        );
    }
    let available_locales = value
        .and_then(|value| value.get("available_locales"))
        .cloned()
        .map(serde_json::from_value::<Vec<String>>)
        .transpose()
        .context("runtime extension catalog available_locales must be an array")?
        .filter(|locales| !locales.is_empty())
        .unwrap_or_else(|| bundles.keys().cloned().collect());
    Ok(OfficialPluginI18nSummary {
        default_locale,
        available_locales,
        bundles,
    })
}

