//! Schema-only descriptions of existing MCP bundle wire types. HTTP handlers
//! continue serializing the original owner DTOs without conversions or aliases.
use utoipa::ToSchema;

#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct McpBundleUploadMultipartBody {
    /// Nonempty ZIP archive, at most 8 MiB, read from the `file` multipart field.
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

/// Wire fields of `official_mcp_bundles::OfficialMcpBundleCatalogSnapshot`.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct OfficialMcpBundleCatalogSnapshotDocument {
    pub source: crate::official_mcp_bundles::OfficialMcpBundleCatalogSource,
    pub entries: Vec<OfficialMcpBundleCatalogEntryDocument>,
}

/// Wire fields of `official_mcp_bundles::OfficialMcpBundleCatalogEntry`.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct OfficialMcpBundleCatalogEntryDocument {
    pub organization: String,
    pub bundle_id: String,
    pub latest_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub exported_from_system_version: String,
    pub release_tag: String,
    pub download_url: String,
    pub artifact_sha256: Option<String>,
}

/// Wire fields of `domain::ExtensionIntegrityWarning`.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct McpBundleIntegrityWarningDocument {
    pub code: String,
    pub message: String,
    pub overridable: bool,
}

/// Wire fields of `domain::ExtensionCompatibilityWarning`.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct McpBundleCompatibilityWarningDocument {
    pub reason: String,
    pub current_host_version: String,
    pub minimum_host_version: String,
}

/// Wire fields of `domain::ExtensionRiskChallenge`.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct McpBundleRiskChallengeDocument {
    pub warnings: Vec<McpBundleIntegrityWarningDocument>,
    pub compatibility: Option<McpBundleCompatibilityWarningDocument>,
}

/// Existing successful library-release deletion acknowledgement.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct McpBundleLibraryDeleteResponse {
    pub deleted: bool,
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/openapi_contract.rs"]
mod tests;
