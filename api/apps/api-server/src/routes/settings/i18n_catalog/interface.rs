use std::sync::Arc;

use control_plane::{
    errors::ControlPlaneError,
    i18n_catalog::{
        OfficialI18nCatalogUpdateCommand, OfficialI18nCatalogUpdateOutcome,
        OfficialI18nCatalogUpdateService, OfficialI18nCatalogUpdateStatus,
        VerifiedOfficialCatalogSeed,
    },
    plugin_management::{
        installed_extension_integrity_warnings, validate_extension_integrity_override,
        ExtensionInstallationService, ExtensionRiskOverride,
    },
    ports::I18nCatalogRepository,
};
use domain::WorkspaceCatalogRevision;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::{
    management::{
        CatalogEntryMutationResponse, CatalogManagementPageResponse, CatalogRevisionResponse,
        DeleteCustomCatalogKeyBody, GetCatalogEntryQuery, ListCatalogEntriesQuery,
        RestoreCatalogOverrideBody, RestoreCatalogOverridesBody, UpsertCatalogTranslationBody,
    },
    ActivateI18nCatalogBody, ActivateI18nCatalogResponse, ActivateInstalledI18nCatalogBody,
    I18nCatalogStateResponse, I18nCatalogUpdateStatusResponse, InstalledI18nCatalogPreviewResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum I18nCatalogInput {
    GetState,
    CheckUpdate,
    ActivateOfficial(ActivateI18nCatalogBody),
    PreviewInstalled {
        installation_id: uuid::Uuid,
    },
    ActivateInstalled {
        installation_id: uuid::Uuid,
        body: ActivateInstalledI18nCatalogBody,
    },
    ListEntries(ListCatalogEntriesQuery),
    GetEntry(GetCatalogEntryQuery),
    UpsertOfficialOverride(UpsertCatalogTranslationBody),
    RestoreOfficialOverride(RestoreCatalogOverrideBody),
    UpsertCustomTranslation(UpsertCatalogTranslationBody),
    DeleteCustomKey(DeleteCustomCatalogKeyBody),
    RestoreAllOfficialOverrides(RestoreCatalogOverridesBody),
}

impl InterfaceContract for I18nCatalogInput {
    const CONTRACT_ID: &'static str = "console-i18n-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum I18nCatalogOutput {
    State(I18nCatalogStateResponse),
    UpdateStatus(I18nCatalogUpdateStatusResponse),
    Activation(ActivateI18nCatalogResponse),
    InstalledPreview(InstalledI18nCatalogPreviewResponse),
    Entries(CatalogManagementPageResponse),
    Entry(super::management::CatalogManagementEntryResponse),
    EntryMutation(CatalogEntryMutationResponse),
    Revision(CatalogRevisionResponse),
}

impl InterfaceContract for I18nCatalogOutput {
    const CONTRACT_ID: &'static str = "console-i18n-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct I18nCatalogDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) bootstrap_workspace_id: uuid::Uuid,
    pub(crate) update_service: Arc<OfficialI18nCatalogUpdateService<MainDurableStore>>,
    pub(crate) api_node_id: String,
    pub(crate) provider_install_root: String,
}

struct I18nCatalogAdapter(I18nCatalogDependencies);

pub(crate) fn port(
    dependencies: I18nCatalogDependencies,
) -> Arc<dyn ConsoleInterfacePort<I18nCatalogInput, I18nCatalogOutput>> {
    Arc::new(I18nCatalogAdapter(dependencies))
}

impl I18nCatalogAdapter {
    fn require_root(&self, principal: &UserPrincipal) -> Result<domain::ActorContext, ApiError> {
        let actor = principal.actor();
        if !actor.is_root || actor.current_workspace_id != self.0.bootstrap_workspace_id {
            return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_actor").into());
        }
        Ok(actor.clone())
    }

    async fn state(&self, workspace_id: uuid::Uuid) -> Result<I18nCatalogStateResponse, ApiError> {
        let catalog_state =
            I18nCatalogRepository::get_workspace_catalog_state(&self.0.store, workspace_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
        let descriptor = match catalog_state.active_release_id() {
            Some(release_id) => Some(
                I18nCatalogRepository::get_i18n_catalog_release_descriptor(
                    &self.0.store,
                    workspace_id,
                    release_id,
                )
                .await?
                .ok_or(ControlPlaneError::NotFound("active_i18n_catalog_release"))?,
            ),
            None => None,
        };
        Ok(match descriptor {
            Some(descriptor) => I18nCatalogStateResponse {
                active_catalog_version: Some(descriptor.catalog_version.as_str().to_owned()),
                revision: catalog_state.revision().value(),
                source: "official",
                source_locale: descriptor.source_locale.as_str().to_owned(),
                locales: descriptor
                    .locales
                    .iter()
                    .map(|value| value.as_str().to_owned())
                    .collect(),
            },
            None => I18nCatalogStateResponse {
                active_catalog_version: None,
                revision: catalog_state.revision().value(),
                source: "official",
                source_locale: domain::I18N_CATALOG_SOURCE_LOCALE.to_owned(),
                locales: Vec::new(),
            },
        })
    }

    fn activation_response(
        outcome: OfficialI18nCatalogUpdateOutcome,
        expected_revision: WorkspaceCatalogRevision,
    ) -> ActivateI18nCatalogResponse {
        match outcome {
            OfficialI18nCatalogUpdateOutcome::Current { catalog_version } => {
                ActivateI18nCatalogResponse {
                    status: "current",
                    catalog_version: catalog_version.as_str().to_owned(),
                    revision: expected_revision.value(),
                }
            }
            OfficialI18nCatalogUpdateOutcome::Activated {
                catalog_version,
                state,
            } => ActivateI18nCatalogResponse {
                status: "activated",
                catalog_version: catalog_version.as_str().to_owned(),
                revision: state.revision().value(),
            },
        }
    }

    async fn load_installed(
        &self,
        installation_id: uuid::Uuid,
    ) -> Result<
        (
            domain::ExtensionInstallationRecord,
            VerifiedOfficialCatalogSeed,
            Vec<domain::ExtensionIntegrityWarning>,
        ),
        ApiError,
    > {
        let installation =
            ExtensionInstallationService::new(self.0.store.clone(), &self.0.provider_install_root)
                .find_local_installation_by_id(&self.0.api_node_id, installation_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("extension_installation"))?;
        if installation.identity.category != domain::ExtensionCategory::I18n
            || installation.application_action != domain::ExtensionApplicationAction::ActivateI18n
        {
            return Err(ControlPlaneError::InvalidInput("i18n_extension_installation").into());
        }
        let local_path = installation
            .local_path
            .as_deref()
            .ok_or(ControlPlaneError::Conflict(
                "extension_artifact_path_missing",
            ))?;
        let bytes = tokio::fs::read(local_path).await?;
        let warnings = installed_extension_integrity_warnings(&installation, &bytes);
        let seed = tokio::task::spawn_blocking(move || {
            let inspection = crate::official_i18n_catalog_seed::inspect_catalog_seed(&bytes)?;
            crate::official_i18n_catalog_seed::decode_downloaded_catalog_seed(&bytes, &inspection)
        })
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_seed"))?
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_seed"))?;
        Ok((installation, seed, warnings))
    }

    async fn entry_after_mutation(
        &self,
        access: control_plane::i18n_catalog::management::CatalogManagementAccess,
        identity: domain::CatalogMessageIdentity,
        locale: domain::CatalogLocale,
        revision: WorkspaceCatalogRevision,
    ) -> Result<CatalogEntryMutationResponse, ApiError> {
        let entry = control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
            self.0.store.clone(),
            self.0.bootstrap_workspace_id,
        )
        .detail(
            control_plane::i18n_catalog::management::GetCatalogEntryCommand {
                access,
                identity,
                locale,
            },
        )
        .await?;
        Ok(CatalogEntryMutationResponse {
            revision: revision.value(),
            entry: entry.into(),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: I18nCatalogInput,
    ) -> Result<I18nCatalogOutput, ApiError> {
        let actor = self.require_root(principal)?;
        let workspace_id = actor.current_workspace_id;
        match input {
            I18nCatalogInput::GetState => {
                Ok(I18nCatalogOutput::State(self.state(workspace_id).await?))
            }
            I18nCatalogInput::CheckUpdate => {
                let status = self.0.update_service.check_update(workspace_id).await?;
                Ok(I18nCatalogOutput::UpdateStatus(match status {
                    OfficialI18nCatalogUpdateStatus::Current {
                        active_catalog_version,
                        latest_catalog_version,
                    } => I18nCatalogUpdateStatusResponse {
                        status: "current",
                        active_catalog_version: Some(active_catalog_version.as_str().to_owned()),
                        latest_catalog_version: latest_catalog_version.as_str().to_owned(),
                    },
                    OfficialI18nCatalogUpdateStatus::UpdateAvailable {
                        active_catalog_version,
                        latest_catalog_version,
                    } => I18nCatalogUpdateStatusResponse {
                        status: "update_available",
                        active_catalog_version: active_catalog_version
                            .map(|value| value.as_str().to_owned()),
                        latest_catalog_version: latest_catalog_version.as_str().to_owned(),
                    },
                }))
            }
            I18nCatalogInput::ActivateOfficial(body) => {
                let expected_revision = super::management::revision(body.expected_revision)?;
                let outcome = self
                    .0
                    .update_service
                    .check_and_activate(OfficialI18nCatalogUpdateCommand {
                        workspace_id,
                        expected_revision,
                    })
                    .await?;
                Ok(I18nCatalogOutput::Activation(Self::activation_response(
                    outcome,
                    expected_revision,
                )))
            }
            I18nCatalogInput::PreviewInstalled { installation_id } => {
                let (_, seed, warnings) = self.load_installed(installation_id).await?;
                let catalog_state =
                    I18nCatalogRepository::get_workspace_catalog_state(&self.0.store, workspace_id)
                        .await?
                        .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
                let active = match catalog_state.active_release_id() {
                    Some(release_id) => {
                        I18nCatalogRepository::get_i18n_catalog_release_descriptor(
                            &self.0.store,
                            workspace_id,
                            release_id,
                        )
                        .await?
                    }
                    None => None,
                };
                let applied = active.as_ref().is_some_and(|descriptor| {
                    descriptor.catalog_version == *seed.catalog_version()
                        && descriptor.semantic_sha256 == *seed.semantic_sha256()
                });
                Ok(I18nCatalogOutput::InstalledPreview(
                    InstalledI18nCatalogPreviewResponse {
                        extension_installation_id: installation_id.to_string(),
                        application_status: if applied { "applied" } else { "not_applied" }
                            .to_string(),
                        active_catalog_version: active
                            .map(|descriptor| descriptor.catalog_version.as_str().to_string()),
                        installed_catalog_version: seed.catalog_version().as_str().to_string(),
                        revision: catalog_state.revision().value(),
                        required_integrity_override: (!warnings.is_empty()).then(|| {
                            domain::ExtensionRiskChallenge {
                                warnings: warnings.clone(),
                                compatibility: None,
                            }
                        }),
                        integrity_warnings: warnings,
                    },
                ))
            }
            I18nCatalogInput::ActivateInstalled {
                installation_id,
                body,
            } => {
                let (_, seed, warnings) = self.load_installed(installation_id).await?;
                let risk_override = body.integrity_override.map(|value| ExtensionRiskOverride {
                    reason: value.reason,
                    acknowledged_warnings: value.acknowledged_warnings,
                });
                if !validate_extension_integrity_override(&warnings, risk_override.as_ref())? {
                    return Err(ControlPlaneError::Conflict(
                        "i18n_catalog_integrity_confirmation_required",
                    )
                    .into());
                }
                let expected_revision = super::management::revision(body.expected_revision)?;
                let outcome = self
                    .0
                    .update_service
                    .activate_installed(
                        OfficialI18nCatalogUpdateCommand {
                            workspace_id,
                            expected_revision,
                        },
                        seed,
                    )
                    .await?;
                Ok(I18nCatalogOutput::Activation(Self::activation_response(
                    outcome,
                    expected_revision,
                )))
            }
            I18nCatalogInput::ListEntries(query) => {
                let page =
                    control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
                        self.0.store.clone(),
                        self.0.bootstrap_workspace_id,
                    )
                    .list(
                        control_plane::i18n_catalog::management::ListCatalogEntriesCommand {
                            access: super::management::access(actor),
                            key: query.key,
                            locale: query.locale.map(super::management::locale).transpose()?,
                            search: query.search,
                            origin: query.origin.map(Into::into),
                            offset: query.offset.unwrap_or(0),
                            limit: query.limit.unwrap_or(super::management::DEFAULT_PAGE_LIMIT),
                        },
                    )
                    .await?;
                Ok(I18nCatalogOutput::Entries(CatalogManagementPageResponse {
                    entries: page.entries.into_iter().map(Into::into).collect(),
                    total: page.total,
                    revision: page.revision.value(),
                }))
            }
            I18nCatalogInput::GetEntry(query) => {
                let entry =
                    control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
                        self.0.store.clone(),
                        self.0.bootstrap_workspace_id,
                    )
                    .detail(
                        control_plane::i18n_catalog::management::GetCatalogEntryCommand {
                            access: super::management::access(actor),
                            identity: super::management::identity(query.key)?,
                            locale: super::management::locale(query.locale)?,
                        },
                    )
                    .await?;
                Ok(I18nCatalogOutput::Entry(entry.into()))
            }
            I18nCatalogInput::UpsertOfficialOverride(body) => {
                let value = super::management::translation(&body)?;
                let identity = value.identity().clone();
                let locale = value.locale().clone();
                let access = super::management::access(actor);
                let state =
                    control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
                        self.0.store.clone(),
                        self.0.bootstrap_workspace_id,
                    )
                    .upsert_official_override(
                        control_plane::i18n_catalog::management::UpsertOfficialOverrideCommand {
                            access: access.clone(),
                            value,
                            expected_revision: super::management::revision(body.expected_revision)?,
                        },
                    )
                    .await?;
                Ok(I18nCatalogOutput::EntryMutation(
                    self.entry_after_mutation(access, identity, locale, state.revision())
                        .await?,
                ))
            }
            I18nCatalogInput::RestoreOfficialOverride(body) => {
                let identity = super::management::identity(body.key)?;
                let locale = super::management::locale(body.locale)?;
                let access = super::management::access(actor);
                let state = control_plane::i18n_catalog::management::I18nCatalogManagementService::new(self.0.store.clone(), self.0.bootstrap_workspace_id)
                    .restore_official_translation(control_plane::i18n_catalog::management::RestoreOfficialTranslationCommand { access: access.clone(), identity: identity.clone(), locale: locale.clone(), expected_revision: super::management::revision(body.expected_revision)? }).await?;
                Ok(I18nCatalogOutput::EntryMutation(
                    self.entry_after_mutation(access, identity, locale, state.revision())
                        .await?,
                ))
            }
            I18nCatalogInput::UpsertCustomTranslation(body) => {
                let value = super::management::translation(&body)?;
                let identity = value.identity().clone();
                let locale = value.locale().clone();
                let access = super::management::access(actor);
                let state =
                    control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
                        self.0.store.clone(),
                        self.0.bootstrap_workspace_id,
                    )
                    .upsert_custom_translation(
                        control_plane::i18n_catalog::management::UpsertCustomTranslationCommand {
                            access: access.clone(),
                            value,
                            expected_revision: super::management::revision(body.expected_revision)?,
                        },
                    )
                    .await?;
                Ok(I18nCatalogOutput::EntryMutation(
                    self.entry_after_mutation(access, identity, locale, state.revision())
                        .await?,
                ))
            }
            I18nCatalogInput::DeleteCustomKey(body) => {
                let state =
                    control_plane::i18n_catalog::management::I18nCatalogManagementService::new(
                        self.0.store.clone(),
                        self.0.bootstrap_workspace_id,
                    )
                    .delete_custom_message(
                        control_plane::i18n_catalog::management::DeleteCustomMessageCommand {
                            access: super::management::access(actor),
                            identity: super::management::identity(body.key)?,
                            expected_revision: super::management::revision(body.expected_revision)?,
                        },
                    )
                    .await?;
                Ok(I18nCatalogOutput::Revision(CatalogRevisionResponse {
                    revision: state.revision().value(),
                }))
            }
            I18nCatalogInput::RestoreAllOfficialOverrides(body) => {
                let state = control_plane::i18n_catalog::management::I18nCatalogManagementService::new(self.0.store.clone(), self.0.bootstrap_workspace_id)
                    .restore_all_official_overrides(control_plane::i18n_catalog::management::RestoreAllOfficialOverridesCommand { access: super::management::access(actor), expected_revision: super::management::revision(body.expected_revision)? }).await?;
                Ok(I18nCatalogOutput::Revision(CatalogRevisionResponse {
                    revision: state.revision().value(),
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<I18nCatalogInput, I18nCatalogOutput> for I18nCatalogAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: I18nCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, I18nCatalogOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.state.get",
        binding_id: "http.console.i18n.catalog.get.v1",
        method: "GET",
        path: "/api/console/settings/i18n/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.update.check",
        binding_id: "http.console.i18n.update-check.get.v1",
        method: "GET",
        path: "/api/console/settings/i18n/update-check",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.update.activate",
        binding_id: "http.console.i18n.activate.post.v1",
        method: "POST",
        path: "/api/console/settings/i18n/activate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.installed_extension.preview",
        binding_id: "http.console.i18n.installed-extension.preview.get.v1",
        method: "GET",
        path: "/api/console/settings/i18n/installed-extension/:installation_id/preview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.installed_extension.activate",
        binding_id: "http.console.i18n.installed-extension.activate.post.v1",
        method: "POST",
        path: "/api/console/settings/i18n/installed-extension/:installation_id/activate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.entries.list",
        binding_id: "http.console.i18n.entries.list.get.v1",
        method: "GET",
        path: "/api/console/settings/i18n/entries",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.entries.detail",
        binding_id: "http.console.i18n.entries.detail.get.v1",
        method: "GET",
        path: "/api/console/settings/i18n/entries/detail",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.overrides.upsert",
        binding_id: "http.console.i18n.overrides.put.v1",
        method: "PUT",
        path: "/api/console/settings/i18n/overrides",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.overrides.restore",
        binding_id: "http.console.i18n.overrides.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/i18n/overrides",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.custom_translations.upsert",
        binding_id: "http.console.i18n.custom-translations.put.v1",
        method: "PUT",
        path: "/api/console/settings/i18n/custom-translations",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.custom_keys.delete",
        binding_id: "http.console.i18n.custom-keys.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/i18n/custom-keys",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "i18n_catalog.overrides.restore_all",
        binding_id: "http.console.i18n.restore-overrides.post.v1",
        method: "POST",
        path: "/api/console/settings/i18n/restore-overrides",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<I18nCatalogInput, I18nCatalogOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-i18n-catalog",
        "graph:console-i18n-catalog-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableI18nCatalogPort;

#[cfg(test)]
impl ConsoleInterfacePort<I18nCatalogInput, I18nCatalogOutput> for UnavailableI18nCatalogPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: I18nCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, I18nCatalogOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("i18n catalog fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f13a1_registry_freezes_i18n_catalog_bindings() {
        let registry = compile_registry(Arc::new(UnavailableI18nCatalogPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared i18n catalog binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
