use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationApiMappingDraft {
    pub mapping: ApplicationApiMappingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingConfig {
    pub input: ApplicationApiMappingInput,
    pub output: ApplicationApiMappingOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<WorkflowExtensionApiConfig>,
}

impl ApplicationApiMappingConfig {
    pub fn default_native() -> Self {
        Self {
            input: ApplicationApiMappingInput {
                query_target: "node-start.query".to_string(),
                model_target: Some("node-start.model".to_string()),
                inputs_target: Some("node-start".to_string()),
                history_target: Some("node-start.history".to_string()),
                attachments_target: Some("node-start.files".to_string()),
            },
            output: ApplicationApiMappingOutput::default(),
            extension: None,
        }
    }

    pub fn extension_slug(&self) -> Option<&str> {
        self.extension
            .as_ref()
            .map(|extension| extension.slug.as_str())
    }
}

impl ApplicationApiMappingDraft {
    pub fn default_native() -> Self {
        Self {
            mapping: ApplicationApiMappingConfig::default_native(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingInput {
    pub query_target: String,
    pub model_target: Option<String>,
    pub inputs_target: Option<String>,
    pub history_target: Option<String>,
    pub attachments_target: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingOutput {
    pub answer_selector: Option<String>,
    pub usage_selector: Option<String>,
    pub files_selector: Option<String>,
    pub error_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowExtensionApiConfig {
    pub slug: String,
    pub method: WorkflowExtensionHttpMethod,
    pub response_mode: WorkflowExtensionResponseMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkflowExtensionHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl WorkflowExtensionHttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExtensionResponseMode {
    Sync,
    Async,
}

impl WorkflowExtensionResponseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Async => "async",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationPublicationJsDependencySnapshot {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
    pub permissions: domain::JsDependencyPermissions,
}

impl From<domain::ApplicationJsDependencySelection> for ApplicationPublicationJsDependencySnapshot {
    fn from(selection: domain::ApplicationJsDependencySelection) -> Self {
        Self {
            installation_id: selection.installation_id,
            provider_code: selection.provider_code,
            plugin_id: selection.plugin_id,
            plugin_version: selection.plugin_version,
            alias: selection.alias,
            package: selection.package,
            version: selection.version,
            target: selection.target,
            artifact_path: selection.artifact_path,
            artifact_hash: selection.artifact_hash,
            integrity: selection.integrity,
            permissions: selection.permissions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationPublicationVersionRecord {
    pub id: Uuid,
    pub application_id: Uuid,
    pub workspace_id: Uuid,
    pub flow_id: Uuid,
    pub flow_version_id: Uuid,
    pub mapping_snapshot: ApplicationApiMappingConfig,
    pub extension_slug: Option<String>,
    pub compiled_plan_id: Uuid,
    pub version_sequence: i64,
    pub active: bool,
    pub api_enabled: bool,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub document_snapshot: serde_json::Value,
    pub runtime_profile_snapshot: serde_json::Value,
    pub output_selector: serde_json::Value,
    pub dependency_snapshot: Vec<ApplicationPublicationJsDependencySnapshot>,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowScheduleTriggerRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
