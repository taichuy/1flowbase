use anyhow::{bail, Result};
use domain::{UiCodeTemplate, UiCodeTemplateLanguage, UiComponentRecord, UiComponentRecordOrigin};
use uuid::Uuid;

use crate::errors::ControlPlaneError;
use crate::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, ReviseUiCodeTemplateInput,
    UiComponentRecordPatch,
};
use crate::ports::{FrontendBlockCatalogRepository, UiManagementRepository};

pub const DEFAULT_UI_TEMPLATE_PROVIDER_CODE: &str = "1flowbase";
pub const DEFAULT_UI_TEMPLATE_CONTRIBUTION_CODE: &str = "frontstage.js-ui-block";

#[derive(Debug, Clone)]
pub struct UiCodeTemplateList {
    pub official: Vec<OfficialUiCodeTemplate>,
    pub managed: Vec<UiCodeTemplate>,
    pub default_template: Option<OfficialUiCodeTemplate>,
}

#[derive(Debug, Clone)]
pub struct OfficialUiCodeTemplate {
    pub provider_code: String,
    pub contribution_code: String,
    pub title: String,
    pub source: String,
    pub language: UiCodeTemplateLanguage,
    pub version: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct EffectiveUiCodeTemplate {
    pub template_id: Option<Uuid>,
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub source: String,
    pub language: UiCodeTemplateLanguage,
    pub version: String,
    pub is_official: bool,
    pub is_default: bool,
}

pub use crate::ports::{ListUiComponentRecordsQuery, UiComponentRecordPage};

pub struct UiManagementService<R> {
    repository: R,
    node_id: String,
    native_targets: Vec<domain::NativePluginTarget>,
}

impl<R> UiManagementService<R>
where
    R: UiManagementRepository + FrontendBlockCatalogRepository,
{
    pub fn new(repository: R, node_id: impl Into<String>) -> Self {
        Self {
            repository,
            node_id: node_id.into(),
            native_targets: Vec::new(),
        }
    }

    pub fn with_native_targets(mut self, targets: Vec<domain::NativePluginTarget>) -> Self {
        self.native_targets = targets;
        self
    }
    async fn assert_template_available(&self, template_id: Uuid) -> Result<()> {
        let template = self
            .repository
            .get_ui_code_template(template_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("ui_code_template"))?;
        if template.owner_plugin_code.is_none() {
            return Ok(());
        }
        if !self
            .repository
            .native_template_is_available(template_id, &self.native_targets)
            .await?
        {
            return Err(ControlPlaneError::Conflict("native_plugin_version_mismatch").into());
        }
        Ok(())
    }
    pub async fn plugin_settings_template(
        &self,
        owner: &str,
        contribution_code: &str,
    ) -> Result<UiCodeTemplate> {
        let template = self
            .repository
            .list_ui_code_templates()
            .await?
            .into_iter()
            .find(|t| {
                t.owner_plugin_code.as_deref() == Some(owner)
                    && t.contribution_code == contribution_code
                    && t.is_default
                    && t.published_revision.is_some()
            })
            .ok_or(ControlPlaneError::NotFound("plugin_settings_template"))?;
        self.assert_template_available(template.id).await?;
        Ok(template)
    }
    pub async fn list_templates(&self) -> Result<UiCodeTemplateList> {
        let blocks = self
            .repository
            .list_system_frontend_blocks(&self.node_id)
            .await?;
        let managed = self.repository.list_ui_code_templates().await?;
        let mut available = Vec::new();
        for template in managed {
            if template.owner_plugin_code.is_none()
                || self
                    .repository
                    .native_template_is_available(template.id, &self.native_targets)
                    .await?
            {
                available.push(template);
            }
        }
        let managed = available;
        let mut official = blocks
            .into_iter()
            .filter_map(|block| {
                Some(OfficialUiCodeTemplate {
                    provider_code: block.provider_code,
                    contribution_code: block.contribution_code,
                    title: block.title,
                    source: block.code_template?,
                    language: UiCodeTemplateLanguage::try_from(
                        block.code_template_language?.as_str(),
                    )
                    .ok()?,
                    version: block.code_template_version?,
                    is_default: false,
                })
            })
            .collect::<Vec<_>>();
        official.sort_by(|a, b| {
            a.title
                .cmp(&b.title)
                .then_with(|| a.contribution_code.cmp(&b.contribution_code))
        });
        for baseline in &mut official {
            baseline.is_default = !managed.iter().any(|template| {
                template.is_default
                    && template.provider_code == baseline.provider_code
                    && template.contribution_code == baseline.contribution_code
            });
        }
        let default_template = official
            .iter()
            .find(|template| {
                template.provider_code == DEFAULT_UI_TEMPLATE_PROVIDER_CODE
                    && template.contribution_code == DEFAULT_UI_TEMPLATE_CONTRIBUTION_CODE
            })
            .cloned();
        Ok(UiCodeTemplateList {
            official,
            managed,
            default_template,
        })
    }

    pub async fn list_published_templates_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<EffectiveUiCodeTemplate>> {
        let blocks = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, workspace_id)
            .await?;
        let managed = self.repository.list_ui_code_templates().await?;
        let mut templates = Vec::new();
        for block in blocks {
            let (Some(source), Some(language), Some(version)) = (
                block.code_template.clone(),
                block
                    .code_template_language
                    .as_deref()
                    .and_then(|value| UiCodeTemplateLanguage::try_from(value).ok()),
                block.code_template_version.clone(),
            ) else {
                continue;
            };
            let custom = managed
                .iter()
                .filter(|template| {
                    template.owner_plugin_code.is_none()
                        && template.provider_code == block.provider_code
                        && template.contribution_code == block.contribution_code
                        && template.published_revision.is_some()
                })
                .collect::<Vec<_>>();
            let has_custom_default = custom.iter().any(|template| template.is_default);
            templates.push(EffectiveUiCodeTemplate {
                template_id: None,
                provider_code: block.provider_code.clone(),
                contribution_code: block.contribution_code.clone(),
                name: block.title.clone(),
                source,
                language,
                version,
                is_official: true,
                is_default: !has_custom_default,
            });
            templates.extend(custom.into_iter().map(|template| {
                let revision = template
                    .published_revision
                    .as_ref()
                    .expect("filtered published revision");
                EffectiveUiCodeTemplate {
                    template_id: Some(template.id),
                    provider_code: template.provider_code.clone(),
                    contribution_code: template.contribution_code.clone(),
                    name: template.name.clone(),
                    source: revision.source.clone(),
                    language: revision.language,
                    version: format!("managed-r{}", revision.revision),
                    is_official: false,
                    is_default: template.is_default,
                }
            }));
        }
        Ok(templates)
    }

    pub async fn resolve_default_template(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<EffectiveUiCodeTemplate> {
        self.list_published_templates_for_workspace(workspace_id)
            .await?
            .into_iter()
            .find(|template| {
                template.provider_code == provider_code
                    && template.contribution_code == contribution_code
                    && template.is_default
            })
            .ok_or_else(|| anyhow::anyhow!("no effective code template for contribution"))
    }

    async fn assert_contribution_available(
        &self,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<()> {
        if self
            .repository
            .list_system_frontend_blocks(&self.node_id)
            .await?
            .iter()
            .any(|block| {
                block.provider_code == provider_code
                    && block.contribution_code == contribution_code
                    && block.code_template.is_some()
            })
        {
            Ok(())
        } else {
            bail!("frontend block contribution is not currently executable")
        }
    }

    pub async fn create_template(
        &self,
        input: CreateUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        self.assert_contribution_available(&input.provider_code, &input.contribution_code)
            .await?;
        self.repository.create_ui_code_template(&input).await
    }

    pub async fn revise_template(
        &self,
        input: ReviseUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        self.assert_template_available(input.template_id).await?;
        self.repository.revise_ui_code_template(&input).await
    }

    pub async fn publish_template(
        &self,
        template_id: Uuid,
        revision: i32,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        self.assert_template_available(template_id).await?;
        self.repository
            .publish_ui_code_template_revision(template_id, revision, actor_user_id)
            .await
    }

    pub async fn set_template_default(&self, template_id: Uuid, actor_user_id: Uuid) -> Result<()> {
        self.assert_template_available(template_id).await?;
        self.repository
            .set_ui_code_template_default(template_id, actor_user_id)
            .await
    }

    pub async fn reset_template_default(
        &self,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<()> {
        for template in self.repository.list_ui_code_templates().await? {
            if template.provider_code == provider_code
                && template.contribution_code == contribution_code
                && template.owner_plugin_code.is_some()
            {
                self.assert_template_available(template.id).await?;
            }
        }
        self.repository
            .reset_ui_code_template_default(provider_code, contribution_code)
            .await
    }

    pub async fn delete_template(&self, template_id: Uuid) -> Result<()> {
        self.assert_template_available(template_id).await?;
        if self.repository.delete_ui_code_template(template_id).await? {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("ui_code_template").into())
        }
    }

    pub async fn list_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        self.repository.list_ui_component_records().await
    }

    pub async fn list_component_records_page(
        &self,
        query: ListUiComponentRecordsQuery,
    ) -> Result<UiComponentRecordPage> {
        self.repository.list_ui_component_records_page(query).await
    }

    pub async fn get_component_record(&self, id: Uuid) -> Result<UiComponentRecord> {
        self.repository
            .get_ui_component_record(id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("ui_component_record").into())
    }

    pub async fn create_component_record(
        &self,
        input: CreateUiComponentRecordInput,
    ) -> Result<UiComponentRecord> {
        domain::validate_ui_component_record_fields(
            &input.component_code,
            &input.name,
            &input.description,
            &input.import_code,
            &input.source_code,
            UiComponentRecordOrigin::Custom,
            &input.source,
            &input.group,
            &input.upstream,
            &input.version,
            &input.keywords,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("ui_component_record"))?;
        self.repository.create_ui_component_record(&input).await
    }

    pub async fn update_component_record(
        &self,
        id: Uuid,
        patch: UiComponentRecordPatch,
    ) -> Result<UiComponentRecord> {
        let current = self.get_component_record(id).await?;
        if current.origin != UiComponentRecordOrigin::Custom {
            return Err(ControlPlaneError::Conflict(
                if current.origin == UiComponentRecordOrigin::Plugin {
                    "plugin_ui_component_read_only"
                } else {
                    "official_ui_component_read_only"
                },
            )
            .into());
        }
        domain::validate_ui_component_record_fields(
            &current.component_code,
            &patch.name,
            &patch.description,
            &patch.import_code,
            &patch.source_code,
            current.origin,
            &patch.source,
            &patch.group,
            &patch.upstream,
            &patch.version,
            &patch.keywords,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("ui_component_record"))?;
        self.repository.update_ui_component_record(id, &patch).await
    }

    pub async fn delete_component_record(&self, id: Uuid) -> Result<()> {
        let current = self.get_component_record(id).await?;
        if current.origin != UiComponentRecordOrigin::Custom {
            return Err(ControlPlaneError::Conflict(
                if current.origin == UiComponentRecordOrigin::Plugin {
                    "plugin_ui_component_read_only"
                } else {
                    "official_ui_component_read_only"
                },
            )
            .into());
        }
        if self.repository.delete_ui_component_record(id).await? {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("ui_component_record").into())
        }
    }
}

impl<R> UiManagementService<R>
where
    R: UiManagementRepository + FrontendBlockCatalogRepository + crate::ports::PluginRepository,
{
    async fn workspace_component_contributions(
        &self,
        workspace_id: Uuid,
        resolver: &crate::frontend_block_catalog::FrontendContributionResolver,
    ) -> Result<Vec<String>> {
        let entries = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, workspace_id)
            .await?;
        Ok(
            crate::frontend_block_catalog::resolve_frontend_contributions(
                &self.repository,
                &self.node_id,
                workspace_id,
                resolver,
                entries,
            )
            .await?
            .into_iter()
            .map(|binding| binding.block_id)
            .collect(),
        )
    }

    pub async fn list_workspace_component_records_page(
        &self,
        mut query: ListUiComponentRecordsQuery,
        workspace_id: Uuid,
        resolver: &crate::frontend_block_catalog::FrontendContributionResolver,
    ) -> Result<UiComponentRecordPage> {
        query.plugin_contribution_ids = Some(
            self.workspace_component_contributions(workspace_id, resolver)
                .await?,
        );
        self.repository.list_ui_component_records_page(query).await
    }

    pub async fn get_workspace_component_record(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        resolver: &crate::frontend_block_catalog::FrontendContributionResolver,
    ) -> Result<UiComponentRecord> {
        let record = self.get_component_record(id).await?;
        if record.origin == UiComponentRecordOrigin::Plugin {
            let allowed = self
                .workspace_component_contributions(workspace_id, resolver)
                .await?;
            let identity = record
                .owner_installation_id
                .zip(record.owner_contribution_code.as_deref())
                .map(|(id, contribution)| format!("{id}:{contribution}"));
            if !identity.is_some_and(|identity| allowed.contains(&identity)) {
                return Err(ControlPlaneError::NotFound("ui_component_record").into());
            }
        }
        Ok(record)
    }
}
