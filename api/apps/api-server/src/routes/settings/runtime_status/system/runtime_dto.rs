use super::*;

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
    pub related_process_bytes: u64,
    pub related_process_count: u64,
    pub cgroup_composition: Option<SystemRuntimeCgroupMemoryCompositionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCgroupMemoryCompositionResponse {
    pub anonymous_bytes: Option<u64>,
    pub file_bytes: Option<u64>,
    pub kernel_bytes: Option<u64>,
    pub shared_memory_bytes: Option<u64>,
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
                related_process_bytes: value.memory.related_process_bytes,
                related_process_count: value.memory.related_process_count,
                cgroup_composition: value.memory.cgroup_composition.as_ref().map(|composition| {
                    SystemRuntimeCgroupMemoryCompositionResponse {
                        anonymous_bytes: composition.anonymous_bytes,
                        file_bytes: composition.file_bytes,
                        kernel_bytes: composition.kernel_bytes,
                        shared_memory_bytes: composition.shared_memory_bytes,
                    }
                }),
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
    pub related_process_bytes: u64,
    pub related_process_count: u64,
    pub services: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemProcessResponse {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: Option<String>,
    pub user: Option<String>,
    pub status: String,
    pub cpu_usage_percent: f32,
    pub cpu_usage_single_core_percent: f32,
    pub memory_bytes: u64,
    pub memory_usage_percent: f32,
    pub start_time_unix_seconds: u64,
    pub terminable: bool,
    pub backend_process: bool,
}

impl From<runtime_profile::RuntimeProcessSample> for SystemProcessResponse {
    fn from(value: runtime_profile::RuntimeProcessSample) -> Self {
        Self {
            pid: value.pid,
            parent_pid: value.parent_pid,
            name: value.name,
            command: value.command,
            user: value.user,
            status: value.status,
            cpu_usage_percent: value.cpu_usage_percent,
            cpu_usage_single_core_percent: value.cpu_usage_single_core_percent,
            memory_bytes: value.memory_bytes,
            memory_usage_percent: value.memory_usage_percent,
            start_time_unix_seconds: value.start_time_unix_seconds,
            terminable: value.terminable,
            backend_process: value.backend_process,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemProcessTerminationResponse {
    pub pid: u32,
    pub outcome: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeProfileResponse {
    pub api_node_id: String,
    pub provider_install_root: String,
    pub host_extension_dropin_root: String,
    pub related_process_memory_complete: bool,
    pub locale_meta: LocaleMetaResponse,
    pub topology: SystemRuntimeTopologyResponse,
    pub services: SystemRuntimeServicesResponse,
    pub hosts: Vec<SystemRuntimeHostResponse>,
    pub runtime_targets: Vec<SystemRuntimeTargetResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeProcessListResponse {
    pub process_total: usize,
    pub processes: Vec<SystemProcessResponse>,
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
