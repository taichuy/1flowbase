use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, header::ACCEPT_LANGUAGE},
};
use control_plane::system_runtime::SystemRuntimeService;
use runtime_profile::{LocaleResolution, LocaleResolutionInput, LocaleSource, RuntimeProfile};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
};

pub use super::release_status::{
    ConsoleReleaseInfoResponse, ConsoleReleaseStatusResponse, ConsoleReleaseUpgradeCommandsResponse,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct SystemRuntimeProfileQuery {
    pub locale: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocaleSourceResponse {
    Query,
    ExplicitHeader,
    UserPreferredLocale,
    AcceptLanguage,
    Fallback,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocaleMetaResponse {
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub source: LocaleSourceResponse,
    pub fallback_locale: String,
    pub supported_locales: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeRelationship {
    SameHost,
    SplitHost,
    RunnerUnreachable,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeTopologyResponse {
    pub relationship: SystemRuntimeRelationship,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeServiceResponse {
    pub reachable: bool,
    pub service: String,
    pub status: Option<String>,
    pub version: Option<String>,
    pub host_fingerprint: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeServicesResponse {
    pub api_server: SystemRuntimeServiceResponse,
    pub plugin_runner: SystemRuntimeServiceResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimePlatformResponse {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target_triple: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCpuResponse {
    pub logical_count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMemoryResponse {
    pub total_bytes: u64,
    pub total_gb: f64,
    pub available_bytes: u64,
    pub available_gb: f64,
    pub process_bytes: u64,
    pub process_gb: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeHostResponse {
    pub host_fingerprint: String,
    pub platform: SystemRuntimePlatformResponse,
    pub cpu: SystemRuntimeCpuResponse,
    pub memory: SystemRuntimeMemoryResponse,
    pub services: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeProfileResponse {
    pub api_node_id: String,
    pub provider_install_root: String,
    pub host_extension_dropin_root: String,
    pub locale_meta: LocaleMetaResponse,
    pub topology: SystemRuntimeTopologyResponse,
    pub services: SystemRuntimeServicesResponse,
    pub hosts: Vec<SystemRuntimeHostResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/system/runtime-profile",
            console_get(
                get_runtime_profile,
                ConsoleOperation("system.runtime_profile.view".to_string()),
            ),
        )
        .route(
            "/system/release-status",
            console_get(
                get_release_status,
                ConsoleOperation("system.release_status.view".to_string()),
            ),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/system/release-status",
    responses(
        (status = 200, body = ConsoleReleaseStatusResponse),
        (status = 401, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_release_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ConsoleReleaseStatusResponse>>, ApiError> {
    require_session(&state, &headers).await?;

    Ok(Json(ApiSuccess::new(
        super::release_status::fetch_console_release_status().await,
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/system/runtime-profile",
    params(SystemRuntimeProfileQuery),
    responses(
        (status = 200, body = SystemRuntimeProfileResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_profile(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SystemRuntimeProfileQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<SystemRuntimeProfileResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let access = SystemRuntimeService::new(state.store.clone())
        .authorize_view(context.user.id)
        .await?;

    let locale = runtime_profile::resolve_locale(LocaleResolutionInput {
        query_locale: query.locale,
        explicit_header_locale: header_locale(&headers),
        user_preferred_locale: access.preferred_locale,
        accept_language: header_accept_language(&headers),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    });

    let api_profile = state
        .api_runtime_profile
        .collect_runtime_profile(state.process_started_at)
        .await?;
    let runner_profile = state
        .plugin_runner_system
        .fetch_runtime_profile()
        .await
        .ok();
    Ok(Json(ApiSuccess::new(merge_runtime_profiles(
        locale,
        api_profile,
        runner_profile,
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
        state.host_extension_dropin_root.clone(),
    ))))
}

fn header_locale(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-1flowbase-locale")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn header_accept_language(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn merge_runtime_profiles(
    locale_meta: LocaleResolution,
    api_profile: RuntimeProfile,
    runner_profile: Option<RuntimeProfile>,
    api_node_id: String,
    provider_install_root: String,
    host_extension_dropin_root: String,
) -> SystemRuntimeProfileResponse {
    let relationship = match runner_profile.as_ref() {
        Some(profile) if profile.host_fingerprint == api_profile.host_fingerprint => {
            SystemRuntimeRelationship::SameHost
        }
        Some(_) => SystemRuntimeRelationship::SplitHost,
        None => SystemRuntimeRelationship::RunnerUnreachable,
    };

    let hosts = match runner_profile.as_ref() {
        Some(profile) if profile.host_fingerprint == api_profile.host_fingerprint => {
            vec![host_from_profile(
                &api_profile,
                vec!["api-server", "plugin-runner"],
            )]
        }
        Some(profile) => vec![
            host_from_profile(&api_profile, vec!["api-server"]),
            host_from_profile(profile, vec!["plugin-runner"]),
        ],
        None => vec![host_from_profile(&api_profile, vec!["api-server"])],
    };

    SystemRuntimeProfileResponse {
        api_node_id,
        provider_install_root,
        host_extension_dropin_root,
        locale_meta: locale_meta.into(),
        topology: SystemRuntimeTopologyResponse { relationship },
        services: SystemRuntimeServicesResponse {
            api_server: service_from_profile(&api_profile),
            plugin_runner: runner_profile
                .as_ref()
                .map(service_from_profile)
                .unwrap_or_else(unreachable_runner_service),
        },
        hosts,
    }
}

fn host_from_profile(profile: &RuntimeProfile, services: Vec<&str>) -> SystemRuntimeHostResponse {
    SystemRuntimeHostResponse {
        host_fingerprint: profile.host_fingerprint.clone(),
        platform: SystemRuntimePlatformResponse {
            os: profile.platform.os.clone(),
            arch: profile.platform.arch.clone(),
            libc: profile.platform.libc.clone(),
            rust_target_triple: profile.platform.rust_target.clone(),
        },
        cpu: SystemRuntimeCpuResponse {
            logical_count: profile.cpu.logical_count,
        },
        memory: SystemRuntimeMemoryResponse {
            total_bytes: profile.memory.total_bytes,
            total_gb: profile.memory.total_gb,
            available_bytes: profile.memory.available_bytes,
            available_gb: profile.memory.available_gb,
            process_bytes: profile.memory.process_bytes,
            process_gb: profile.memory.process_gb,
        },
        services: services.into_iter().map(str::to_string).collect(),
    }
}

fn service_from_profile(profile: &RuntimeProfile) -> SystemRuntimeServiceResponse {
    SystemRuntimeServiceResponse {
        reachable: true,
        service: profile.service.clone(),
        status: Some(profile.service_status.clone()),
        version: Some(profile.service_version.clone()),
        host_fingerprint: Some(profile.host_fingerprint.clone()),
    }
}

fn unreachable_runner_service() -> SystemRuntimeServiceResponse {
    SystemRuntimeServiceResponse {
        reachable: false,
        service: "plugin-runner".to_string(),
        status: None,
        version: None,
        host_fingerprint: None,
    }
}

impl From<LocaleResolution> for LocaleMetaResponse {
    fn from(value: LocaleResolution) -> Self {
        Self {
            requested_locale: value.requested_locale,
            resolved_locale: value.resolved_locale,
            source: value.source.into(),
            fallback_locale: value.fallback_locale,
            supported_locales: value.supported_locales,
        }
    }
}

impl From<LocaleSource> for LocaleSourceResponse {
    fn from(value: LocaleSource) -> Self {
        match value {
            LocaleSource::Query => Self::Query,
            LocaleSource::ExplicitHeader => Self::ExplicitHeader,
            LocaleSource::UserPreferredLocale => Self::UserPreferredLocale,
            LocaleSource::AcceptLanguage => Self::AcceptLanguage,
            LocaleSource::Fallback => Self::Fallback,
        }
    }
}
