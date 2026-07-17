use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sysinfo::{Disks, NetworkData, Networks, ProcessRefreshKind, ProcessesToUpdate, System};
use time::OffsetDateTime;

const MAX_FRESH_SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetricAvailability {
    Available,
    WarmingUp,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetricScopeKind {
    Cgroup,
    Host,
    RuntimeVisible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCpuMetrics {
    pub availability: RuntimeMetricAvailability,
    pub scope_kind: RuntimeMetricScopeKind,
    pub usage_percent: Option<f64>,
    pub logical_count: u64,
    pub limit_cores: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMemoryMetrics {
    pub availability: RuntimeMetricAvailability,
    pub scope_kind: RuntimeMetricScopeKind,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub process_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStorageMetrics {
    pub availability: RuntimeMetricAvailability,
    pub scope_kind: RuntimeMetricScopeKind,
    pub mount_point: Option<String>,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeNetworkMetrics {
    pub availability: RuntimeMetricAvailability,
    pub scope_kind: RuntimeMetricScopeKind,
    pub received_bytes_per_second: Option<f64>,
    pub transmitted_bytes_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeDiskIoMetrics {
    pub availability: RuntimeMetricAvailability,
    pub scope_kind: RuntimeMetricScopeKind,
    pub read_bytes_per_second: Option<f64>,
    pub written_bytes_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetricsSnapshot {
    pub captured_at: OffsetDateTime,
    pub sample_interval_milliseconds: Option<u64>,
    pub cpu: RuntimeCpuMetrics,
    pub memory: RuntimeMemoryMetrics,
    pub storage: RuntimeStorageMetrics,
    pub network: RuntimeNetworkMetrics,
    pub disk_io: RuntimeDiskIoMetrics,
}

#[derive(Debug)]
pub(crate) struct RuntimeMetricSampler {
    system: System,
    networks: Networks,
    disks: Disks,
    current_pid: Option<sysinfo::Pid>,
    primary_path: PathBuf,
    previous_sample_at: Option<Instant>,
    previous_cgroup_cpu_usage_micros: Option<u64>,
    last_snapshot: Option<RuntimeMetricsSnapshot>,
}

impl RuntimeMetricSampler {
    pub(crate) fn new() -> Self {
        Self {
            system: System::new_all(),
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            current_pid: sysinfo::get_current_pid().ok(),
            primary_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            previous_sample_at: None,
            previous_cgroup_cpu_usage_micros: None,
            last_snapshot: None,
        }
    }

    pub(crate) fn collect(&mut self) -> RuntimeMetricsSnapshot {
        let sampled_at = Instant::now();
        let interval = self
            .previous_sample_at
            .map(|previous| sampled_at.saturating_duration_since(previous));
        if interval.is_some_and(|value| value < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL) {
            if let Some(snapshot) = &self.last_snapshot {
                return snapshot.clone();
            }
        }
        let rate_availability = rate_availability(interval);

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        if let Some(pid) = self.current_pid {
            let pids = [pid];
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                false,
                ProcessRefreshKind::nothing()
                    .with_memory()
                    .with_cpu()
                    .with_disk_usage(),
            );
        }
        self.networks.refresh(true);
        self.disks.refresh(true);

        let logical_count = self.system.cpus().len() as u64;
        let process_bytes = self
            .current_pid
            .and_then(|pid| self.system.process(pid))
            .map(|process| process.memory())
            .unwrap_or_default();
        let cgroup = CgroupSnapshot::current(logical_count);
        let memory = memory_metrics(&self.system, process_bytes, cgroup.as_ref());
        let cpu = cpu_metrics(
            &self.system,
            logical_count,
            interval,
            rate_availability,
            cgroup.as_ref(),
            self.previous_cgroup_cpu_usage_micros,
        );
        let storage = storage_metrics(&self.disks, &self.primary_path);
        let network = network_metrics(&self.networks, interval, rate_availability);
        let disk_io = disk_io_metrics(&self.disks, interval, rate_availability);

        self.previous_sample_at = Some(sampled_at);
        self.previous_cgroup_cpu_usage_micros =
            cgroup.and_then(|snapshot| snapshot.cpu_usage_micros);

        let snapshot = RuntimeMetricsSnapshot {
            captured_at: OffsetDateTime::now_utc(),
            sample_interval_milliseconds: interval.map(duration_milliseconds),
            cpu,
            memory,
            storage,
            network,
            disk_io,
        };
        self.last_snapshot = Some(snapshot.clone());
        snapshot
    }
}

fn rate_availability(interval: Option<Duration>) -> RuntimeMetricAvailability {
    match interval {
        None => RuntimeMetricAvailability::WarmingUp,
        Some(value) if value < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL => {
            RuntimeMetricAvailability::WarmingUp
        }
        Some(value) if value > MAX_FRESH_SAMPLE_INTERVAL => RuntimeMetricAvailability::Stale,
        Some(_) => RuntimeMetricAvailability::Available,
    }
}

fn duration_milliseconds(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(crate) fn cpu_metrics(
    system: &System,
    logical_count: u64,
    interval: Option<Duration>,
    rate_availability: RuntimeMetricAvailability,
    cgroup: Option<&CgroupSnapshot>,
    previous_cgroup_cpu_usage_micros: Option<u64>,
) -> RuntimeCpuMetrics {
    let host_limit = logical_count.max(1) as f64;
    let cgroup_scope =
        cgroup.filter(|snapshot| snapshot.scoped && snapshot.cpu_usage_micros.is_some());
    let cgroup_measurement = cgroup_scope.and_then(|snapshot| {
        if rate_availability != RuntimeMetricAvailability::Available {
            return None;
        }
        let current = snapshot.cpu_usage_micros?;
        let previous = previous_cgroup_cpu_usage_micros?;
        let elapsed = interval?.as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }
        let limit_cores = snapshot.cpu_limit_cores.unwrap_or(host_limit).max(0.01);
        let used_seconds = current.saturating_sub(previous) as f64 / 1_000_000.0;
        Some((used_seconds / elapsed / limit_cores * 100.0, limit_cores))
    });

    let (scope_kind, limit_cores, usage_percent) = if let Some(snapshot) = cgroup_scope {
        let limit = snapshot.cpu_limit_cores.unwrap_or(host_limit).max(0.01);
        (
            RuntimeMetricScopeKind::Cgroup,
            limit,
            cgroup_measurement.map(|(usage, _)| usage.clamp(0.0, 100.0)),
        )
    } else {
        (
            RuntimeMetricScopeKind::Host,
            host_limit,
            (rate_availability == RuntimeMetricAvailability::Available)
                .then(|| f64::from(system.global_cpu_usage()).clamp(0.0, 100.0)),
        )
    };

    let availability = if usage_percent.is_some() {
        RuntimeMetricAvailability::Available
    } else if cgroup_scope.is_some() && rate_availability == RuntimeMetricAvailability::Available {
        RuntimeMetricAvailability::WarmingUp
    } else {
        rate_availability
    };

    RuntimeCpuMetrics {
        availability,
        scope_kind,
        usage_percent,
        logical_count,
        limit_cores,
    }
}

fn memory_metrics(
    system: &System,
    process_bytes: u64,
    cgroup: Option<&CgroupSnapshot>,
) -> RuntimeMemoryMetrics {
    let host_total = system.total_memory();
    let host_available = system.available_memory();
    let cgroup_memory = cgroup.and_then(|snapshot| {
        if !snapshot.scoped {
            return None;
        }
        let used = snapshot.memory_used_bytes?;
        let total = snapshot
            .memory_limit_bytes
            .unwrap_or(host_total)
            .min(host_total);
        (total > 0).then_some((used.min(total), total))
    });
    let (scope_kind, used_bytes, total_bytes, available_bytes) =
        if let Some((used, total)) = cgroup_memory {
            (
                RuntimeMetricScopeKind::Cgroup,
                used,
                total,
                total.saturating_sub(used),
            )
        } else {
            (
                RuntimeMetricScopeKind::Host,
                host_total.saturating_sub(host_available),
                host_total,
                host_available,
            )
        };

    RuntimeMemoryMetrics {
        availability: if total_bytes > 0 {
            RuntimeMetricAvailability::Available
        } else {
            RuntimeMetricAvailability::Unavailable
        },
        scope_kind,
        total_bytes,
        available_bytes,
        used_bytes,
        process_bytes,
    }
}

fn storage_metrics(disks: &Disks, primary_path: &Path) -> RuntimeStorageMetrics {
    let selected = disks
        .list()
        .iter()
        .filter(|disk| disk.total_space() > 0)
        .filter(|disk| primary_path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .or_else(|| disks.list().iter().find(|disk| disk.total_space() > 0));

    if let Some(disk) = selected {
        let total_bytes = disk.total_space();
        let available_bytes = disk.available_space();
        RuntimeStorageMetrics {
            availability: RuntimeMetricAvailability::Available,
            scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
            mount_point: Some(disk.mount_point().to_string_lossy().into_owned()),
            file_system: Some(disk.file_system().to_string_lossy().into_owned()),
            total_bytes: Some(total_bytes),
            available_bytes: Some(available_bytes),
            used_bytes: Some(total_bytes.saturating_sub(available_bytes)),
        }
    } else {
        RuntimeStorageMetrics {
            availability: RuntimeMetricAvailability::Unavailable,
            scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
            mount_point: None,
            file_system: None,
            total_bytes: None,
            available_bytes: None,
            used_bytes: None,
        }
    }
}

fn network_metrics(
    networks: &Networks,
    interval: Option<Duration>,
    rate_availability: RuntimeMetricAvailability,
) -> RuntimeNetworkMetrics {
    let visible = networks
        .list()
        .iter()
        .filter(|(name, data)| !is_loopback_interface(name, data))
        .map(|(_, data)| data)
        .collect::<Vec<_>>();
    if visible.is_empty() {
        return RuntimeNetworkMetrics {
            availability: RuntimeMetricAvailability::Unavailable,
            scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
            received_bytes_per_second: None,
            transmitted_bytes_per_second: None,
        };
    }

    let seconds = interval
        .map(|value| value.as_secs_f64())
        .unwrap_or_default();
    let rates_ready = rate_availability == RuntimeMetricAvailability::Available && seconds > 0.0;
    let received = visible.iter().map(|data| data.received()).sum::<u64>();
    let transmitted = visible.iter().map(|data| data.transmitted()).sum::<u64>();
    RuntimeNetworkMetrics {
        availability: rate_availability,
        scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
        received_bytes_per_second: rates_ready.then_some(received as f64 / seconds),
        transmitted_bytes_per_second: rates_ready.then_some(transmitted as f64 / seconds),
    }
}

fn is_loopback_interface(name: &str, data: &NetworkData) -> bool {
    name == "lo"
        || name.to_ascii_lowercase().contains("loopback")
        || (!data.ip_networks().is_empty()
            && data
                .ip_networks()
                .iter()
                .all(|network| network.addr.is_loopback()))
}

fn disk_io_metrics(
    disks: &Disks,
    interval: Option<Duration>,
    rate_availability: RuntimeMetricAvailability,
) -> RuntimeDiskIoMetrics {
    if disks.list().is_empty() {
        return RuntimeDiskIoMetrics {
            availability: RuntimeMetricAvailability::Unavailable,
            scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
            read_bytes_per_second: None,
            written_bytes_per_second: None,
        };
    }

    let seconds = interval
        .map(|value| value.as_secs_f64())
        .unwrap_or_default();
    let rates_ready = rate_availability == RuntimeMetricAvailability::Available && seconds > 0.0;
    let (read_bytes, written_bytes) = disks.list().iter().fold((0_u64, 0_u64), |sum, disk| {
        let usage = disk.usage();
        (
            sum.0.saturating_add(usage.read_bytes),
            sum.1.saturating_add(usage.written_bytes),
        )
    });
    RuntimeDiskIoMetrics {
        availability: rate_availability,
        scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
        read_bytes_per_second: rates_ready.then_some(read_bytes as f64 / seconds),
        written_bytes_per_second: rates_ready.then_some(written_bytes as f64 / seconds),
    }
}

#[derive(Debug)]
pub(crate) struct CgroupSnapshot {
    pub(crate) scoped: bool,
    pub(crate) cpu_usage_micros: Option<u64>,
    pub(crate) cpu_limit_cores: Option<f64>,
    pub(crate) memory_used_bytes: Option<u64>,
    pub(crate) memory_limit_bytes: Option<u64>,
}

impl CgroupSnapshot {
    #[cfg(target_os = "linux")]
    fn current(logical_count: u64) -> Option<Self> {
        let cgroup_membership = fs::read_to_string("/proc/self/cgroup").ok()?;
        let relative_path = cgroup_membership
            .lines()
            .find_map(|line| line.strip_prefix("0::"))?
            .trim_start_matches('/');
        let root = Path::new("/sys/fs/cgroup").join(relative_path);
        let cpu_usage_micros = read_table_value(&root.join("cpu.stat"), "usage_usec");
        let cpu_limit_cores = read_cpu_limit(&root, logical_count);
        let memory_used_bytes = read_u64(&root.join("memory.current"));
        let memory_limit_bytes = read_limit(&root.join("memory.max"));
        let scoped = cgroup_scope_detected(
            is_container_environment(&cgroup_membership),
            cpu_limit_cores,
            memory_limit_bytes,
            logical_count,
        );

        Some(Self {
            scoped,
            cpu_usage_micros,
            cpu_limit_cores,
            memory_used_bytes,
            memory_limit_bytes,
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn current(_logical_count: u64) -> Option<Self> {
        None
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn cgroup_scope_detected(
    containerized: bool,
    cpu_limit_cores: Option<f64>,
    memory_limit_bytes: Option<u64>,
    logical_count: u64,
) -> bool {
    containerized
        || cpu_limit_cores.is_some_and(|limit| limit < logical_count.max(1) as f64)
        || memory_limit_bytes.is_some()
}

#[cfg(target_os = "linux")]
fn is_container_environment(cgroup_membership: &str) -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
        || has_container_cgroup_marker(cgroup_membership)
        || fs::read_to_string("/proc/1/cgroup")
            .ok()
            .is_some_and(|value| has_container_cgroup_marker(&value))
}

#[cfg(target_os = "linux")]
pub(crate) fn has_container_cgroup_marker(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    ["docker", "kubepods", "containerd", "libpod", "lxc"]
        .iter()
        .any(|marker| normalized.contains(marker))
}

#[cfg(target_os = "linux")]
fn read_cpu_limit(root: &Path, logical_count: u64) -> Option<f64> {
    let quota_limit = fs::read_to_string(root.join("cpu.max"))
        .ok()
        .and_then(|value| {
            let mut parts = value.split_whitespace();
            let quota = parts.next()?;
            let period = parts.next()?.parse::<f64>().ok()?;
            if quota == "max" || period <= 0.0 {
                None
            } else {
                quota.parse::<f64>().ok().map(|value| value / period)
            }
        });
    let cpuset_limit = fs::read_to_string(root.join("cpuset.cpus.effective"))
        .ok()
        .and_then(|value| parse_cpu_set(value.trim()))
        .map(|count| count as f64);
    [quota_limit, cpuset_limit, Some(logical_count.max(1) as f64)]
        .into_iter()
        .flatten()
        .filter(|value| *value > 0.0)
        .reduce(f64::min)
}

#[cfg(target_os = "linux")]
fn parse_cpu_set(value: &str) -> Option<u64> {
    if value.is_empty() {
        return None;
    }
    value.split(',').try_fold(0_u64, |total, segment| {
        let mut bounds = segment.split('-');
        let start = bounds.next()?.parse::<u64>().ok()?;
        let end = bounds
            .next()
            .map(str::parse::<u64>)
            .transpose()
            .ok()?
            .unwrap_or(start);
        (end >= start).then_some(total.saturating_add(end - start + 1))
    })
}

#[cfg(target_os = "linux")]
fn read_table_value(path: &Path, key: &str) -> Option<u64> {
    fs::read_to_string(path).ok()?.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        (parts.next()? == key)
            .then(|| parts.next()?.parse::<u64>().ok())
            .flatten()
    })
}

#[cfg(target_os = "linux")]
fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn read_limit(path: &Path) -> Option<u64> {
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    (value != "max").then(|| value.parse().ok()).flatten()
}
