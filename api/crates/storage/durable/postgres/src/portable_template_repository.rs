//! Read-only workspace projections for portable definitions; never bootstraps draft state.
use crate::PgControlPlaneStore;
use anyhow::{Context, Result};
use async_trait::async_trait;
use control_plane_contracts::{portable_template::*, ports::*};
use sqlx::Row;
use uuid::Uuid;

#[async_trait]
impl PortableTemplateReadRepository for PgControlPlaneStore {
    async fn portable_template_snapshot(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<PortableTemplatePackage> {
        // Scope validation is retained even though callers already resolved their principal.
        FrontstagePageRepository::load_actor_context_for_workspace(
            self,
            actor_user_id,
            workspace_id,
        )
        .await?;
        let mut package = PortableTemplatePackage {
            schema_version: PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),
            pages: Vec::new(),
            applications: Vec::new(),
            data_models: Vec::new(),
            plugins: Vec::new(),
        };
        for model in ModelDefinitionRepository::list_model_definitions(self, workspace_id).await? {
            if model.source_kind != domain::DataModelSourceKind::MainSource {
                continue;
            }
            package.data_models.push(PortableDataModel {
                id: model.id,
                code: model.code,
                title: model.title,
                description: model.description,
                scope_kind: model.scope_kind,
                template_provider: model.template_provider,
                template_code: model.template_code,
                template_version: model.template_version,
                status: model.status,
                builtin: model.protection.is_protected,
                fields: model
                    .fields
                    .into_iter()
                    .map(|f| PortableModelField {
                        id: f.id,
                        code: f.code,
                        title: f.title,
                        description: f.description,
                        field_kind: f.field_kind,
                        is_system: f.is_system,
                        is_required: f.is_required,
                        api_required: f.api_required,
                        is_unique: f.is_unique,
                        default_value: f.default_value,
                        display_interface: f.display_interface,
                        display_options: f.display_options,
                        relation_target_model_id: f.relation_target_model_id,
                        relation_options: f.relation_options,
                    })
                    .collect(),
            });
        }
        for app in ApplicationRepository::list_applications(
            self,
            workspace_id,
            actor_user_id,
            ApplicationVisibility::All,
        )
        .await?
        {
            let flow_document: Option<serde_json::Value> = sqlx::query_scalar(
                "select d.document from flow_drafts d join flows f on f.id = d.flow_id where f.application_id = $1 and f.scope_id = $2"
            ).bind(app.id).bind(workspace_id).fetch_optional(self.pool()).await?;
            let mapping =
                ApplicationApiMappingRepository::get_application_api_mapping(self, app.id)
                    .await?
                    .map(|m| m.mapping);
            let publication =
                ApplicationPublicationRepository::load_active_application_publication(self, app.id)
                    .await?;
            let published_js_dependencies = publication
                .as_ref()
                .is_some_and(|p| !p.dependency_snapshot.is_empty());
            let published = publication.map(|p| PortablePublication {
                flow_document: p.document_snapshot,
                mapping: p.mapping_snapshot,
                api_enabled: p.api_enabled,
            });
            let schedule =
                WorkflowScheduleTriggerRepository::get_workflow_schedule_trigger(self, app.id)
                    .await?
                    .map(|s| PortableSchedule {
                        cron: s.cron,
                        timezone: s.timezone,
                        input_payload: s.input_payload,
                    });
            let dependency_issues = if ApplicationJsDependencySelectionRepository::list_application_js_dependency_selections(self, workspace_id, app.id).await?.is_empty() && !published_js_dependencies { Vec::new() } else { vec!["application_js_dependencies_not_portable".into()] };
            package.applications.push(PortableApplication {
                id: app.id,
                application_type: app.application_type,
                workflow_trigger_type: app.workflow_trigger_type,
                name: app.name,
                description: app.description,
                icon: app.icon,
                icon_type: app.icon_type,
                icon_background: app.icon_background,
                flow_document: flow_document.unwrap_or(serde_json::Value::Null),
                mapping,
                published,
                schedule,
                dependency_issues,
            });
        }
        for page in FrontstagePageRepository::list_frontstage_pages(self, workspace_id).await? {
            let mut tabs = Vec::new();
            for tab in
                FrontstagePageRepository::list_frontstage_page_tabs(self, workspace_id, page.id)
                    .await?
            {
                let detail = FrontstagePageRepository::get_frontstage_page_tab_detail(
                    self,
                    workspace_id,
                    page.id,
                    &tab.id.to_string(),
                )
                .await?
                .context("portable template tab document missing")?;
                let block_ids: Vec<String> = sqlx::query_scalar(
                    "select block_id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and tab_id = $3 order by sibling_rank, block_id"
                ).bind(workspace_id).bind(page.id).bind(tab.id).fetch_all(self.pool()).await?;
                let mut blocks = Vec::new();
                for block_id in block_ids {
                    let block = FrontstageBlockTreeRepository::get_frontstage_block_node(
                        self,
                        workspace_id,
                        page.id,
                        &block_id,
                    )
                    .await?
                    .context("portable template block missing")?;
                    let code = FrontstagePageRepository::get_frontstage_block_code(
                        self,
                        workspace_id,
                        page.id,
                        &block.code_ref,
                    )
                    .await?
                    .context("portable template source code missing")?;
                    blocks.push(PortableBlock {
                        block_id: block.block_id,
                        parent_block_id: block.parent_block_id,
                        rank: block.rank,
                        presentation: block.presentation,
                        title: block.title,
                        description: block.description,
                        code_ref: block.code_ref,
                        schema_version: block.schema_version,
                        input_mapping: block.input_mapping,
                        output_mapping: block.output_mapping,
                        runtime_descriptor: block.runtime_descriptor,
                        source_code: code.source_code,
                    });
                }
                tabs.push(PortableTab {
                    id: tab.id,
                    title: tab.title,
                    rank: tab.rank,
                    is_default: tab.is_default,
                    route_segment: tab.route_segment,
                    document_root_uid: tab.document_root_uid,
                    document_payload: detail.document.payload,
                    blocks,
                });
            }
            let visibility_rows = sqlx::query("select roles.code as role_code, rules.tab_id, rules.visibility from frontstage_page_visibility_rules rules join roles on roles.id = rules.role_id left join frontstage_page_tabs tabs on tabs.id = rules.tab_id where rules.workspace_id = $1 and (rules.page_id = $2 or tabs.page_id = $2)")
                .bind(workspace_id).bind(page.id).fetch_all(self.pool()).await?;
            let visibility_rules = visibility_rows
                .into_iter()
                .map(|row| {
                    let visibility: String = row.get("visibility");
                    Ok(PortablePageVisibilityRule {
                        role_code: row.get("role_code"),
                        tab_id: row.get("tab_id"),
                        visibility: domain::frontstage::FrontstagePageVisibility::from_db(&visibility)
                            .context("unknown page visibility")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            package.pages.push(PortablePage {
                id: page.id,
                parent_id: page.parent_id,
                kind: page.kind,
                title: page.title,
                icon: page.icon,
                tooltip: page.tooltip,
                is_hidden: page.is_hidden,
                placement: page.placement,
                content_presentation: page.content_presentation,
                slug: page.slug,
                rank: page.rank,
                tabs,
                visibility_rules,
            });
        }
        for plugin in PluginRepository::list_installations(self).await? {
            package.plugins.push(PortablePluginDependency {
                plugin_id: plugin.plugin_id,
                plugin_version: plugin.plugin_version,
                checksum: plugin.expected_checksum,
                contribution_code: None,
            });
        }
        Ok(package)
    }
}
