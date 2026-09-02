use std::{collections::HashMap, sync::Arc};

use control_plane::{
    network_egress::NetworkEgressProviderService,
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
    plugin_management::{
        DeletePluginFamilyCommand, InstallResolvedOfficialPluginCommand,
        InstallUploadedPluginCommand, PluginCatalogFilter, PluginManagementService,
    },
    ports::OfficialPluginSourcePort,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::{
    host_infrastructure::CacheStore,
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum NetworkPluginInput {
    ListOfficial {
        query: OfficialPluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ListFamilies {
        locale: ConsoleLocaleHints,
    },
    SwitchVersion {
        provider_code: String,
        body: SwitchNetworkEgressPluginVersionBody,
        locale: ConsoleLocaleHints,
    },
    UninstallVersion {
        provider_code: String,
        installation_id: String,
        locale: ConsoleLocaleHints,
    },
    UninstallFamily {
        provider_code: String,
        locale: ConsoleLocaleHints,
    },
    InstallOfficial(InstallOfficialPluginBody),
    InstallUploaded {
        file_name: String,
        package_bytes: Vec<u8>,
    },
}

impl InterfaceContract for NetworkPluginInput {
    const CONTRACT_ID: &'static str = "console-network-plugin-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed plugin output is projected immediately into the console response"
)]
pub(crate) enum NetworkPluginOutput {
    Official(NetworkEgressOfficialPluginCatalogResponse),
    Families(Vec<NetworkEgressPluginFamilyResponse>),
    Installed(InstallPluginResponse),
    Empty,
}

impl InterfaceContract for NetworkPluginOutput {
    const CONTRACT_ID: &'static str = "console-network-plugin-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkPluginDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<ApiRuntimeServices>,
    pub(crate) official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub(crate) official_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub(crate) cache_store: Arc<dyn CacheStore>,
    pub(crate) provider_install_root: String,
    pub(crate) provider_secret_master_key: String,
    pub(crate) api_node_id: String,
    pub(crate) bootstrap_workspace_id: uuid::Uuid,
    pub(crate) allow_uploaded_host_extensions: bool,
}

struct NetworkPluginAdapter(NetworkPluginDependencies);

impl NetworkPluginAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation: &'static str,
    ) -> crate::app_state::ApiPluginManagementService {
        PluginManagementService::new(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.official_plugin_source.clone(),
            self.0.provider_install_root.clone(),
        )
        .with_node_id(self.0.api_node_id.clone())
        .with_allow_uploaded_host_extensions(self.0.allow_uploaded_host_extensions)
        .with_model_routing_cache_store(self.0.cache_store.clone())
        .for_network_egress_provider_console_operation(operation)
    }

    fn provider_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
        NetworkEgressProviderService::new(
            self.0.store.clone(),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            ProviderRegistryNetworkEgressSecretResolver::new(
                self.0.store.clone(),
                self.0.provider_secret_master_key.clone(),
            ),
            self.0.provider_secret_master_key.clone(),
            self.0.api_node_id.clone(),
        )
    }

    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .0
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn families(
        &self,
        principal: &UserPrincipal,
        locale: ConsoleLocaleHints,
        operation: &'static str,
    ) -> Result<HashMap<String, NetworkEgressPluginFamilyResponse>, ApiError> {
        let locale_meta = locale.resolve_meta(None, self.preferred_locale(principal).await?);
        let catalog = self
            .service(principal.actor(), operation)
            .list_catalog(
                principal.actor().user_id,
                PluginCatalogFilter {
                    plugin_type: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                },
                requested_locales(&locale_meta),
            )
            .await?;
        Ok(project_plugin_families(catalog.entries)?)
    }

    async fn resolved_official_command(
        &self,
        actor_user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        body: InstallOfficialPluginBody,
    ) -> Result<InstallResolvedOfficialPluginCommand, ApiError> {
        let mut cursor = None;
        let (entry, source_kind) = loop {
            let page = self
                .0
                .official_catalog_source
                .list_page_for_workspace(workspace_id, "runtime-extensions", cursor.as_deref())
                .await?;
            if let Some(entry) = page.entries.into_iter().find(|entry| {
                entry
                    .source
                    .metadata
                    .get("plugin_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(body.plugin_id.as_str())
            }) {
                break (entry, page.source_kind);
            }
            let Some(next) = page.metadata.next_cursor else {
                return Err(
                    control_plane::errors::ControlPlaneError::NotFound("official_plugin").into(),
                );
            };
            cursor = Some(next);
        };
        let plugin_type = entry
            .source
            .metadata
            .get("plugin_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_plugin_type",
            ))?
            .to_string();
        let downloaded = self
            .0
            .official_catalog_source
            .download_artifact_for_workspace(workspace_id, &entry)
            .await?;
        let expected_checksum = downloaded.descriptor.expected_checksum.clone().ok_or(
            control_plane::errors::ControlPlaneError::InvalidInput("official_plugin_checksum"),
        )?;
        Ok(InstallResolvedOfficialPluginCommand {
            actor_user_id,
            plugin_id: body.plugin_id,
            plugin_type,
            minimum_host_version: entry.host_version_requirement,
            source_kind,
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
            expected_checksum,
            compatibility_override: crate::routes::plugins::to_compatibility_override(
                body.compatibility_override,
            ),
            risk_override: crate::routes::plugins::to_risk_override(body.risk_override),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkPluginInput,
    ) -> Result<NetworkPluginOutput, ApiError> {
        let actor = principal.actor();
        match input {
            NetworkPluginInput::ListOfficial { query, locale } => {
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let local_catalog = self
                    .service(actor, "network_egress_plugins.official_catalog.view")
                    .list_catalog(
                        actor.user_id,
                        PluginCatalogFilter {
                            plugin_type: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                        },
                        requested_locales(&locale_meta),
                    )
                    .await?;
                let installed = project_plugin_families(local_catalog.entries)?;
                let filter = official_filter(&query);
                let page = self
                    .0
                    .official_catalog_source
                    .search_for_workspace(
                        actor.current_workspace_id,
                        "runtime-extensions",
                        crate::official_extension_catalog::OfficialExtensionCatalogSearchQuery {
                            slot_code: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                            q: filter.search_query,
                            limit: filter.limit,
                            cursor: query.cursor,
                        },
                    )
                    .await?;
                let entries = page
                    .entries
                    .into_iter()
                    .filter_map(|entry| {
                        match project_catalog_entry(
                            self.0.official_catalog_source.as_ref(),
                            entry,
                            &installed,
                        ) {
                            Ok(Some(entry)) => Some(Ok(entry)),
                            Ok(None) => None,
                            Err(error) => Some(Err(error)),
                        }
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
                    .expect("runtime profile resolves a supported locale");
                let source_label = crate::app_state::resolve_official_source_label_with(
                    &self.0.store,
                    self.0.bootstrap_workspace_id,
                    &locale,
                    &page.source_kind,
                    page.source_kind.clone(),
                )
                .await?;
                Ok(NetworkPluginOutput::Official(
                    NetworkEgressOfficialPluginCatalogResponse {
                        source_kind: page.source_kind,
                        source_label,
                        registry_url: page.snapshot_locator,
                        source_freshness: "fresh".to_string(),
                        locale_meta,
                        page: OfficialPluginCatalogPageResponse {
                            limit: filter.limit,
                            next_cursor: page.next_cursor,
                        },
                        entries,
                    },
                ))
            }
            NetworkPluginInput::ListFamilies { locale } => {
                let mut families = self
                    .families(principal, locale, "network_egress_plugins.families.view")
                    .await?;
                mark_referenced_versions_not_uninstallable(&self.0.store, &mut families).await?;
                Ok(NetworkPluginOutput::Families(
                    families.into_values().collect(),
                ))
            }
            NetworkPluginInput::SwitchVersion {
                provider_code,
                body,
                locale,
            } => {
                let installation_id: uuid::Uuid = body.installation_id.parse().map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("installation_id")
                })?;
                let families = self
                    .families(principal, locale, "network_egress_plugins.families.switch")
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                if !family
                    .installed_versions
                    .iter()
                    .any(|version| version.installation_id == installation_id.to_string())
                {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "installation_id",
                    )
                    .into());
                }
                self.provider_service()
                    .activate_version(installation_id)
                    .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::UninstallVersion {
                provider_code,
                installation_id,
                locale,
            } => {
                let installation_id: uuid::Uuid = installation_id.parse().map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("installation_id")
                })?;
                let families = self
                    .families(
                        principal,
                        locale,
                        "network_egress_plugins.families.uninstall",
                    )
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                let version = family
                    .installed_versions
                    .iter()
                    .find(|version| version.installation_id == installation_id.to_string())
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "installation_id",
                    ))?;
                if !version.can_uninstall {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "network_egress_plugin_version_uninstall_blocked",
                    )
                    .into());
                }
                control_plane::plugin_management::ExtensionInstallationService::new(
                    self.0.store.clone(),
                    &self.0.provider_install_root,
                )
                .delete_local_installation(&self.0.api_node_id, installation_id)
                .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::UninstallFamily {
                provider_code,
                locale,
            } => {
                let families = self
                    .families(
                        principal,
                        locale,
                        "network_egress_plugins.families.uninstall",
                    )
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                if self
                    .0
                    .store
                    .list_network_egress_providers()
                    .await?
                    .into_iter()
                    .filter_map(|provider| provider.extension_family)
                    .any(|provider_family| provider_family.artifact_id() == family.provider_code)
                {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "network_egress_plugin_family_uninstall_blocked",
                    )
                    .into());
                }
                self.service(actor, "network_egress_plugins.families.uninstall")
                    .delete_family(DeletePluginFamilyCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                    })
                    .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::InstallOfficial(body) => {
                let command = self
                    .resolved_official_command(actor.user_id, actor.current_workspace_id, body)
                    .await?;
                let result = self
                    .service(actor, "network_egress_plugins.install.official")
                    .install_resolved_official_plugin(command)
                    .await?;
                Ok(NetworkPluginOutput::Installed(to_install_response(result)))
            }
            NetworkPluginInput::InstallUploaded {
                file_name,
                package_bytes,
            } => {
                let result = self
                    .service(actor, "network_egress_plugins.install.upload")
                    .install_uploaded_network_egress_provider(InstallUploadedPluginCommand {
                        actor_user_id: actor.user_id,
                        file_name,
                        package_bytes,
                    })
                    .await?;
                Ok(NetworkPluginOutput::Installed(to_install_response(result)))
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkPluginInput, NetworkPluginOutput> for NetworkPluginAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkPluginInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkPluginOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.official_catalog.view", binding_id: "http.console.network-egress-plugins.official-catalog.v1", method: "GET", path: "/api/console/settings/network-center/proxy-plugins/official-catalog", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.families.view", binding_id: "http.console.network-egress-plugins.families.v1", method: "GET", path: "/api/console/settings/network-center/proxy-plugins/families", mutating: false },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.families.switch", binding_id: "http.console.network-egress-plugins.switch-version.v1", method: "POST", path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code/switch-version", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.families.uninstall", binding_id: "http.console.network-egress-plugins.uninstall-version.v1", method: "DELETE", path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code/versions/:installation_id", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.families.uninstall", binding_id: "http.console.network-egress-plugins.uninstall-family.v1", method: "DELETE", path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.install.official", binding_id: "http.console.network-egress-plugins.install-official.v1", method: "POST", path: "/api/console/settings/network-center/proxy-plugins/install-official", mutating: true },
    ConsoleInterfaceDeclaration { interface_id: "network_egress_plugins.install.upload", binding_id: "http.console.network-egress-plugins.install-upload.v1", method: "POST", path: "/api/console/settings/network-center/proxy-plugins/install-upload", mutating: true },
];

pub(crate) fn compile_registry(
    dependencies: NetworkPluginDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-plugins",
        "graph:console-network-plugins-v1",
        DECLARATIONS,
        Arc::new(NetworkPluginAdapter(dependencies)),
    )
}
