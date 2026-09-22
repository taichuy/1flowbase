use super::plugins::TemplateDependencies;
use crate::error_response::ApiError;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use control_plane::portable_template::{
    PortableTemplateInstallService, PortableTemplatePackage, PortableTemplateSelection,
    PortableTemplateService,
};
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
                self.0.resolve_plugins(actor, &package.plugins).await?;
                serde_json::to_value(
                    PortableTemplateInstallService::new(repository)
                        .with_node_id(self.0.api_node_id.clone())
                        .install(actor.user_id, package)
                        .await?,
                )?
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
