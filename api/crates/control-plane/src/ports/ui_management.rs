use anyhow::Result;
use async_trait::async_trait;
use domain::{
    FrontendComponentContract, UiCodeTemplate, UiCodeTemplateLanguage, UiComponentLocator,
    UiComponentOverride, UiComponentOverrideState, UiComponentRecord, UiComponentRecordUpstream,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CreateUiCodeTemplateInput {
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub source: String,
    pub language: UiCodeTemplateLanguage,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ReviseUiCodeTemplateInput {
    pub template_id: Uuid,
    pub name: String,
    pub source: String,
    pub language: UiCodeTemplateLanguage,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ReviseUiComponentContractInput {
    pub locator: UiComponentLocator,
    pub contract: FrontendComponentContract,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateUiComponentRecordInput {
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub source: String,
    pub group: String,
    pub upstream: UiComponentRecordUpstream,
    pub version: String,
    pub keywords: Vec<String>,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UiComponentRecordPatch {
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub source: String,
    pub group: String,
    pub upstream: UiComponentRecordUpstream,
    pub version: String,
    pub keywords: Vec<String>,
    pub actor_user_id: Uuid,
}

#[async_trait]
pub trait UiManagementRepository: Send + Sync {
    async fn list_ui_code_templates(&self, include_archived: bool) -> Result<Vec<UiCodeTemplate>>;
    async fn get_ui_code_template(&self, template_id: Uuid) -> Result<Option<UiCodeTemplate>>;
    async fn create_ui_code_template(
        &self,
        input: &CreateUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate>;
    async fn revise_ui_code_template(
        &self,
        input: &ReviseUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate>;
    async fn publish_ui_code_template_revision(
        &self,
        template_id: Uuid,
        revision: i32,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate>;
    async fn set_ui_code_template_default(
        &self,
        template_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<()>;
    async fn reset_ui_code_template_default(
        &self,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<()>;
    async fn set_ui_code_template_archived(
        &self,
        template_id: Uuid,
        archived: bool,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate>;

    async fn list_ui_component_overrides(&self) -> Result<Vec<UiComponentOverride>>;
    async fn get_ui_component_override(
        &self,
        locator: &UiComponentLocator,
    ) -> Result<Option<UiComponentOverride>>;
    async fn revise_ui_component_contract(
        &self,
        input: &ReviseUiComponentContractInput,
    ) -> Result<UiComponentOverride>;
    async fn set_ui_component_state(
        &self,
        locator: &UiComponentLocator,
        state: UiComponentOverrideState,
        actor_user_id: Uuid,
    ) -> Result<UiComponentOverride>;

    async fn list_ui_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        Err(anyhow::anyhow!(
            "ui component record repository is unavailable"
        ))
    }
    async fn get_ui_component_record(&self, _id: Uuid) -> Result<Option<UiComponentRecord>> {
        Err(anyhow::anyhow!(
            "ui component record repository is unavailable"
        ))
    }
    async fn create_ui_component_record(
        &self,
        _input: &CreateUiComponentRecordInput,
    ) -> Result<UiComponentRecord> {
        Err(anyhow::anyhow!(
            "ui component record repository is unavailable"
        ))
    }
    async fn update_ui_component_record(
        &self,
        _id: Uuid,
        _patch: &UiComponentRecordPatch,
    ) -> Result<UiComponentRecord> {
        Err(anyhow::anyhow!(
            "ui component record repository is unavailable"
        ))
    }
    async fn delete_ui_component_record(&self, _id: Uuid) -> Result<bool> {
        Err(anyhow::anyhow!(
            "ui component record repository is unavailable"
        ))
    }
}
