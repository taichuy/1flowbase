use crate::{error_response::ApiError, routes::system::LocaleMetaResponse};
use axum::{body::{to_bytes, Body}, extract::Multipart, http::{Request, StatusCode}, middleware::Next, response::{IntoResponse, Response}};
use control_plane::plugin_management::InstallPluginResult;
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use crate::plugin_risk::PluginRiskOverrideBody;
#[derive(Debug, Deserialize, ToSchema, Serialize)]
pub struct InstallOfficialPluginBody {
    pub plugin_id: String,
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
    pub risk_override: Option<PluginRiskOverrideBody>,
}

#[derive(ToSchema)]
#[allow(dead_code)]
pub struct PluginUploadMultipartBody {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct PluginCompatibilityOverrideBody {
    pub reason: String,
    pub acknowledged_current_host_version: String,
    pub acknowledged_minimum_host_version: String,
}

#[derive(Debug, Deserialize, IntoParams, Clone, Serialize)]
pub struct OfficialPluginCatalogQuery {
    /// Optional plugin kind filter for official catalog views.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Installation record returned by the plugin API.")]
pub struct PluginInstallationResponse {
    pub id: String,
    pub provider_code: String,
    pub runtime_slot: Option<String>,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contract_version: String,
    pub protocol: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: String,
    pub verification_status: String,
    pub desired_state: String,
    pub expected_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub local_artifact: Option<PluginArtifactInstanceResponse>,
    #[schema(value_type = Object)]
    pub metadata_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PluginArtifactInstanceResponse {
    pub node_id: String,
    pub installation_id: String,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub local_path: Option<String>,
    pub package_path: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub artifact_status: String,
    pub runtime_status: String,
    pub availability_status: String,
    pub checked_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialPluginArtifactResponse {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target: String,
    pub download_url: String,
    pub checksum: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Official registry entry returned by the plugin API.")]
pub struct OfficialPluginCatalogEntryResponse {
    pub plugin_id: String,
    pub plugin_type: String,
    pub provider_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub protocol: String,
    pub latest_version: String,
    pub minimum_host_version: String,
    pub current_host_version: String,
    pub compatibility_status: String,
    pub compatibility_warning_reason: Option<String>,
    pub selected_artifact: OfficialPluginArtifactResponse,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
    pub install_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialPluginCatalogPageResponse {
    pub limit: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PluginTaskResponse {
    pub id: String,
    pub installation_id: Option<String>,
    pub workspace_id: Option<String>,
    pub provider_code: String,
    pub task_kind: String,
    pub status: String,
    pub status_message: Option<String>,
    #[schema(value_type = Object)]
    pub detail_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InstallPluginResponse {
    pub installation: PluginInstallationResponse,
    pub task: PluginTaskResponse,
}

pub fn format_time(value: time::OffsetDateTime) -> Result<String, ApiError> {
    Ok(value.format(&Rfc3339)?)
}

pub async fn enforce_plugin_upload_limit(
    plugin_upload_max_bytes: usize,
    request: Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();
    match to_bytes(body, plugin_upload_max_bytes).await {
        Ok(bytes) => {
            next.run(Request::from_parts(parts, Body::from(bytes)))
                .await
        }
        Err(_) => StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    }
}

pub async fn read_upload_file(
    multipart: &mut Multipart,
) -> Result<(String, Vec<u8>), ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("plugin_file"))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let file_name = field
            .file_name()
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "plugin_file_name",
            ))?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("plugin_file"))?;
        if bytes.is_empty() {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("plugin_file").into(),
            );
        }
        return Ok((file_name, bytes.to_vec()));
    }

    Err(control_plane::errors::ControlPlaneError::InvalidInput("plugin_file").into())
}

pub fn requested_locales(
    locale_meta: &LocaleMetaResponse,
) -> control_plane::i18n::RequestedLocales {
    control_plane::i18n::RequestedLocales::new(
        locale_meta.resolved_locale.clone(),
        locale_meta.fallback_locale.clone(),
    )
}

pub fn to_installation_response_with_artifact(
    installation: domain::PluginInstallationRecord,
    local_artifact: Option<domain::PluginArtifactInstanceRecord>,
) -> Result<PluginInstallationResponse, ApiError> {
    Ok(PluginInstallationResponse {
        id: installation.id.to_string(),
        provider_code: installation.provider_code,
        runtime_slot: runtime_slot_for_contract(&installation.contract_version),
        plugin_id: installation.plugin_id,
        plugin_version: installation.plugin_version,
        contract_version: installation.contract_version,
        protocol: installation.protocol,
        display_name: installation.display_name,
        source_kind: installation.source_kind,
        trust_level: installation.trust_level,
        verification_status: installation.verification_status.as_str().to_string(),
        desired_state: installation.desired_state.as_str().to_string(),
        expected_checksum: installation.expected_checksum,
        signature_status: installation.signature_status.as_str().to_string(),
        signature_algorithm: installation.signature_algorithm,
        signing_key_id: installation.signing_key_id,
        local_artifact: local_artifact
            .map(to_artifact_instance_response)
            .transpose()?,
        metadata_json: installation.metadata_json,
        created_at: format_time(installation.created_at)?,
        updated_at: format_time(installation.updated_at)?,
    })
}

pub fn to_artifact_instance_response(
    artifact: domain::PluginArtifactInstanceRecord,
) -> Result<PluginArtifactInstanceResponse, ApiError> {
    Ok(PluginArtifactInstanceResponse {
        node_id: artifact.node_id,
        installation_id: artifact.installation_id.to_string(),
        local_version: artifact.local_version,
        local_checksum: artifact.local_checksum,
        local_path: artifact.local_path,
        package_path: artifact.package_path,
        manifest_fingerprint: artifact.manifest_fingerprint,
        artifact_status: artifact.artifact_status.as_str().to_string(),
        runtime_status: artifact.runtime_status.as_str().to_string(),
        availability_status: artifact.availability_status.as_str().to_string(),
        checked_at: format_time(artifact.checked_at)?,
        last_error: artifact.last_error,
    })
}

pub fn to_install_response(
    result: InstallPluginResult,
) -> Result<InstallPluginResponse, ApiError> {
    Ok(InstallPluginResponse {
        installation: to_installation_response_with_artifact(
            result.installation,
            Some(result.local_artifact),
        )?,
        task: to_task_response(result.task)?,
    })
}

pub fn to_task_response(
    task: domain::PluginTaskRecord,
) -> Result<PluginTaskResponse, ApiError> {
    Ok(PluginTaskResponse {
        id: task.id.to_string(),
        installation_id: task.installation_id.map(|id| id.to_string()),
        workspace_id: task.workspace_id.map(|id| id.to_string()),
        provider_code: task.provider_code,
        task_kind: task.task_kind.as_str().to_string(),
        status: task.status.as_str().to_string(),
        status_message: task.status_message,
        detail_json: task.detail_json,
        created_at: format_time(task.created_at)?,
        updated_at: format_time(task.updated_at)?,
        finished_at: format_optional_time(task.finished_at)?,
    })
}

#[doc(hidden)]
pub fn runtime_slot_for_contract(contract_version: &str) -> Option<String> {
    match contract_version {
        CURRENT_PROVIDER_CONTRACT => Some("model_provider".to_string()),
        "1flowbase.data_source/v1" => Some("data_source".to_string()),
        _ => None,
    }
}
#[async_trait::async_trait]
pub trait OfficialPluginInstallResolver: Send + Sync {
    async fn resolve(&self, source: &dyn crate::official_extension_catalog::OfficialExtensionCatalogSourcePort, actor_user_id: uuid::Uuid, workspace_id: uuid::Uuid, body: InstallOfficialPluginBody) -> Result<control_plane::plugin_management::InstallResolvedOfficialPluginCommand, ApiError>;
}

pub fn format_optional_time(value: Option<time::OffsetDateTime>) -> Result<Option<String>, ApiError> {
    value.map(format_time).transpose()
}
