use std::collections::BTreeMap;

use anyhow::{bail, Result};
use domain::{
    FrontendComponentContract, UiCodeTemplate, UiCodeTemplateLanguage, UiComponentLocator,
    UiComponentOverride, UiComponentOverrideState,
};
use uuid::Uuid;

use crate::ports::{
    CreateUiCodeTemplateInput, ReviseUiCodeTemplateInput, ReviseUiComponentContractInput,
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
pub struct UiComponentCandidate {
    pub installation_id: Uuid,
    pub plugin_id: String,
    pub plugin_version: String,
    pub locator: UiComponentLocator,
    pub module_version: String,
    pub exports: Vec<String>,
    pub binding: domain::FrontendModuleBinding,
    pub assets: Vec<domain::FrontendModuleAsset>,
    pub type_declarations: String,
    pub official_contract: Option<FrontendComponentContract>,
    pub effective_contract: Option<FrontendComponentContract>,
    pub override_record: Option<UiComponentOverride>,
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

    pub async fn list_component_candidates(&self) -> Result<Vec<UiComponentCandidate>> {
        let blocks = self
            .repository
            .list_system_frontend_blocks(&self.node_id)
            .await?;
        self.merge_component_candidates(blocks).await
    }

    pub async fn list_effective_components_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<UiComponentCandidate>> {
        let blocks = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, workspace_id)
            .await?;
        Ok(self
            .merge_component_candidates(blocks)
            .await?
            .into_iter()
            .filter(|candidate| candidate.effective_contract.is_some())
            .collect())
    }

    async fn merge_component_candidates(
        &self,
        blocks: Vec<domain::FrontendBlockCatalogEntry>,
    ) -> Result<Vec<UiComponentCandidate>> {
        let overrides = self.repository.list_ui_component_overrides().await?;
        let override_map = overrides
            .into_iter()
            .map(|value| {
                (
                    (
                        value.locator.provider_code.clone(),
                        value.locator.contribution_code.clone(),
                        value.locator.module_source.clone(),
                        value.locator.export_name.clone(),
                    ),
                    value,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut candidates = Vec::new();
        for block in blocks {
            for module in block.code_modules {
                for export_name in &module.exports {
                    let locator = UiComponentLocator {
                        provider_code: block.provider_code.clone(),
                        contribution_code: block.contribution_code.clone(),
                        module_source: module.source.clone(),
                        export_name: export_name.clone(),
                    };
                    let official_contract = module
                        .components
                        .iter()
                        .find(|contract| &contract.export_name == export_name)
                        .cloned();
                    let record = override_map
                        .get(&(
                            locator.provider_code.clone(),
                            locator.contribution_code.clone(),
                            locator.module_source.clone(),
                            locator.export_name.clone(),
                        ))
                        .cloned();
                    let effective_contract = match record.as_ref().map(|value| value.state) {
                        Some(UiComponentOverrideState::Hidden) => None,
                        Some(UiComponentOverrideState::Published) => record
                            .as_ref()
                            .and_then(|value| value.published_revision.as_ref())
                            .map(|value| value.contract.clone()),
                        Some(UiComponentOverrideState::Inherit) | None => official_contract.clone(),
                    };
                    candidates.push(UiComponentCandidate {
                        installation_id: block.installation_id,
                        plugin_id: block.plugin_id.clone(),
                        plugin_version: block.plugin_version.clone(),
                        locator,
                        module_version: module.version.clone(),
                        exports: module.exports.clone(),
                        binding: module.binding,
                        assets: module.assets.clone(),
                        type_declarations: module.type_declarations.clone(),
                        official_contract,
                        effective_contract,
                        override_record: record,
                    });
                }
            }
        }
        candidates.sort_by(|a, b| {
            a.locator
                .module_source
                .cmp(&b.locator.module_source)
                .then_with(|| a.locator.export_name.cmp(&b.locator.export_name))
        });
        Ok(candidates)
    }

    pub async fn assert_component_locator_available(
        &self,
        locator: &UiComponentLocator,
    ) -> Result<()> {
        if self
            .list_component_candidates()
            .await?
            .iter()
            .any(|candidate| &candidate.locator == locator)
        {
            Ok(())
        } else {
            bail!("component module/export is not currently executable")
        }
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

    pub async fn revise_component_contract(
        &self,
        input: ReviseUiComponentContractInput,
    ) -> Result<UiComponentOverride> {
        self.assert_component_locator_available(&input.locator)
            .await?;
        self.repository.revise_ui_component_contract(&input).await
    }

    pub async fn set_component_state(
        &self,
        locator: &UiComponentLocator,
        state: UiComponentOverrideState,
        actor_user_id: Uuid,
    ) -> Result<UiComponentOverride> {
        self.assert_component_locator_available(locator).await?;
        self.repository
            .set_ui_component_state(locator, state, actor_user_id)
            .await
    }
}
