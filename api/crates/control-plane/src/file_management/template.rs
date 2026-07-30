use anyhow::Result;
use domain::{CatalogLocale, CatalogMessageIdentity, ModelFieldKind};
use uuid::Uuid;

use crate::{i18n_catalog::CatalogResolver, ports::CatalogResolutionRepository};

pub const ATTACHMENTS_DEFAULT_TITLE: &str = "Attachments";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadataTitleReference {
    pub resource_code: &'static str,
    pub field_code: Option<&'static str>,
    pub key: &'static str,
    pub historical_default: &'static str,
}

#[derive(Debug, Clone)]
pub struct FileFieldTemplate {
    pub code: String,
    pub title: String,
    pub historical_title: String,
    pub field_kind: ModelFieldKind,
    pub is_required: bool,
}

const ATTACHMENTS_FIELDS: [(&str, &str, &str, ModelFieldKind, bool); 9] = [
    ("title", "Title", "标题", ModelFieldKind::String, false),
    (
        "filename",
        "Filename",
        "文件名",
        ModelFieldKind::String,
        true,
    ),
    (
        "extname",
        "Extension",
        "扩展名",
        ModelFieldKind::String,
        false,
    ),
    ("size", "Size", "大小", ModelFieldKind::Number, true),
    (
        "mimetype",
        "MIME Type",
        "MIME 类型",
        ModelFieldKind::String,
        true,
    ),
    (
        "path",
        "Storage Path",
        "存储路径",
        ModelFieldKind::String,
        true,
    ),
    ("meta", "Metadata", "元数据", ModelFieldKind::Json, true),
    (
        "url",
        "Cached URL",
        "缓存地址",
        ModelFieldKind::String,
        false,
    ),
    (
        "storage_id",
        "Storage ID",
        "存储器 ID",
        ModelFieldKind::String,
        true,
    ),
];

pub fn attachments_template_fields() -> Vec<FileFieldTemplate> {
    ATTACHMENTS_FIELDS
        .iter()
        .map(
            |(code, title, historical_title, field_kind, is_required)| FileFieldTemplate {
                code: (*code).into(),
                title: (*title).into(),
                historical_title: (*historical_title).into(),
                field_kind: *field_kind,
                is_required: *is_required,
            },
        )
        .collect()
}

pub fn file_metadata_title_references() -> Vec<FileMetadataTitleReference> {
    std::iter::once(FileMetadataTitleReference {
        resource_code: "attachments",
        field_code: None,
        key: ATTACHMENTS_DEFAULT_TITLE,
        historical_default: ATTACHMENTS_DEFAULT_TITLE,
    })
    .chain(
        ATTACHMENTS_FIELDS
            .iter()
            .map(
                |(code, title, historical_title, _, _)| FileMetadataTitleReference {
                    resource_code: "attachments",
                    field_code: Some(*code),
                    key: *title,
                    historical_default: *historical_title,
                },
            ),
    )
    .collect()
}

pub async fn project_attachments_model_titles<R>(
    resolver: &CatalogResolver<R>,
    workspace_id: Uuid,
    locale: &CatalogLocale,
    model: &mut domain::ModelDefinitionRecord,
) -> Result<()>
where
    R: CatalogResolutionRepository,
{
    let is_attachments = domain::builtin_contract_for_model(model)
        .is_some_and(|contract| contract.code == "attachments");
    if !is_attachments {
        return Ok(());
    }

    if title_uses_builtin_default(
        &model.title,
        ATTACHMENTS_DEFAULT_TITLE,
        ATTACHMENTS_DEFAULT_TITLE,
    ) {
        model.title =
            resolve_builtin_title(resolver, workspace_id, locale, ATTACHMENTS_DEFAULT_TITLE)
                .await?;
    }
    let templates = attachments_template_fields();
    for field in &mut model.fields {
        let Some(template) = templates
            .iter()
            .find(|template| template.code == field.code)
        else {
            continue;
        };
        if title_uses_builtin_default(&field.title, &template.title, &template.historical_title) {
            field.title =
                resolve_builtin_title(resolver, workspace_id, locale, &template.title).await?;
        }
    }
    Ok(())
}

pub async fn project_builtin_file_table_title<R>(
    resolver: &CatalogResolver<R>,
    workspace_id: Uuid,
    locale: &CatalogLocale,
    table: &mut domain::FileTableRecord,
) -> Result<()>
where
    R: CatalogResolutionRepository,
{
    if table.is_builtin
        && table.is_default
        && title_uses_builtin_default(
            &table.title,
            ATTACHMENTS_DEFAULT_TITLE,
            ATTACHMENTS_DEFAULT_TITLE,
        )
    {
        table.title =
            resolve_builtin_title(resolver, workspace_id, locale, ATTACHMENTS_DEFAULT_TITLE)
                .await?;
    }
    Ok(())
}

fn title_uses_builtin_default(
    persisted: &str,
    canonical_english: &str,
    historical_default: &str,
) -> bool {
    // Without provenance, exact equality with a known shipped default is the only safe
    // upgrade signal. Any other non-empty value remains user-owned metadata verbatim.
    persisted.trim().is_empty() || persisted == canonical_english || persisted == historical_default
}

async fn resolve_builtin_title<R>(
    resolver: &CatalogResolver<R>,
    workspace_id: Uuid,
    locale: &CatalogLocale,
    key: &str,
) -> Result<String>
where
    R: CatalogResolutionRepository,
{
    let identity =
        CatalogMessageIdentity::new(key).expect("file metadata title key must be non-empty");
    Ok(resolver
        .resolve(workspace_id, &identity, locale)
        .await?
        .value)
}
