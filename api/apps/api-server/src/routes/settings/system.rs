use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap},
    Json, Router,
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
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
    runtime_profile_client::RuntimeProfileSnapshotCache,
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
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeMetricAvailabilityResponse {
    Available,
    WarmingUp,
    Stale,
    Unavailable,
}

impl From<runtime_profile::RuntimeMetricAvailability> for SystemRuntimeMetricAvailabilityResponse {
    fn from(value: runtime_profile::RuntimeMetricAvailability) -> Self {
        match value {
            runtime_profile::RuntimeMetricAvailability::Available => Self::Available,
            runtime_profile::RuntimeMetricAvailability::WarmingUp => Self::WarmingUp,
            runtime_profile::RuntimeMetricAvailability::Stale => Self::Stale,
            runtime_profile::RuntimeMetricAvailability::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeMetricScopeKindResponse {
    Cgroup,
    Host,
    RuntimeVisible,
}

impl From<runtime_profile::RuntimeMetricScopeKind> for SystemRuntimeMetricScopeKindResponse {
    fn from(value: runtime_profile::RuntimeMetricScopeKind) -> Self {
        match value {
            runtime_profile::RuntimeMetricScopeKind::Cgroup => Self::Cgroup,
            runtime_profile::RuntimeMetricScopeKind::Host => Self::Host,
            runtime_profile::RuntimeMetricScopeKind::RuntimeVisible => Self::RuntimeVisible,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCpuMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub usage_percent: Option<f64>,
    pub logical_count: u64,
    pub limit_cores: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMemoryMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub process_bytes: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeStorageMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub mount_point: Option<String>,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeNetworkMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub received_bytes_per_second: Option<f64>,
    pub transmitted_bytes_per_second: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeDiskIoMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub read_bytes_per_second: Option<f64>,
    pub written_bytes_per_second: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMetricsResponse {
    pub captured_at_unix_milliseconds: i64,
    pub sample_interval_milliseconds: Option<u64>,
    pub cpu: SystemRuntimeCpuMetricsResponse,
    pub memory: SystemRuntimeMemoryMetricsResponse,
    pub storage: SystemRuntimeStorageMetricsResponse,
    pub network: SystemRuntimeNetworkMetricsResponse,
    pub disk_io: SystemRuntimeDiskIoMetricsResponse,
}

impl From<&runtime_profile::RuntimeMetricsSnapshot> for SystemRuntimeMetricsResponse {
    fn from(value: &runtime_profile::RuntimeMetricsSnapshot) -> Self {
        Self {
            captured_at_unix_milliseconds: value
                .captured_at
                .unix_timestamp()
                .saturating_mul(1_000)
                .saturating_add(i64::from(value.captured_at.millisecond())),
            sample_interval_milliseconds: value.sample_interval_milliseconds,
            cpu: SystemRuntimeCpuMetricsResponse {
                availability: value.cpu.availability.into(),
                scope_kind: value.cpu.scope_kind.into(),
                usage_percent: value.cpu.usage_percent,
                logical_count: value.cpu.logical_count,
                limit_cores: value.cpu.limit_cores,
            },
            memory: SystemRuntimeMemoryMetricsResponse {
                availability: value.memory.availability.into(),
                scope_kind: value.memory.scope_kind.into(),
                total_bytes: value.memory.total_bytes,
                available_bytes: value.memory.available_bytes,
                used_bytes: value.memory.used_bytes,
                process_bytes: value.memory.process_bytes,
            },
            storage: SystemRuntimeStorageMetricsResponse {
                availability: value.storage.availability.into(),
                scope_kind: value.storage.scope_kind.into(),
                mount_point: value.storage.mount_point.clone(),
                file_system: value.storage.file_system.clone(),
                total_bytes: value.storage.total_bytes,
                available_bytes: value.storage.available_bytes,
                used_bytes: value.storage.used_bytes,
            },
            network: SystemRuntimeNetworkMetricsResponse {
                availability: value.network.availability.into(),
                scope_kind: value.network.scope_kind.into(),
                received_bytes_per_second: value.network.received_bytes_per_second,
                transmitted_bytes_per_second: value.network.transmitted_bytes_per_second,
            },
            disk_io: SystemRuntimeDiskIoMetricsResponse {
                availability: value.disk_io.availability.into(),
                scope_kind: value.disk_io.scope_kind.into(),
                read_bytes_per_second: value.disk_io.read_bytes_per_second,
                written_bytes_per_second: value.disk_io.written_bytes_per_second,
            },
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeTargetResponse {
    pub target_id: String,
    pub reachable: bool,
    pub host_fingerprint: Option<String>,
    pub metrics: Option<SystemRuntimeMetricsResponse>,
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
    pub runtime_targets: Vec<SystemRuntimeTargetResponse>,
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

    let profiles = RuntimeProfileSnapshotCache::new(
        state.infrastructure.cache_store(),
        state.infrastructure.distributed_lock(),
        state.api_runtime_profile.clone(),
        state.plugin_runner_system.clone(),
        state.api_node_id.clone(),
        state.process_started_at,
    )
    .get_or_refresh()
    .await?;
    Ok(Json(ApiSuccess::new(merge_runtime_profiles(
        locale,
        profiles.api_profile,
        profiles.runner_profile,
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
    let runtime_targets = vec![
        runtime_target_from_profile(&api_profile),
        runner_profile
            .as_ref()
            .map(runtime_target_from_profile)
            .unwrap_or_else(unreachable_runner_target),
    ];

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
        runtime_targets,
    }
}

fn runtime_target_from_profile(profile: &RuntimeProfile) -> SystemRuntimeTargetResponse {
    SystemRuntimeTargetResponse {
        target_id: profile.service.clone(),
        reachable: true,
        host_fingerprint: Some(profile.host_fingerprint.clone()),
        metrics: Some(SystemRuntimeMetricsResponse::from(&profile.metrics)),
    }
}

fn unreachable_runner_target() -> SystemRuntimeTargetResponse {
    SystemRuntimeTargetResponse {
        target_id: "plugin-runner".to_string(),
        reachable: false,
        host_fingerprint: None,
        metrics: None,
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
