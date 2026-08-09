use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::FrontendComponentContract;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiComponentOverrideState {
    Inherit,
    Published,
    Hidden,
}

impl UiComponentOverrideState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::Published => "published",
            Self::Hidden => "hidden",
        }
    }
}

impl TryFrom<&str> for UiComponentOverrideState {
    type Error = UiManagementInvariantError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "inherit" => Ok(Self::Inherit),
            "published" => Ok(Self::Published),
            "hidden" => Ok(Self::Hidden),
            _ => Err(UiManagementInvariantError::InvalidComponentState),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentLocator {
    pub provider_code: String,
    pub contribution_code: String,
    pub module_source: String,
    pub export_name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentContractRevision {
    pub id: Uuid,
    pub component_override_id: Uuid,
    pub revision: i32,
    pub contract: FrontendComponentContract,
    pub is_latest: bool,
    pub is_published: bool,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentOverride {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub locator: UiComponentLocator,
    pub state: UiComponentOverrideState,
    pub latest_revision: Option<UiComponentContractRevision>,
    pub published_revision: Option<UiComponentContractRevision>,
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
    InvalidComponentState,
    EmptyComponentLocator,
    ComponentExportMismatch,
    EmptyComponentCode,
    EmptyComponentDescription,
    EmptyComponentLimitations,
    EmptyComponentExamples,
    EmptyComponentInsertSnippet,
}

impl fmt::Display for UiManagementInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyTemplateName => "template name must not be empty",
            Self::EmptyTemplateSource => "template source must not be empty",
            Self::TemplateSourceTooLarge => "template source exceeds 262144 bytes",
            Self::InvalidTemplateLanguage => "template language must be jsx or tsx",
            Self::InvalidComponentState => "component state is invalid",
            Self::EmptyComponentLocator => "component locator fields must not be empty",
            Self::ComponentExportMismatch => "component contract export does not match locator",
            Self::EmptyComponentCode => "component code must not be empty",
            Self::EmptyComponentDescription => "component description must not be empty",
            Self::EmptyComponentLimitations => "component limitations must not be empty",
            Self::EmptyComponentExamples => "component examples must not be empty",
            Self::EmptyComponentInsertSnippet => "component insert snippet must not be empty",
        })
    }
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

pub fn validate_ui_component_contract(
    locator: &UiComponentLocator,
    contract: &FrontendComponentContract,
) -> Result<(), UiManagementInvariantError> {
    if [
        locator.provider_code.as_str(),
        locator.contribution_code.as_str(),
        locator.module_source.as_str(),
        locator.export_name.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(UiManagementInvariantError::EmptyComponentLocator);
    }
    if contract.export_name != locator.export_name {
        return Err(UiManagementInvariantError::ComponentExportMismatch);
    }
    if contract.component_code.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentCode);
    }
    if contract.description.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentDescription);
    }
    if contract.limitations.is_empty()
        || contract
            .limitations
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(UiManagementInvariantError::EmptyComponentLimitations);
    }
    if contract.examples.is_empty()
        || contract
            .examples
            .iter()
            .any(|example| example.title.trim().is_empty() || example.code.trim().is_empty())
    {
        return Err(UiManagementInvariantError::EmptyComponentExamples);
    }
    if contract.insert_snippet.trim().is_empty() {
        return Err(UiManagementInvariantError::EmptyComponentInsertSnippet);
    }
    Ok(())
}
