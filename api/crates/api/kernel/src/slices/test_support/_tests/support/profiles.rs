use runtime_profile::*;
use time::OffsetDateTime;
pub fn sample_runtime_profile(service: &str, host_fingerprint: &str) -> RuntimeProfile {
    let captured_at = OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap();
    let (related_process_bytes, related_process_count) = match service {
        "api-server" => (320 * 1024 * 1024, 2),
        "runtime-extension-host" => (448 * 1024 * 1024, 3),
        _ => (256 * 1024 * 1024, 1),
    };
    RuntimeProfile {
        host_fingerprint: host_fingerprint.to_string(),
        platform: RuntimePlatform {
            os: "linux".to_string(),
            arch: "amd64".to_string(),
            libc: Some("musl".to_string()),
            rust_target: "x86_64-unknown-linux-musl".to_string(),
        },
        cpu: RuntimeCpu { logical_count: 8 },
        memory: RuntimeMemory::from_bytes(
            16 * 1024 * 1024 * 1024,
            8 * 1024 * 1024 * 1024,
            256 * 1024 * 1024,
        ),
        uptime_seconds: 42,
        started_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        captured_at,
        service: service.to_string(),
        service_version: "0.1.0".to_string(),
        service_status: "ok".to_string(),
        metrics: RuntimeMetricsSnapshot {
            captured_at,
            sample_interval_milliseconds: Some(2_000),
            cpu: RuntimeCpuMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::Host,
                usage_percent: Some(12.5),
                logical_count: 8,
                limit_cores: 8.0,
            },
            memory: RuntimeMemoryMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::Host,
                total_bytes: 16 * 1024 * 1024 * 1024,
                available_bytes: 8 * 1024 * 1024 * 1024,
                used_bytes: 8 * 1024 * 1024 * 1024,
                process_bytes: 256 * 1024 * 1024,
                related_process_bytes,
                related_process_count,
                cgroup_composition: None,
            },
            storage: RuntimeStorageMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                mount_point: Some("/".to_string()),
                file_system: Some("overlay".to_string()),
                total_bytes: Some(64 * 1024 * 1024 * 1024),
                available_bytes: Some(48 * 1024 * 1024 * 1024),
                used_bytes: Some(16 * 1024 * 1024 * 1024),
            },
            network: RuntimeNetworkMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                received_bytes_per_second: Some(2_048.0),
                transmitted_bytes_per_second: Some(1_024.0),
            },
            disk_io: RuntimeDiskIoMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                read_bytes_per_second: Some(4_096.0),
                written_bytes_per_second: Some(8_192.0),
            },
        },
    }
}

mod wrappers;
pub use wrappers::*;
