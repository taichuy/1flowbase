use std::{thread, time::Duration};

use runtime_profile::{bytes_to_gb, RuntimeMetricAvailability, RuntimeProfileCollector};

#[test]
fn runtime_profile_formats_memory_in_gb_with_two_decimals() {
    assert_eq!(bytes_to_gb(201_326_592), 0.19);
    assert_eq!(bytes_to_gb(17_179_869_184), 16.0);
}

#[test]
fn ac_002_runtime_profile_collector_warms_up_before_reporting_rates() {
    let collector = RuntimeProfileCollector::new(
        "runtime-profile-test",
        "0.0.0-test",
        time::OffsetDateTime::now_utc(),
        "ok",
    )
    .expect("runtime profile collector should initialize");

    let first = collector
        .collect()
        .expect("first runtime profile sample should succeed");
    assert_eq!(
        first.metrics.cpu.availability,
        RuntimeMetricAvailability::WarmingUp
    );
    assert_eq!(
        first.metrics.network.availability,
        RuntimeMetricAvailability::WarmingUp
    );
    assert_eq!(
        first.metrics.memory.availability,
        RuntimeMetricAvailability::Available
    );
    assert!(first.metrics.memory.total_bytes > 0);
    assert!(first.metrics.memory.related_process_bytes >= first.metrics.memory.process_bytes);
    assert!(first.metrics.memory.related_process_count >= 1);

    thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(50));

    let second = collector
        .collect()
        .expect("second runtime profile sample should succeed");
    assert_eq!(
        second.metrics.cpu.availability,
        RuntimeMetricAvailability::Available
    );
    assert!(second.metrics.cpu.usage_percent.is_some());
    assert_eq!(
        second.metrics.network.availability,
        RuntimeMetricAvailability::Available
    );
    assert!(second.metrics.network.received_bytes_per_second.is_some());
    assert!(second
        .metrics
        .network
        .transmitted_bytes_per_second
        .is_some());
    assert!(second.metrics.sample_interval_milliseconds.is_some());

    let immediate = collector
        .collect()
        .expect("an immediate repeated sample should reuse the latest snapshot");
    assert_eq!(immediate.metrics.captured_at, second.metrics.captured_at);
    assert_eq!(
        immediate.metrics.cpu.availability,
        RuntimeMetricAvailability::Available
    );
}
