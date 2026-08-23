use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub const UI_CODE_TEMPLATE_SOURCE_LIMIT: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCodeTemplateLanguage {
    Jsx,
    Tsx,
}

impl UiCodeTemplateLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jsx => "jsx",
            Self::Tsx => "tsx",
        }
    }
}

impl TryFrom<&str> for UiCodeTemplateLanguage {
    type Error = UiManagementInvariantError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "jsx" => Ok(Self::Jsx),
            "tsx" => Ok(Self::Tsx),
            _ => Err(UiManagementInvariantError::InvalidTemplateLanguage),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCodeTemplateRevision {
    pub id: Uuid,
    pub template_id: Uuid,
    pub revision: i32,
    pub source: String,
    pub language: UiCodeTemplateLanguage,
    pub is_latest: bool,
    pub is_published: bool,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCodeTemplate {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub latest_revision: UiCodeTemplateRevision,
    pub published_revision: Option<UiCodeTemplateRevision>,
    pub is_default: bool,
    pub archived_at: Option<OffsetDateTime>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiComponentRecordOrigin {
    Official,
    Custom,
}

impl UiComponentRecordOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Custom => "custom",
        }
    }
}

impl TryFrom<&str> for UiComponentRecordOrigin {
    type Error = UiManagementInvariantError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "official" => Ok(Self::Official),
            "custom" => Ok(Self::Custom),
            _ => Err(UiManagementInvariantError::InvalidComponentOrigin),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentRecordUpstream {
    pub identity: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentRecord {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub origin: UiComponentRecordOrigin,
    pub source: String,
    pub group: String,
    pub upstream: UiComponentRecordUpstream,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_updated_at: Option<OffsetDateTime>,
    pub source_locator: Option<String>,
    pub source_checksum: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiManagementInvariantError {
    EmptyTemplateName,
    EmptyTemplateSource,
    TemplateSourceTooLarge,
    InvalidTemplateLanguage,
    EmptyComponentCode,
    EmptyComponentDescription,
    InvalidComponentCode,
    EmptyComponentName,
    EmptyComponentImportCode,
    EmptyComponentSourceCode,
    InvalidComponentOrigin,
    InvalidComponentSource,
    InvalidComponentGroup,
    EmptyComponentUpstreamIdentity,
    EmptyComponentUpstreamVersion,
    InvalidComponentVersion,
    InvalidComponentKeywords,
}

impl fmt::Display for UiManagementInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyTemplateName => "template name must not be empty",
            Self::EmptyTemplateSource => "template source must not be empty",
            Self::TemplateSourceTooLarge => "template source exceeds 262144 bytes",
            Self::InvalidTemplateLanguage => "template language must be jsx or tsx",
            Self::EmptyComponentCode => "component code must not be empty",
            Self::EmptyComponentDescription => "component description must not be empty",
            Self::InvalidComponentCode => "component code is invalid",
            Self::EmptyComponentName => "component name must not be empty",
            Self::EmptyComponentImportCode => "component import code must not be empty",
            Self::EmptyComponentSourceCode => "component source code must not be empty",
            Self::InvalidComponentOrigin => "component origin is invalid",
            Self::InvalidComponentSource => "component source is invalid",
            Self::InvalidComponentGroup => "component group is invalid",
            Self::EmptyComponentUpstreamIdentity => "component upstream identity must not be empty",
            Self::EmptyComponentUpstreamVersion => "component upstream version must not be empty",
            Self::InvalidComponentVersion => "component version must be semantic x.y.z",
            Self::InvalidComponentKeywords => "component keywords must be non-empty and unique",
        })
    }
}

fn valid_catalog_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_semver(value: &str) -> bool {
    let mut parts = value.split('.');
    matches!(parts.next(), Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        && matches!(parts.next(), Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        && matches!(parts.next(), Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        && parts.next().is_none()
}

#[allow(clippy::too_many_arguments)]
pub fn validate_ui_component_record_fields(
    component_code: &str,
    name: &str,
    description: &str,
    import_code: &str,
    source_code: &str,
    _origin: UiComponentRecordOrigin,
    source: &str,
    group: &str,
    upstream: &UiComponentRecordUpstream,
    version: &str,
    keywords: &[String],
) -> Result<(), UiManagementInvariantError> {
    if !valid_catalog_identifier(component_code) {
        return Err(UiManagementInvariantError::InvalidComponentCode);
    }
    if name.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentName);
    }
    if description.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentDescription);
    }
    // Code is intentionally opaque. Shape validation is limited to non-empty storage content.
    if import_code.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentImportCode);
    }
    if source_code.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentSourceCode);
    }
    if !valid_catalog_identifier(source) {
        return Err(UiManagementInvariantError::InvalidComponentSource);
    }
    if !valid_catalog_identifier(group) {
        return Err(UiManagementInvariantError::InvalidComponentGroup);
    }
    if upstream.identity.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentUpstreamIdentity);
    }
    if upstream.version.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentUpstreamVersion);
    }
    if !valid_semver(version) {
        return Err(UiManagementInvariantError::InvalidComponentVersion);
    }
    let mut unique = std::collections::BTreeSet::new();
    if keywords
        .iter()
        .any(|keyword| keyword.trim().is_empty() || !unique.insert(keyword))
    {
        return Err(UiManagementInvariantError::InvalidComponentKeywords);
    }
    Ok(())
}

impl Error for UiManagementInvariantError {}

pub fn validate_ui_code_template(
    name: &str,
    source: &str,
) -> Result<(), UiManagementInvariantError> {
    if name.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyTemplateName);
    }
    if source.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyTemplateSource);
    }
    if source.len() > UI_CODE_TEMPLATE_SOURCE_LIMIT {
        return Err(UiManagementInvariantError::TemplateSourceTooLarge);
    }
    Ok(())
}
