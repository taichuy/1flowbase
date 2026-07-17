#![cfg(target_os = "linux")]

use std::time::Duration;

use sysinfo::System;

use crate::metrics::{
    cgroup_scope_detected, cpu_metrics, has_container_cgroup_marker, CgroupSnapshot,
    RuntimeMetricAvailability, RuntimeMetricScopeKind,
};

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
