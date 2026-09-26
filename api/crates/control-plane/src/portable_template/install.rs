//! Sequential installation through resource owners. A failed owner is not a rollback.
use super::*;
use crate::ports::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;
mod applications;
mod models;
mod pages;
mod visibility;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortableTemplateInstallResult {
    pub complete: bool,
    pub created: Vec<PortableTemplateCreatedResource>,
    pub updated: Vec<PortableTemplateCreatedResource>,
    pub id_map: BTreeMap<String, String>,
    pub failures: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableTemplateCreatedResource {
    pub kind: String,
    pub source_id: String,
    pub target_id: String,
}
impl PortableTemplateInstallResult {
    fn created(&mut self, kind: &str, source: impl ToString, target: impl ToString) {
        let (source_id, target_id) = (source.to_string(), target.to_string());
        self.id_map.insert(source_id.clone(), target_id.clone());
        self.created.push(PortableTemplateCreatedResource {
            kind: kind.into(),
            source_id,
            target_id,
        });
    }
    fn mapped(&self, id: Uuid) -> Result<Uuid> {
        self.id_map
            .get(&id.to_string())
            .context("missing portable identity mapping")?
            .parse()
            .map_err(Into::into)
    }
    fn updated(&mut self, kind: &str, source: impl ToString, target: impl ToString) {
        let (source_id, target_id) = (source.to_string(), target.to_string());
        self.id_map.insert(source_id.clone(), target_id.clone());
        self.updated.push(PortableTemplateCreatedResource {
            kind: kind.into(),
            source_id,
            target_id,
        });
    }
    fn value(&self, mut value: serde_json::Value) -> serde_json::Value {
        rewrite_template_value(&mut value, &self.id_map);
        value
    }
}
/// Repository composition is supplied by the host; no storage writes bypass owners.
pub trait PortableTemplateInstallRepository:
    ApplicationRepository
    + PortableTemplateReadRepository
    + PortableTemplateIdentityRepository
    + McpManagementRepository
    + ModelDefinitionRepository
    + FlowRepository
    + ApplicationApiMappingRepository
    + ApplicationPublicationRepository
    + ApplicationCompiledPlanRepository
    + ApplicationCompileContextRepository
    + ApplicationJsDependencySelectionRepository
    + WorkflowScheduleTriggerRepository
    + FrontstagePageRepository
    + FrontstageBlockTreeRepository
    + FrontendBlockCatalogRepository
    + RoleRepository
    + RoleConsolePolicyReader
    + Clone
{
}
impl<T> PortableTemplateInstallRepository for T where
    T: ApplicationRepository
        + PortableTemplateReadRepository
        + PortableTemplateIdentityRepository
        + McpManagementRepository
        + ModelDefinitionRepository
        + FlowRepository
        + ApplicationApiMappingRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationCompileContextRepository
        + ApplicationJsDependencySelectionRepository
        + WorkflowScheduleTriggerRepository
        + FrontstagePageRepository
        + FrontstageBlockTreeRepository
        + FrontendBlockCatalogRepository
        + RoleRepository
        + RoleConsolePolicyReader
        + Clone
{
}

pub struct PortableTemplateInstallService<R> {
    repository: R,
    node_id: Option<String>,
}
impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    async fn record_created(
        &self,
        actor: &domain::ActorContext,
        result: &mut PortableTemplateInstallResult,
        kind: &str,
        source: impl ToString,
        target: impl ToString,
    ) -> Result<()> {
        let source = source.to_string();
        let target = target.to_string();
        self.repository
            .record_portable_template_identity(actor.current_workspace_id, kind, &source, &target)
            .await?;
        result.created(kind, source, target);
        Ok(())
    }
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            node_id: None,
        }
    }
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }
    fn page_owner(
        &self,
        actor: &domain::ActorContext,
    ) -> crate::frontstage::FrontstagePageService<R> {
        let owner = crate::frontstage::FrontstagePageService::for_actor(
            self.repository.clone(),
            actor.clone(),
        );
        match &self.node_id {
            Some(id) => owner.with_node_id(id.clone()),
            None => owner,
        }
    }
    /// Host resolves and verifies plugin dependencies before calling this command.
    pub async fn install(
        &self,
        actor_user_id: Uuid,
        package: PortableTemplatePackage,
    ) -> Result<PortableTemplateInstallResult> {
        let preview = PortableTemplateService::new(self.repository.clone())
            .preview(actor_user_id, &package)
            .await?;
        anyhow::ensure!(
            preview.valid,
            "portable template preflight: {}",
            preview.failures.join("; ")
        );
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        let target = self
            .repository
            .portable_template_snapshot(actor_user_id, actor.current_workspace_id)
            .await?;
        self.preflight_visibility(&actor, &package).await?;
        let mut result = PortableTemplateInstallResult::default();
        let source_keys = super::identity::installed_source_keys(&package);
        result.id_map = self
            .repository
            .load_portable_template_identity_map(actor.current_workspace_id)
            .await?
            .into_iter()
            .filter(|(source, _)| source_keys.contains(source))
            .collect();
        self.map_frontend_installations(&actor, &package, &mut result)
            .await?;
        // Acknowledged owner calls may commit before a later audit/cache operation fails.
        // Consequently failures explicitly require inspecting the target before retry.
        let outcome = async {
            self.install_models(&actor, &package, &target, &mut result)
                .await
                .context("data models")?;
            self.create_applications(&actor, &package, &target, &mut result)
                .await
                .context("application creation")?;
            self.create_pages(&actor, &package, &target, &mut result)
                .await
                .context("page creation")?;
            self.fill_model_fields(&actor, &package, &result)
                .await
                .context("field references")?;
            self.fill_applications(&actor, &package, &mut result)
                .await
                .context("application drafts/publications")?;
            self.fill_pages(&actor, &package, &mut result)
                .await
                .context("page content")?;
            self.install_visibility(&actor, &package, &result)
                .await
                .context("page visibility")?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        match outcome { Ok(())=>result.complete=true, Err(error)=>result.failures.push(format!("{error:#}; installation stopped. Owners may have committed the failing resource; inspect target before retry.")) }
        Ok(result)
    }
}
