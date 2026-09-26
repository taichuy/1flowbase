use super::references::{contains_identity, route_references, string_values};
use super::*;
use crate::mcp_bundle::ExportMcpBundleCommand;
use crate::mcp_management::McpManagementService;
use crate::ports::ApplicationRepository;
use crate::ports::McpManagementRepository;
use anyhow::{bail, Result};
use std::collections::BTreeSet;
use uuid::Uuid;

pub struct PortableTemplateService<R> {
    repository: R,
}
impl<
        R: PortableTemplateReadRepository
            + PortableTemplateIdentityRepository
            + ApplicationRepository
            + McpManagementRepository
            + Clone,
    > PortableTemplateService<R>
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    async fn snapshot(&self, actor_user_id: Uuid) -> Result<PortableTemplatePackage> {
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        self.repository
            .portable_template_snapshot(actor_user_id, actor.current_workspace_id)
            .await
    }
    pub async fn catalog(&self, actor_user_id: Uuid) -> Result<PortableTemplateCatalog> {
        let all = self.snapshot(actor_user_id).await?;
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        let mcp_instances = match McpManagementService::new(self.repository.clone())
            .authorize_bundle_management(actor_user_id)
            .await
        {
            Ok(()) => {
                self.repository
                    .list_mcp_instances(actor.current_workspace_id)
                    .await?
            }
            Err(error)
                if matches!(
                    error.downcast_ref::<crate::errors::ControlPlaneError>(),
                    Some(crate::errors::ControlPlaneError::PermissionDenied(_))
                ) =>
            {
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        Ok(PortableTemplateCatalog {
            mcp_instances: mcp_instances
                .into_iter()
                .map(|instance| PortableMcpCatalogItem {
                    id: instance.instance_id,
                    name: instance.name,
                })
                .collect(),
            pages: all
                .pages
                .iter()
                .map(|p| PortableCatalogItem {
                    id: p.id,
                    name: p
                        .title
                        .clone()
                        .or(p.slug.clone())
                        .unwrap_or_else(|| p.id.to_string()),
                    parent_id: p.parent_id,
                    code: p.slug.clone(),
                })
                .collect(),
            applications: all
                .applications
                .iter()
                .map(|a| PortableCatalogItem {
                    id: a.id,
                    name: a.name.clone(),
                    parent_id: None,
                    code: None,
                })
                .collect(),
            data_models: all
                .data_models
                .iter()
                .filter(|m| !m.builtin)
                .map(|m| PortableCatalogItem {
                    id: m.id,
                    name: m.title.clone(),
                    parent_id: None,
                    code: Some(m.code.clone()),
                })
                .collect(),
        })
    }
    pub async fn export(
        &self,
        actor_user_id: Uuid,
        selection: PortableTemplateSelection,
    ) -> Result<PortableTemplatePackage> {
        let mcp_ids = selection.mcp_instance_ids.clone();
        let mut package = export_selected_template(self.snapshot(actor_user_id).await?, selection)?;
        if !mcp_ids.is_empty() {
            let mut bundle = McpManagementService::new(self.repository.clone())
                .export_bundle(ExportMcpBundleCommand {
                    actor_user_id,
                    organization: "1flowbase".into(),
                    bundle_id: "application-template".into(),
                    bundle_version: "1.0.0".into(),
                    locale: "zh_Hans".into(),
                    current_system_version: env!("CARGO_PKG_VERSION").into(),
                })
                .await?;
            let selected: BTreeSet<_> = mcp_ids.into_iter().collect();
            anyhow::ensure!(
                selected.iter().all(|id| bundle
                    .instances
                    .iter()
                    .any(|instance| &instance.instance_id == id)),
                "portable_template_selection_missing_mcp_instance"
            );
            bundle
                .instances
                .retain(|instance| selected.contains(&instance.instance_id));
            let tools: BTreeSet<_> = bundle
                .instances
                .iter()
                .flat_map(|instance| {
                    instance
                        .bindings
                        .iter()
                        .map(|binding| binding.tool_id.as_str())
                })
                .collect();
            bundle
                .tools
                .retain(|tool| tools.contains(tool.tool_id.as_str()));
            let connections: BTreeSet<_> = bundle
                .tools
                .iter()
                .filter_map(|tool| match &tool.execution_target {
                    domain::McpToolExecutionTarget::McpProxy {
                        upstream_connection_id,
                        ..
                    } => Some(*upstream_connection_id),
                    _ => None,
                })
                .collect();
            bundle
                .connections
                .retain(|connection| connections.contains(&connection.connection_id));
            bundle.manifest.files.clear();
            package.mcp_bundle = Some(bundle);
        }
        let failures = validate_portable_template(&package);
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(package)
    }
    pub async fn preview(
        &self,
        actor_user_id: Uuid,
        package: &PortableTemplatePackage,
    ) -> Result<PortableTemplatePreview> {
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        let identities = self
            .repository
            .load_portable_template_identity_map(actor.current_workspace_id)
            .await?;
        Ok(preview_portable_template_with_map(
            package,
            &self.snapshot(actor_user_id).await?,
            &identities,
        ))
    }
}

/// Selection closure is computed before projection. Unselected objects never enter the package.
pub fn export_selected_template(
    mut all: PortableTemplatePackage,
    selection: PortableTemplateSelection,
) -> Result<PortableTemplatePackage> {
    let mcp_selected = !selection.mcp_instance_ids.is_empty();
    let mut pages: BTreeSet<Uuid> = selection.page_ids.into_iter().collect();
    let mut apps: BTreeSet<Uuid> = selection.application_ids.into_iter().collect();
    let mut models: BTreeSet<Uuid> = selection.data_model_ids.into_iter().collect();
    if pages.is_empty()
        && apps.is_empty()
        && models.is_empty()
        && selection.mcp_instance_ids.is_empty()
    {
        bail!("portable_template_empty_selection");
    }
    if pages
        .iter()
        .any(|id| !all.pages.iter().any(|p| p.id == *id))
        || apps
            .iter()
            .any(|id| !all.applications.iter().any(|a| a.id == *id))
        || models
            .iter()
            .any(|id| !all.data_models.iter().any(|m| m.id == *id))
    {
        bail!("portable_template_selection_missing_or_external_source");
    }
    loop {
        let before = (pages.len(), apps.len(), models.len());
        for page in &all.pages {
            if page.parent_id.is_some_and(|id| pages.contains(&id)) {
                pages.insert(page.id);
            }
        }
        let mut texts = Vec::new();
        for page in all.pages.iter().filter(|p| pages.contains(&p.id)) {
            for tab in &page.tabs {
                string_values(&tab.document_payload, &mut texts);
                for block in &tab.blocks {
                    string_values(&block.runtime_descriptor, &mut texts);
                    texts.push(block.source_code.clone());
                }
            }
        }
        for app in all.applications.iter().filter(|a| apps.contains(&a.id)) {
            string_values(&serde_json::to_value(app)?, &mut texts);
        }
        for model in all
            .data_models
            .iter()
            .filter(|m| models.contains(&m.id) && !m.builtin)
        {
            for field in model.fields.iter().filter(|f| !f.is_system) {
                string_values(&serde_json::to_value(field)?, &mut texts);
            }
        }
        for text in &texts {
            for slug in route_references(text, "/api/ex/") {
                let app = all.applications.iter().find(|a| {
                    a.published
                        .as_ref()
                        .is_some_and(|p| p.mapping.extension_slug() == Some(slug.as_str()))
                        || a.mapping
                            .as_ref()
                            .is_some_and(|m| m.extension_slug() == Some(slug.as_str()))
                });
                match app {
                    Some(app) => {
                        apps.insert(app.id);
                    }
                    None => bail!("portable_template_unresolved_extension:{slug}"),
                }
            }
            for code in route_references(text, "/api/runtime/models/") {
                match all.data_models.iter().find(|m| m.code == code) {
                    Some(model) => {
                        models.insert(model.id);
                    }
                    None => bail!("portable_template_unresolved_model:{code}"),
                }
            }
            for app in &all.applications {
                if contains_identity(text, &app.id.to_string()) {
                    apps.insert(app.id);
                }
            }
            for model in &all.data_models {
                if contains_identity(text, &model.id.to_string())
                    || model
                        .fields
                        .iter()
                        .any(|f| contains_identity(text, &f.id.to_string()))
                {
                    models.insert(model.id);
                }
            }
            for page in &all.pages {
                if contains_identity(text, &page.id.to_string())
                    || page
                        .tabs
                        .iter()
                        .any(|t| contains_identity(text, &t.id.to_string()))
                {
                    pages.insert(page.id);
                }
            }
        }
        // Typed user relations are required even if their referenced identifier is malformed/absent.
        let mut relations = Vec::new();
        for model in all
            .data_models
            .iter()
            .filter(|m| models.contains(&m.id) && !m.builtin)
        {
            relations.extend(
                model
                    .fields
                    .iter()
                    .filter(|f| !f.is_system)
                    .filter_map(|f| f.relation_target_model_id),
            );
        }
        for id in relations {
            if !all.data_models.iter().any(|m| m.id == id) {
                bail!("portable_template_unresolved_relation:{id}");
            }
            models.insert(id);
        }
        if before == (pages.len(), apps.len(), models.len()) {
            break;
        }
    }
    // Keep the selected pages' navigation ancestry without selecting unrelated siblings.
    // Detaching a child would discard its root route and can make its placement invalid.
    loop {
        let parents: Vec<_> = all
            .pages
            .iter()
            .filter(|page| pages.contains(&page.id))
            .filter_map(|page| page.parent_id)
            .collect();
        let before = pages.len();
        for parent in parents {
            if !all.pages.iter().any(|page| page.id == parent) {
                bail!("portable_template_unresolved_page_parent:{parent}");
            }
            pages.insert(parent);
        }
        if pages.len() == before {
            break;
        }
    }
    all.pages.retain(|p| pages.contains(&p.id));
    all.applications.retain(|a| apps.contains(&a.id));
    all.data_models.retain(|m| models.contains(&m.id));
    all.plugins = collect_portable_plugin_dependencies(&all, &all.plugins);
    let failures = validate_portable_template(&all)
        .into_iter()
        .filter(|failure| !mcp_selected || failure != "portable_template_empty")
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }
    Ok(all)
}

pub(crate) fn collect_portable_plugin_dependencies(
    package: &PortableTemplatePackage,
    installed: &[PortablePluginDependency],
) -> Vec<PortablePluginDependency> {
    fn walk(
        v: &serde_json::Value,
        installed: &[PortablePluginDependency],
        out: &mut Vec<PortablePluginDependency>,
    ) {
        match v {
            serde_json::Value::Object(o) => {
                if let (Some(id), Some(version)) = (
                    o.get("plugin_id")
                        .or_else(|| o.get("pluginId"))
                        .and_then(|v| v.as_str()),
                    o.get("plugin_version")
                        .or_else(|| o.get("pluginVersion"))
                        .and_then(|v| v.as_str()),
                ) {
                    let dep = PortablePluginDependency {
                        plugin_id: id.into(),
                        plugin_version: version.into(),
                        checksum: installed
                            .iter()
                            .find(|p| p.plugin_id == id && p.plugin_version == version)
                            .and_then(|p| p.checksum.clone()),
                        contribution_code: o
                            .get("contribution_code")
                            .or_else(|| o.get("code"))
                            .and_then(|v| v.as_str())
                            .map(str::to_owned),
                    };
                    if !out.contains(&dep) {
                        out.push(dep);
                    }
                }
                for value in o.values() {
                    walk(value, installed, out);
                }
            }
            serde_json::Value::Array(a) => {
                for value in a {
                    walk(value, installed, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    // This is a definition-only projection, never an installation/configuration record.
    for app in &package.applications {
        walk(&app.flow_document, installed, &mut out);
        if let Some(p) = &app.published {
            walk(&p.flow_document, installed, &mut out);
        }
    }
    for page in &package.pages {
        for tab in &page.tabs {
            walk(&tab.document_payload, installed, &mut out);
            for b in &tab.blocks {
                walk(&b.runtime_descriptor, installed, &mut out);
            }
        }
    }
    out
}
