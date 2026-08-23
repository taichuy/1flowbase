use anyhow::Result;
use async_trait::async_trait;
use domain::{
    UiCodeTemplate, UiCodeTemplateLanguage, UiComponentRecord, UiComponentRecordUpstream,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialUiComponentCatalogRecord {
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
    pub catalog_updated_at: OffsetDateTime,
    pub source_locator: String,
    pub source_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogIndex {
    pub catalog_version: String,
    pub generated_at: OffsetDateTime,
    pub page_size: usize,
    pub total_components: usize,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogPage {
    pub catalog_version: String,
    pub total_components: usize,
    pub page_size: usize,
    pub page: u32,
    pub cursor: String,
    pub next_cursor: Option<String>,
    pub records: Vec<OfficialUiComponentCatalogRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogSearchEntry {
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub group: String,
    pub upstream: UiComponentRecordUpstream,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogSearchResult {
    pub catalog_version: String,
    pub page: u32,
    pub page_size: usize,
    pub total_entries: usize,
    pub entries: Vec<UiComponentCatalogSearchEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentCatalogSeed {
    pub catalog_version: String,
    pub source_fingerprint: String,
    pub records: Vec<OfficialUiComponentCatalogRecord>,
}

#[async_trait]
pub trait UiComponentCatalogSource: Send + Sync {
    async fn index(&self) -> Result<UiComponentCatalogIndex>;
    async fn page(&self, page: u32) -> Result<UiComponentCatalogPage>;
    async fn search(
        &self,
        query: &str,
        page: u32,
        page_size: usize,
    ) -> Result<UiComponentCatalogSearchResult>;
    async fn seed(&self) -> Result<UiComponentCatalogSeed>;
}

#[async_trait]
pub trait UiComponentCatalogRepository: Send + Sync {
    async fn count_ui_component_records(&self) -> Result<usize>;
    async fn list_official_ui_component_records(&self) -> Result<Vec<UiComponentRecord>>;
    async fn upsert_official_ui_component_record(
        &self,
        record: &OfficialUiComponentCatalogRecord,
        actor_user_id: Uuid,
    ) -> Result<()>;
    async fn replace_official_ui_component_source_group(
        &self,
        source: &str,
        group: &str,
        records: &[OfficialUiComponentCatalogRecord],
        actor_user_id: Uuid,
    ) -> Result<()>;
    async fn replace_official_ui_component_catalog_groups(
        &self,
        records: &[OfficialUiComponentCatalogRecord],
        actor_user_id: Uuid,
    ) -> Result<bool>;
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

    async fn list_ui_component_records(&self) -> Result<Vec<UiComponentRecord>>;
    async fn get_ui_component_record(&self, id: Uuid) -> Result<Option<UiComponentRecord>>;
    async fn create_ui_component_record(
        &self,
        input: &CreateUiComponentRecordInput,
    ) -> Result<UiComponentRecord>;
    async fn update_ui_component_record(
        &self,
        id: Uuid,
        patch: &UiComponentRecordPatch,
    ) -> Result<UiComponentRecord>;
    async fn delete_ui_component_record(&self, id: Uuid) -> Result<bool>;
}
