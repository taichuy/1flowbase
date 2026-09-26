use super::plugins::TemplateDependencies;
use crate::error_response::ApiError;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with;
use control_plane::mcp_bundle::{ImportMcpBundleCommand, PreviewMcpBundleCommand};
use control_plane::mcp_management::McpManagementService;
use control_plane::portable_template::{
    PortableTemplateEffect, PortableTemplateInstallService, PortableTemplatePackage,
    PortableTemplateSelection, PortableTemplateService,
};
use control_plane::ports::RuntimeRegistrySync;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::{json, Value};
use std::sync::Arc;
pub(crate) enum TemplateInput {
    Catalog,
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
            json!({"operation": match self {Self::Catalog=>"catalog",Self::Export(_)=>"export",Self::Preview(_)=>"preview",Self::Install(_)=>"install"}}),
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
struct TemplateAdapter(TemplateDependencies);
impl TemplateAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: TemplateInput,
    ) -> Result<TemplateOutput, ApiError> {
        let actor = principal.actor();
        let repository = self.0.store.for_actor(actor.clone());
        let service = PortableTemplateService::new(repository.clone());
        let result = match input {
            TemplateInput::Catalog => serde_json::to_value(service.catalog(actor.user_id).await?)?,
            TemplateInput::Export(selection) => {
                serde_json::to_value(service.export(actor.user_id, selection).await?)?
            }
            TemplateInput::Preview(package) => {
                let mut preview = service.preview(actor.user_id, &package).await?;
                if let Some(bundle) = &package.mcp_bundle {
                    let mcp_preview = McpManagementService::new(self.0.store.clone())
                        .preserve_unmentioned_instance_entries(actor.user_id, bundle.clone())
                        .await?;
                    let mcp_preview = McpManagementService::new(self.0.store.clone())
                        .preview_bundle(PreviewMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package: mcp_preview,
                            interface_catalog: mcp_interface_catalog_entries_with(
                                &self.0.mcp_interface_catalog,
                                actor,
                            )
                            .await?,
                            current_system_version: env!("CARGO_PKG_VERSION").into(),
                        })
                        .await?;
                    if mcp_preview.effect_summary.conflicts > 0
                        || mcp_preview.effect_summary.failed > 0
                    {
                        preview
                            .failures
                            .push("portable_template_mcp_conflict".into());
                        preview.valid = false;
                    }
                    preview
                        .effects
                        .extend(mcp_preview.instances.iter().map(|item| {
                            PortableTemplateEffect {
                                kind: "mcp_instance".into(),
                                source_id: item.id.clone(),
                                target_id: matches!(
                                    item.effect,
                                    domain::McpBundleItemEffect::Update
                                        | domain::McpBundleItemEffect::AlreadyPresent
                                )
                                .then(|| item.id.clone()),
                                action: match item.effect {
                                    domain::McpBundleItemEffect::Create => "create",
                                    domain::McpBundleItemEffect::AlreadyPresent => "unchanged",
                                    _ => "update",
                                }
                                .into(),
                            }
                        }));
                    for (kind, items) in [
                        ("mcp_tool", &mcp_preview.tools),
                        ("mcp_connection", &mcp_preview.connections),
                    ] {
                        preview.effects.extend(items.iter().map(|item| {
                            PortableTemplateEffect {
                                kind: kind.into(),
                                source_id: item.id.clone(),
                                target_id: (item.effect != domain::McpBundleItemEffect::Create)
                                    .then(|| item.id.clone()),
                                action: match item.effect {
                                    domain::McpBundleItemEffect::Create => "create",
                                    domain::McpBundleItemEffect::AlreadyPresent => "unchanged",
                                    _ => "update",
                                }
                                .into(),
                            }
                        }));
                    }
                    for item in mcp_preview.tools.iter().chain(&mcp_preview.connections) {
                        if let Some(reason) = &item.reason {
                            preview
                                .warnings
                                .push(format!("MCP {}: {}", item.id, reason));
                        }
                    }
                    preview.mcp_shared_tool_impacts = mcp_preview.shared_tool_impacts;
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
            TemplateInput::Install(mut package) => {
                if let Some(bundle) = package.mcp_bundle.take() {
                    package.mcp_bundle = Some(
                        McpManagementService::new(self.0.store.clone())
                            .preserve_unmentioned_instance_entries(actor.user_id, bundle)
                            .await?,
                    );
                }
                let mcp_catalog = if package.mcp_bundle.is_some() {
                    Some(
                        mcp_interface_catalog_entries_with(&self.0.mcp_interface_catalog, actor)
                            .await?,
                    )
                } else {
                    None
                };
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
                    let mcp_preview = McpManagementService::new(self.0.store.clone())
                        .preview_bundle(PreviewMcpBundleCommand {
                            actor_user_id: actor.user_id,
                            package: bundle.clone(),
                            interface_catalog: mcp_catalog.clone().unwrap_or_default(),
                            current_system_version: env!("CARGO_PKG_VERSION").into(),
                        })
                        .await?;
                    if mcp_preview.effect_summary.conflicts > 0
                        || mcp_preview.effect_summary.failed > 0
                    {
                        return Err(control_plane::errors::ControlPlaneError::Conflict(
                            "portable_template_mcp_conflict",
                        )
                        .into());
                    }
                }
                self.0.resolve_plugins(actor, &package.plugins).await?;
                let mcp_bundle = package.mcp_bundle.clone();
                let mut installed = PortableTemplateInstallService::new(repository)
                    .with_node_id(self.0.api_node_id.clone())
                    .install(actor.user_id, package)
                    .await?;
                if installed.complete {
                    if let Some(bundle) = mcp_bundle {
                        let report = McpManagementService::new(self.0.store.clone())
                            .import_bundle(ImportMcpBundleCommand {
                                actor_user_id: actor.user_id,
                                package: bundle,
                                interface_catalog: mcp_catalog.unwrap_or_default(),
                                current_system_version: env!("CARGO_PKG_VERSION").into(),
                            })
                            .await;
                        match report {
                            Ok(report) => {
                                if report.effect_summary.conflicts > 0
                                    || report.effect_summary.failed > 0
                                {
                                    installed.complete = false;
                                    installed
                                        .failures
                                        .push("portable_template_mcp_import_incomplete".into());
                                }
                                for (kind, items) in [
                                    ("mcp_instance", report.instances),
                                    ("mcp_tool", report.tools),
                                    ("mcp_connection", report.connections),
                                ] {
                                    for item in items {
                                        let resource = control_plane::portable_template::PortableTemplateCreatedResource {
                                            kind: kind.into(), source_id: item.id.clone(), target_id: item.id,
                                        };
                                        if item.effect == domain::McpBundleItemEffect::Create {
                                            installed.created.push(resource);
                                        } else if item.effect == domain::McpBundleItemEffect::Update
                                        {
                                            installed.updated.push(resource);
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                installed.complete = false;
                                installed.failures.push(format!("MCP instance import: {error:#}; earlier definitions may already be committed"));
                            }
                        }
                    }
                }
                // Owner writes may partially commit. Synchronize those definitions too.
                if let Err(error) = self.0.runtime_registry_sync.rebuild().await {
                    installed.complete = false;
                    installed.failures.push(format!("runtime model registry synchronization: {error:#}; definitions may already be committed"));
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
            self.execute_inner(principal, input)
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
