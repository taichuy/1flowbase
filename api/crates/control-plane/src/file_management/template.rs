use anyhow::Result;
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId, ModelFieldKind};
use uuid::Uuid;

use crate::{i18n_catalog::CatalogResolver, ports::CatalogResolutionRepository};

pub const FILE_MANAGEMENT_CATALOG_MODULE: &str = "@taichuy/platform/file-management";
pub const ATTACHMENTS_DEFAULT_TITLE: &str = "Attachments";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadataTitleReference {
    pub resource_code: &'static str,
    pub field_code: Option<&'static str>,
    pub module: &'static str,
    pub msgid: &'static str,
}

#[derive(Debug, Clone)]
pub struct FileFieldTemplate {
    pub code: String,
    pub title: String,
    pub field_kind: ModelFieldKind,
    pub is_required: bool,
}

const ATTACHMENTS_FIELDS: [(&str, &str, ModelFieldKind, bool); 9] = [
    ("title", "Title", ModelFieldKind::String, false),
    ("filename", "Filename", ModelFieldKind::String, true),
    ("extname", "Extension", ModelFieldKind::String, false),
    ("size", "Size", ModelFieldKind::Number, true),
    ("mimetype", "MIME Type", ModelFieldKind::String, true),
    ("path", "Storage Path", ModelFieldKind::String, true),
    ("meta", "Metadata", ModelFieldKind::Json, true),
    ("url", "Cached URL", ModelFieldKind::String, false),
    ("storage_id", "Storage ID", ModelFieldKind::String, true),
];

pub fn attachments_template_fields() -> Vec<FileFieldTemplate> {
    ATTACHMENTS_FIELDS
        .iter()
        .map(|(code, title, field_kind, is_required)| FileFieldTemplate {
            code: (*code).into(),
            title: (*title).into(),
            field_kind: *field_kind,
            is_required: *is_required,
        })
        .collect()
}

pub fn file_metadata_title_references() -> Vec<FileMetadataTitleReference> {
    std::iter::once(FileMetadataTitleReference {
        resource_code: "attachments",
        field_code: None,
        module: FILE_MANAGEMENT_CATALOG_MODULE,
        msgid: ATTACHMENTS_DEFAULT_TITLE,
    })
    .chain(
        ATTACHMENTS_FIELDS
            .iter()
            .map(|(code, title, _, _)| FileMetadataTitleReference {
                resource_code: "attachments",
                field_code: Some(*code),
                module: FILE_MANAGEMENT_CATALOG_MODULE,
                msgid: *title,
            }),
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

    if title_uses_builtin_default(&model.title, ATTACHMENTS_DEFAULT_TITLE) {
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
        if title_uses_builtin_default(&field.title, &template.title) {
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
        && title_uses_builtin_default(&table.title, ATTACHMENTS_DEFAULT_TITLE)
    {
        table.title =
            resolve_builtin_title(resolver, workspace_id, locale, ATTACHMENTS_DEFAULT_TITLE)
                .await?;
    }
    Ok(())
}

fn title_uses_builtin_default(persisted: &str, canonical_english: &str) -> bool {
    persisted.trim().is_empty() || persisted == canonical_english
}

async fn resolve_builtin_title<R>(
    resolver: &CatalogResolver<R>,
    workspace_id: Uuid,
    locale: &CatalogLocale,
    msgid: &str,
) -> Result<String>
where
    R: CatalogResolutionRepository,
{
    let identity = CatalogMessageIdentity::new(
        CatalogModuleId::new(FILE_MANAGEMENT_CATALOG_MODULE)
            .expect("file management catalog module must be valid"),
        msgid,
    )
    .expect("file metadata title msgid must be non-empty");
    Ok(resolver
        .resolve(workspace_id, &identity, locale)
        .await?
        .value)
}
