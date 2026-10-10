use super::plugins::TemplateDependencies;
use crate::error_response::ApiError;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with;
use anyhow::Context;
use control_plane::portable_template::PortableTemplateIdentityRepository;
use control_plane::portable_template::{
    PortableTemplateInstallService, PortableTemplatePackage, PortableTemplateSelection,
    PortableTemplateService,
};
use control_plane::ports::RuntimeRegistrySync;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::{json, Value};
use std::sync::Arc;
pub(crate) enum TemplateInput {
    Catalog,
    Library(super::catalog::CatalogQuery),
    ExportArchive(PortableTemplateSelection),
    ResolvePreview(super::catalog::TemplateRequest),
    ResolveInstall(super::catalog::TemplateRequest),
    Export(PortableTemplateSelection),
    Preview(PortableTemplatePackage),
    Install(PortableTemplatePackage),
}
pub(crate) struct TemplateOutput(pub Value);
impl InterfaceContract for TemplateInput {
    const CONTRACT_ID: &'static str = "api-server.console-system-templates.input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::object_schema(&[
            (
                "operation",
                json!({"type":"string","maxLength":16,"enum":["catalog","export","preview","install"]}),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"operation": match self {Self::Catalog|Self::Library(_)=>"catalog",Self::Export(_)|Self::ExportArchive(_)=>"export",Self::Preview(_)|Self::ResolvePreview(_)=>"preview",Self::Install(_)|Self::ResolveInstall(_)=>"install"}}),
        )
    }
}
impl InterfaceContract for TemplateOutput {
    const CONTRACT_ID: &'static str = "api-server.console-system-templates.output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::object_schema(&[
            ("completed", json!({"type":"boolean"})),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(json!({"completed":true}))
    }
}
pub(crate) struct TemplateAdapter(pub TemplateDependencies);
impl TemplateAdapter {
    pub(crate) async fn execute_inner(
        &self,
        actor: &domain::ActorContext,
        input: TemplateInput,
    ) -> Result<TemplateOutput, ApiError> {
        let input = match input {
            TemplateInput::ResolvePreview(request) => {
                TemplateInput::Preview(super::catalog::resolve(&self.0, actor, request).await?)
            }
            TemplateInput::ResolveInstall(request) => {
                TemplateInput::Install(super::catalog::resolve(&self.0, actor, request).await?)
            }
            other => other,
        };
        let repository = self.0.store.for_actor(actor.clone());
        let service = PortableTemplateService::new(repository.clone());
        let result = match input {
            TemplateInput::Catalog => {
                let mut catalog = service.catalog(actor.user_id).await?;
                catalog.i18n_entries = self.0.translation_catalog(actor).await?;
                serde_json::to_value(catalog)?
            }
            TemplateInput::Library(query) => super::catalog::list(&self.0, actor, query).await?,
            TemplateInput::ExportArchive(selection) => {
                use base64::Engine;
                let package = self.0.export_template(actor, selection).await?;
                tokio::task::spawn_blocking(move || {
                    let bytes = super::archive::encode(&package)?;
                    Ok::<_, anyhow::Error>(json!({"archive_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"file_name":"application-template.zip"}))
                })
                .await
                .context("application_template_export_encode_task")??
            }
            TemplateInput::ResolvePreview(_) | TemplateInput::ResolveInstall(_) => {
                unreachable!("resolved before dispatch")
            }
            TemplateInput::Export(selection) => {
                serde_json::to_value(self.0.export_template(actor, selection).await?)?
            }
            TemplateInput::Preview(package) => {
                let mut preview = service.preview(actor.user_id, &package).await?;
                if !package.i18n_entries.is_empty() {
                    match control_plane::portable_template::i18n_merge::preview(
                        &self.0.translation_service(),
                        &repository,
                        &control_plane::portable_template::template_baseline_scope(
                            actor.current_workspace_id,
                            &package,
                        ),
                        &self.0.translation_access(actor),
                        &package.i18n_entries,
                    )
                    .await
                    {
                        Ok(effects) => preview.effects.extend(effects),
                        Err(error) => {
                            preview.valid = false;
                            preview
                                .failures
                                .push(format!("Translation preview: {error:#}"));
                        }
                    }
                }
                if let Some(release) = &package.release {
                    let records = repository
                        .load_application_template_releases(
                            actor.current_workspace_id,
                            &release.template_id,
                        )
                        .await?;
                    if let Err(error) =
                        control_plane::portable_template::application_template_needs_install(
                            release,
                            &control_plane::portable_template::application_template_checksum(
                                &package,
                            )?,
                            &records,
                        )
                    {
                        preview.valid = false;
                        preview.failures.push(format!("{error:#}"));
                    }
                }
                if let Some(bundle) = &package.mcp_bundle {
                    let identities = repository
                        .load_portable_template_identity_map(actor.current_workspace_id)
                        .await?;
                    let mut bundle = bundle.clone();
                    control_plane::portable_template::remap_mcp_bundle_interfaces(
                        &mut bundle,
                        &identities,
                    );
                    let mcp_preview = control_plane::portable_template::mcp_merge::preview(
                        &repository,
                        &control_plane::portable_template::template_baseline_scope(
                            actor.current_workspace_id,
                            &package,
                        ),
                        actor.user_id,
                        &bundle,
                        &mcp_interface_catalog_entries_with(&self.0.mcp_interface_catalog, actor)
                            .await?,
                        env!("CARGO_PKG_VERSION"),
                    )
                    .await?;
                    preview.mcp_shared_tool_impacts = mcp_preview.shared_tool_impacts;
                    preview.effects.extend(mcp_preview.effects);
                    preview.failures.extend(mcp_preview.conflicts);
                    preview.valid = preview.valid && preview.failures.is_empty();
                }
                if let Err(error) = PortableTemplateInstallService::new(repository.clone())
                    .preflight_visibility(actor, &package)
                    .await
                {
                    preview.failures.push(format!("{error:#}"));
                    preview.valid = false;
                }
                serde_json::to_value(preview)?
            }
            TemplateInput::Install(package) => {
                // A dedicated database session lock serializes API and startup installs across nodes.
                let _install_guard = repository
                    .lock_application_template_install(actor.current_workspace_id)
                    .await?;
                if !control_plane::portable_template::prepare_application_template_release(
                    &repository,
                    actor.current_workspace_id,
                    &package,
                )
                .await?
                {
                    return Ok(TemplateOutput(serde_json::to_value(
                        control_plane::portable_template::PortableTemplateInstallResult {
                            complete: true,
                            ..Default::default()
                        },
                    )?));
                }
                let release_state = package
                    .release
                    .clone()
                    .map(|release| {
                        control_plane::portable_template::application_template_checksum(&package)
                            .map(|checksum| (release, checksum))
                    })
                    .transpose()?;
                let preview = service.preview(actor.user_id, &package).await?;
                if !preview.valid {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "portable_template_preflight",
                    )
                    .into());
                }
                PortableTemplateInstallService::new(repository.clone())
                    .preflight_visibility(actor, &package)
                    .await?;
                if let Some(bundle) = &package.mcp_bundle {
                    let identities = repository
                        .load_portable_template_identity_map(actor.current_workspace_id)
                        .await?;
                    let mut bundle = bundle.clone();
                    control_plane::portable_template::remap_mcp_bundle_interfaces(
                        &mut bundle,
                        &identities,
                    );
                    let mcp_preview = control_plane::portable_template::mcp_merge::preview(
                        &repository,
                        &control_plane::portable_template::template_baseline_scope(
                            actor.current_workspace_id,
                            &package,
                        ),
                        actor.user_id,
                        &bundle,
                        &mcp_interface_catalog_entries_with(&self.0.mcp_interface_catalog, actor)
                            .await?,
                        env!("CARGO_PKG_VERSION"),
                    )
                    .await?;
                    if !mcp_preview.conflicts.is_empty() {
                        return Err(control_plane::errors::ControlPlaneError::Conflict(
                            "portable_template_mcp_conflict",
                        )
                        .into());
                    }
                }
                let translation_scope = control_plane::portable_template::template_baseline_scope(
                    actor.current_workspace_id,
                    &package,
                );
                if !package.i18n_entries.is_empty() {
                    control_plane::portable_template::i18n_merge::preview(
                        &self.0.translation_service(),
                        &repository,
                        &translation_scope,
                        &self.0.translation_access(actor),
                        &package.i18n_entries,
                    )
                    .await?;
                }
                self.0.resolve_plugins(actor, &package.plugins).await?;
                let translations = package.i18n_entries.clone();
                let mcp_bundle = package.mcp_bundle.clone();
                let mut installed = PortableTemplateInstallService::new(repository.clone())
                    .with_node_id(self.0.api_node_id.clone())
                    .install(actor.user_id, package)
                    .await?;
                if installed.complete {
                    if let Some(mut bundle) = mcp_bundle {
                        control_plane::portable_template::remap_mcp_bundle_interfaces(
                            &mut bundle,
                            &installed.id_map,
                        );
                        let report = async {
                            let catalog = mcp_interface_catalog_entries_with(
                                &self.0.mcp_interface_catalog,
                                actor,
                            )
                            .await?;
                            control_plane::portable_template::mcp_merge::install(
                                &repository,
                                &translation_scope,
                                actor.user_id,
                                &bundle,
                                &catalog,
                                env!("CARGO_PKG_VERSION"),
                            )
                            .await
                        }
                        .await;
                        match report {
                            Ok(outcome) => {
                                installed.created.extend(outcome.created);
                                installed.updated.extend(outcome.updated);
                                installed.skipped.extend(outcome.skipped);
                                installed.failures.extend(outcome.failures);
                                installed.complete = installed.failures.is_empty();
                            }
                            Err(error) => {
                                installed.complete = false;
                                installed.failures.push(format!("MCP template installation: {error:#}; earlier definitions may already be committed"));
                            }
                        }
                    }
                }
                if !translations.is_empty() {
                    match control_plane::portable_template::i18n_merge::install(
                        &self.0.translation_service(), &repository, &translation_scope,
                        &self.0.translation_access(actor), &translations,
                    ).await {
                        Ok(outcome) => {
                            installed.created.extend(outcome.created);
                            installed.updated.extend(outcome.updated);
                            installed.skipped.extend(outcome.skipped);
                            installed.failures.extend(outcome.failures);
                        }
                        Err(error) => installed.failures.push(format!("Translation installation: {error:#}; earlier definitions may already be committed")),
                    }
                    installed.complete = installed.complete && installed.failures.is_empty();
                }
                // Owner writes may partially commit. Synchronize those definitions too.
                if let Err(error) = self.0.runtime_registry_sync.rebuild().await {
                    installed.complete = false;
                    installed.failures.push(format!("runtime model registry synchronization: {error:#}; definitions may already be committed"));
                }
                if installed.complete {
                    if let Some((release, checksum)) = release_state {
                        repository
                            .record_application_template_release(
                                actor.current_workspace_id,
                                &release.template_id,
                                release.release_version,
                                &checksum,
                                true,
                            )
                            .await?;
                    }
                }
                serde_json::to_value(installed)?
            }
        };
        Ok(TemplateOutput(result))
    }
}
impl ConsoleInterfacePort<TemplateInput, TemplateOutput> for TemplateAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: TemplateInput,
    ) -> ConsoleInterfaceFuture<'a, TemplateOutput> {
        Box::pin(async move {
            self.execute_inner(principal.actor(), input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "system_templates.catalog",
        binding_id: "http.console.settings.system-templates.catalog.v1",
        method: "GET",
        path: "/api/console/settings/system-templates/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_templates.export",
        binding_id: "http.console.settings.system-templates.export.v1",
        method: "POST",
        path: "/api/console/settings/system-templates/export",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_templates.preview",
        binding_id: "http.console.settings.system-templates.preview.v1",
        method: "POST",
        path: "/api/console/settings/system-templates/preview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_templates.install",
        binding_id: "http.console.settings.system-templates.install.v1",
        method: "POST",
        path: "/api/console/settings/system-templates/install",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    dependencies: TemplateDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-system-templates",
        "graph:console-system-templates-v1",
        DECLARATIONS,
        Arc::new(TemplateAdapter(dependencies)),
    )
}
