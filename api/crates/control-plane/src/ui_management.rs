use anyhow::{bail, Result};
use domain::{UiCodeTemplate, UiCodeTemplateLanguage, UiComponentRecord, UiComponentRecordOrigin};
use uuid::Uuid;

use crate::errors::ControlPlaneError;
use crate::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, ReviseUiCodeTemplateInput,
    UiComponentRecordPatch,
};
use crate::ports::{FrontendBlockCatalogRepository, UiManagementRepository};

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

#[derive(Debug, Clone)]
pub struct ListUiComponentRecordsQuery {
    pub query: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct UiComponentRecordPage {
    pub items: Vec<UiComponentRecord>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
}

pub struct UiManagementService<R> {
    repository: R,
    node_id: String,
}

impl<R> UiManagementService<R>
where
    R: UiManagementRepository + FrontendBlockCatalogRepository,
{
    pub fn new(repository: R, node_id: impl Into<String>) -> Self {
        Self {
            repository,
            node_id: node_id.into(),
        }
    }

    pub async fn list_templates(
        &self,
        include_archived: bool,
    ) -> Result<(Vec<OfficialUiCodeTemplate>, Vec<UiCodeTemplate>)> {
        let blocks = self
            .repository
            .list_system_frontend_blocks(&self.node_id)
            .await?;
        let managed = self
            .repository
            .list_ui_code_templates(include_archived)
            .await?;
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
        Ok((official, managed))
    }

    pub async fn list_published_templates_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<EffectiveUiCodeTemplate>> {
        let blocks = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, workspace_id)
            .await?;
        let managed = self.repository.list_ui_code_templates(false).await?;
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
                    template.provider_code == block.provider_code
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
        self.repository.revise_ui_code_template(&input).await
    }

    pub async fn publish_template(
        &self,
        template_id: Uuid,
        revision: i32,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        self.repository
            .publish_ui_code_template_revision(template_id, revision, actor_user_id)
            .await
    }

    pub async fn set_template_default(&self, template_id: Uuid, actor_user_id: Uuid) -> Result<()> {
        self.repository
            .set_ui_code_template_default(template_id, actor_user_id)
            .await
    }

    pub async fn reset_template_default(
        &self,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<()> {
        self.repository
            .reset_ui_code_template_default(provider_code, contribution_code)
            .await
    }

    pub async fn set_template_archived(
        &self,
        template_id: Uuid,
        archived: bool,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        self.repository
            .set_ui_code_template_archived(template_id, archived, actor_user_id)
            .await
    }

    pub async fn list_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        self.repository.list_ui_component_records().await
    }

    pub async fn list_component_records_page(
        &self,
        query: ListUiComponentRecordsQuery,
    ) -> Result<UiComponentRecordPage> {
        let mut records = self.repository.list_ui_component_records().await?;
        if let Some(search) = query
            .query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let search = search.to_lowercase();
            records.retain(|record| component_record_matches(record, &search));
        }
        records.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.component_code.cmp(&right.component_code))
        });
        let total = records.len();
        let offset = query.offset.min(total);
        let limit = query.limit.max(1);
        let end = offset.saturating_add(limit).min(total);
        let has_more = end < total;
        Ok(UiComponentRecordPage {
            items: records[offset..end].to_vec(),
            total,
            offset,
            limit,
            has_more,
            next_offset: has_more.then_some(end),
        })
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
            return Err(ControlPlaneError::Conflict("official_ui_component_read_only").into());
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
            return Err(ControlPlaneError::Conflict("official_ui_component_read_only").into());
        }
        if self.repository.delete_ui_component_record(id).await? {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("ui_component_record").into())
        }
    }
}

fn component_record_matches(record: &UiComponentRecord, search: &str) -> bool {
    [
        record.component_code.as_str(),
        record.name.as_str(),
        record.description.as_str(),
        record.source.as_str(),
        record.group.as_str(),
        record.upstream.identity.as_str(),
        record.upstream.version.as_str(),
        record.version.as_str(),
    ]
    .iter()
    .any(|value| value.to_lowercase().contains(search))
        || record
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().contains(search))
}
