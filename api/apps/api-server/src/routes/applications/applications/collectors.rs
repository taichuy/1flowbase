use super::ApplicationCollectorResponse;
use crate::{
    error_response::ApiError,
    official_extension_catalog::{
        OfficialExtensionCatalogSearchQuery, OfficialExtensionCatalogSourcePort,
    },
};
use control_plane::{
    plugin_management::ExtensionInstallationService,
    ports::{ExtensionInstallationRepository, RoleConsolePolicyReader},
};
use std::{collections::BTreeMap, sync::Arc};
use storage_durable_postgres::MainDurableStore;

pub(crate) struct CollectorCatalogDependencies {
    pub source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub install_root: String,
    pub node_id: String,
}

pub(crate) async fn catalog(
    store: &MainDurableStore,
    dependencies: &CollectorCatalogDependencies,
    actor: &domain::ActorContext,
    locale: &domain::CatalogLocale,
) -> Result<Vec<ApplicationCollectorResponse>, ApiError> {
    let policies = store.load_role_console_policies_for_user(actor).await?;
    let group = domain::ConsolePolicyGroup::settings_feature("system.extension-center")
        .expect("static feature");
    let allowed = |operation: &str| {
        actor.is_root
            || domain::effective_console_simple_operation(
                &policies,
                &group,
                &domain::ConsoleOperationId::try_from(operation).expect("static operation"),
            )
    };
    let can_install = allowed("extension_center.install");
    let can_update = allowed("extension_center.update");
    let service = ExtensionInstallationService::new(store.clone(), &dependencies.install_root);
    let mut entries: BTreeMap<String, ApplicationCollectorResponse> = BTreeMap::new();
    // Read node-scoped records including missing packages, so offline/missing states remain honest.
    let records = store
        .list_extension_installations_for_node(&dependencies.node_id)
        .await?;
    let mut local = BTreeMap::new();
    for record in records {
        if !domain::is_client_collector_receipt(&record.receipt) {
            continue;
        }
        let id = record.identity.catalog_id();
        let replace =
            local
                .get(&id)
                .is_none_or(|previous: &domain::ExtensionInstallationRecord| {
                    (record.is_current, record.updated_at)
                        > (previous.is_current, previous.updated_at)
                });
        if replace {
            local.insert(id, record);
        }
    }
    for (id, record) in local {
        let retained = serde_json::from_value::<domain::ClientCollectorManifest>(
            record.receipt["client_collector"].clone(),
        );
        let Ok(retained) = retained else {
            continue;
        };
        let valid = service.client_collector_manifest(&record).await.ok();
        let manifest = valid.as_ref().unwrap_or(&retained);
        let base = valid.as_ref().map(|_| {
            format!(
                "/api/public/client-collectors/{}/{}/{}/assets",
                record.identity.organization, record.identity.artifact_id, record.identity.version
            )
        });
        let asset_url = |name: &str| {
            base.as_ref()
                .filter(|_| manifest.assets.iter().any(|asset| asset.name == name))
                .map(|base| format!("{base}/{name}"))
        };
        let readme = if locale.as_str() == "zh_Hans" {
            "README.md"
        } else {
            "README.en.md"
        };
        entries.insert(
            id.clone(),
            ApplicationCollectorResponse {
                collector_code: manifest.collector_code.clone(),
                source_client: manifest.source_client.clone(),
                display_name: manifest.display_name.clone(),
                description: manifest
                    .description
                    .get(locale.as_str())
                    .or_else(|| manifest.description.get("en_US"))
                    .cloned()
                    .unwrap_or_default(),
                version: record.identity.version.clone(),
                execution_target: manifest.execution_target.clone(),
                catalog_id: id,
                category: "runtime-extensions".into(),
                installation_status: if valid.is_some() {
                    "installed"
                } else {
                    "missing"
                }
                .into(),
                installed_version: Some(record.identity.version.clone()),
                extension_installation_id: Some(record.id.to_string()),
                installable: false,
                can_install,
                can_update,
                documentation_url: asset_url(readme),
                shell_installer_url: asset_url("install.sh"),
                powershell_installer_url: asset_url("install.ps1"),
                asset_base_url: base,
            },
        );
    }
    let mut cursor = None;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let page = dependencies
            .source
            .search_for_workspace(
                actor.current_workspace_id,
                "runtime-extensions",
                OfficialExtensionCatalogSearchQuery {
                    slot_code: None,
                    q: None,
                    limit: 100,
                    cursor: cursor.clone(),
                },
            )
            .await;
        let Ok(page) = page else {
            break;
        }; // Retained distributions remain usable when sources are offline.
        for remote in page.entries {
            let Ok(Some(descriptor)) = remote.source.client_collector() else {
                continue;
            };
            if remote.category != "runtime-extensions"
                || remote.id
                    != format!(
                        "runtime-extensions:{}/{}",
                        remote.organization, remote.artifact
                    )
                || dependencies.source.resolve_artifact(&remote).is_err()
            {
                continue;
            }
            let entry =
                entries
                    .entry(remote.id.clone())
                    .or_insert_with(|| ApplicationCollectorResponse {
                        collector_code: descriptor.collector_code.clone(),
                        source_client: descriptor.source_client.clone(),
                        display_name: descriptor.display_name.clone(),
                        description: remote.description.clone(),
                        version: remote.version.clone(),
                        execution_target: descriptor.execution_target.clone(),
                        catalog_id: remote.id.clone(),
                        category: remote.category.clone(),
                        installation_status: "not_installed".into(),
                        installed_version: None,
                        extension_installation_id: None,
                        installable: true,
                        can_install,
                        can_update,
                        asset_base_url: None,
                        documentation_url: None,
                        shell_installer_url: None,
                        powershell_installer_url: None,
                    });
            entry.version = remote.version;
            entry.installable = true;
        }
        cursor = page.next_cursor;
        if cursor
            .as_ref()
            .is_none_or(|value| !seen.insert(value.clone()))
        {
            break;
        }
    }
    Ok(entries.into_values().collect())
}
