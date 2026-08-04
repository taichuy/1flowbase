use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{PluginCompatibilityOverrideBody, PluginRiskOverrideBody};

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct LocalExtensionInventoryQuery {
    pub category: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionRiskWarningResponse {
    pub code: String,
    pub message: String,
    pub overridable: bool,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionCompatibilityWarningResponse {
    pub reason: String,
    pub current_host_version: String,
    pub minimum_host_version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInstalledVersionResponse {
    pub id: String,
    pub version: String,
    pub source_kind: String,
    pub trust_level: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub local_path: Option<String>,
    pub expected_checksum: Option<String>,
    pub local_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub status: String,
    pub is_current: bool,
    pub deletable: bool,
    pub delete_reasons: Vec<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInventoryEntryResponse {
    pub id: String,
    pub catalog_id: String,
    pub category: String,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub node_id: String,
    pub source_kind: String,
    pub trust_level: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub local_path: Option<String>,
    pub expected_checksum: Option<String>,
    pub local_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub status: String,
    pub is_current: bool,
    pub application_action: String,
    pub application_status: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub installed_versions: Vec<LocalExtensionInstalledVersionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInventoryPageResponse {
    pub limit: usize,
    pub total_entries: usize,
    pub next_cursor: Option<String>,
    pub entries: Vec<LocalExtensionInventoryEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionInstallResponse {
    pub installation: LocalExtensionInventoryEntryResponse,
    pub local_artifact_was_present: bool,
    pub node_plugin_installation_id: Option<String>,
    pub application_action: String,
    pub application_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionRiskChallengeErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub risk_challenge: ExtensionRiskChallengeResponse,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionRiskChallengeResponse {
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub compatibility: Option<ExtensionCompatibilityWarningResponse>,
}

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct ExtensionCatalogGatewayQuery {
    pub slot_code: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionCatalogGatewayEntryResponse {
    pub category: String,
    pub id: String,
    pub name: String,
    pub organization: String,
    pub artifact: String,
    pub version: String,
    pub description: String,
    pub host_version_requirement: String,
    pub slot_codes: Vec<String>,
    pub keywords: Vec<String>,
    #[schema(value_type = Object)]
    pub source: serde_json::Value,
    #[schema(value_type = Option<Object>)]
    pub signature: Option<serde_json::Value>,
    pub checksum: Option<String>,
    #[schema(value_type = Object)]
    pub download_locator: serde_json::Value,
    pub catalog_page: u32,
    pub catalog_source: String,
    pub current_version: Option<String>,
    pub installation_status: String,
    pub artifact_kind: Option<String>,
    pub installation_source: Option<String>,
    pub trust: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub compatibility: Option<ExtensionCompatibilityWarningResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionCatalogGatewayPageResponse {
    pub category: String,
    pub catalog_page: String,
    pub catalog_page_number: u32,
    pub catalog_page_checksum: String,
    pub catalog_page_locator: String,
    pub limit: usize,
    pub next_cursor: Option<String>,
    pub total_entries: usize,
    pub entries: Vec<ExtensionCatalogGatewayEntryResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtensionUpdateCheckItemBody {
    pub catalog_id: String,
    pub current_version: String,
    pub installed_versions: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtensionUpdateCheckBody {
    pub category: String,
    pub catalog_page: Option<String>,
    pub items: Vec<ExtensionUpdateCheckItemBody>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionUpdateCheckItemResponse {
    pub catalog_id: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionUpdateCheckResponse {
    pub category: String,
    pub catalog_page: Option<String>,
    pub items: Vec<ExtensionUpdateCheckItemResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InstallOfficialExtensionBody {
    pub category: String,
    pub catalog_id: String,
    pub version: String,
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
    pub risk_override: Option<PluginRiskOverrideBody>,
}

#[derive(ToSchema)]
#[allow(dead_code)]
pub struct ExtensionUploadMultipartBody {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    category: Option<String>,
    organization: Option<String>,
    artifact_id: Option<String>,
    version: Option<String>,
    risk_override: Option<String>,
    compatibility_override: Option<String>,
}
