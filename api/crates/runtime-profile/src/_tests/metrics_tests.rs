#![cfg(target_os = "linux")]

use std::time::Duration;

use sysinfo::{Pid, System};

use crate::metrics::{
    cgroup_scope_detected, cpu_metrics, has_container_cgroup_marker, parse_cgroup_memory_stat,
    summarize_related_process_memory, CgroupSnapshot, RuntimeMetricAvailability,
    RuntimeMetricScopeKind, RuntimeProcessMemorySample,
};

#[test]
fn ac_007_related_process_memory_counts_the_root_and_recursive_descendants_only() {
    let root = Pid::from_u32(10);
    let samples = [
        RuntimeProcessMemorySample {
            pid: root,
            parent_pid: Some(Pid::from_u32(1)),
            resident_bytes: 100,
            is_thread: false,
        },
        RuntimeProcessMemorySample {
            pid: Pid::from_u32(11),
            parent_pid: Some(root),
            resident_bytes: 40,
            is_thread: false,
        },
        RuntimeProcessMemorySample {
            pid: Pid::from_u32(12),
            parent_pid: Some(Pid::from_u32(11)),
            resident_bytes: 20,
            is_thread: false,
        },
        RuntimeProcessMemorySample {
            pid: Pid::from_u32(13),
            parent_pid: Some(root),
            resident_bytes: 100,
            is_thread: true,
        },
        RuntimeProcessMemorySample {
            pid: Pid::from_u32(20),
            parent_pid: Some(Pid::from_u32(1)),
            resident_bytes: 1_000,
            is_thread: false,
        },
    ];

    let summary = summarize_related_process_memory(root, &samples);

    assert_eq!(summary.root_process_bytes, 100);
    assert_eq!(summary.related_process_bytes, 160);
    assert_eq!(summary.related_process_count, 3);
}

#[test]
fn ac_010_cgroup_memory_composition_preserves_unavailable_fields() {
    let composition =
        parse_cgroup_memory_stat("anon 1048576\nfile 2097152\nkernel 524288\nshmem 262144\n")
            .expect("at least one cgroup memory component should be available");

    assert_eq!(composition.anonymous_bytes, Some(1_048_576));
    assert_eq!(composition.file_bytes, Some(2_097_152));
    assert_eq!(composition.kernel_bytes, Some(524_288));
    assert_eq!(composition.shared_memory_bytes, Some(262_144));

    let partial = parse_cgroup_memory_stat("anon 4096\n").expect("anon should be available");
    assert_eq!(partial.anonymous_bytes, Some(4_096));
    assert_eq!(partial.file_bytes, None);
    assert_eq!(partial.kernel_bytes, None);
    assert_eq!(partial.shared_memory_bytes, None);
}

#[test]
fn local_systemd_session_without_limits_uses_host_scope() {
    assert!(!has_container_cgroup_marker(
        "0::/user.slice/user-1000.slice/session-7.scope"
    ));
    assert!(!cgroup_scope_detected(false, Some(8.0), None, 8,));
    assert!(cgroup_scope_detected(true, Some(8.0), None, 8));
    assert!(cgroup_scope_detected(false, Some(2.0), None, 8));
}

#[test]
fn cpu_metrics_fall_back_to_host_when_cgroup_usage_is_unavailable() {
    let system = System::new_all();
    let cgroup = CgroupSnapshot {
        scoped: true,
        cpu_usage_micros: None,
        cpu_limit_cores: Some(2.0),
        memory_used_bytes: Some(1),
        memory_limit_bytes: Some(2),
        memory_composition: None,
    };

    let metrics = cpu_metrics(
        &system,
        8,
        Some(Duration::from_secs(2)),
        RuntimeMetricAvailability::Available,
        Some(&cgroup),
        Some(10),
    );

    assert_eq!(metrics.scope_kind, RuntimeMetricScopeKind::Host);
    assert!(metrics.usage_percent.is_some());
}

#[test]
fn newly_available_cgroup_cpu_counter_warms_up_before_reporting_usage() {
    let system = System::new_all();
    let cgroup = CgroupSnapshot {
        scoped: true,
        cpu_usage_micros: Some(1_000),
        cpu_limit_cores: Some(2.0),
        memory_used_bytes: Some(1),
        memory_limit_bytes: Some(2),
        memory_composition: None,
    };

    let metrics = cpu_metrics(
        &system,
        8,
        Some(Duration::from_secs(2)),
        RuntimeMetricAvailability::Available,
        Some(&cgroup),
        None,
    );

    assert_eq!(metrics.scope_kind, RuntimeMetricScopeKind::Cgroup);
    assert_eq!(metrics.availability, RuntimeMetricAvailability::WarmingUp);
    assert_eq!(metrics.usage_percent, None);
}
