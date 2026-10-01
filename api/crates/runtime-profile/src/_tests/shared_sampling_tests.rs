use runtime_profile::{
    RuntimeMetricAvailability, RuntimeProcessSampler, RuntimeProfileCollector, RuntimeSampleSource,
};
use std::{
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

fn profile(source: Arc<RuntimeSampleSource>, service: &str) -> RuntimeProfileCollector {
    RuntimeProfileCollector::new_with_sample_source(
        service,
        "test",
        time::OffsetDateTime::now_utc(),
        "ok",
        source,
    )
    .unwrap()
}
fn source() -> Arc<RuntimeSampleSource> {
    Arc::new(RuntimeSampleSource::new(Duration::from_secs(1)))
}
#[test]
fn no_consumers_do_not_refresh_the_os() {
    let source = source();
    let _api = profile(source.clone(), "api-server");
    let _host = profile(source.clone(), "runtime-extension-host");
    let _processes = RuntimeProcessSampler::with_sample_source(source.clone());
    assert_eq!(source.raw_refresh_count(), 0);
}
#[test]
fn profile_only_projects_two_services_from_one_observation() {
    let source = source();
    let api = profile(source.clone(), "api-server");
    let host = profile(source.clone(), "runtime-extension-host");
    let a = api.collect().unwrap();
    let h = host.collect().unwrap();
    assert_eq!(source.raw_refresh_count(), 1);
    assert_eq!(a.metrics, h.metrics);
    assert_eq!(a.service, "api-server");
    assert_eq!(h.service, "runtime-extension-host");
    assert_eq!(
        a.metrics.cpu.availability,
        RuntimeMetricAvailability::WarmingUp
    );
}
#[test]
fn process_only_reuses_the_process_projection_cache() {
    let source = source();
    let processes = RuntimeProcessSampler::with_sample_source(source.clone());
    let first = processes.collect();
    assert_eq!(first, processes.collect());
    assert!(first
        .processes
        .iter()
        .any(|p| p.pid == std::process::id() && !p.terminable));
    assert_eq!(source.raw_refresh_count(), 1);
}
#[test]
fn profiles_and_processes_share_one_real_os_refresh() {
    let source = source();
    let api = profile(source.clone(), "api-server");
    let host = profile(source.clone(), "runtime-extension-host");
    let processes = RuntimeProcessSampler::with_sample_source(source.clone());
    let a = api.collect().unwrap();
    let p = processes.collect();
    let h = host.collect().unwrap();
    assert!(!p.processes.is_empty());
    assert_eq!(a.metrics, h.metrics);
    assert_eq!(source.raw_refresh_count(), 1);
}
#[test]
fn concurrent_clients_share_one_observation_after_waiting_for_the_owner() {
    let source = source();
    let barrier = Arc::new(Barrier::new(8));
    thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let source = source.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    source.collect().unwrap()
                })
            })
            .collect();
        let snapshots: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(snapshots
            .windows(2)
            .all(|pair| Arc::ptr_eq(&pair[0], &pair[1])));
    });
    assert_eq!(source.raw_refresh_count(), 1);
}
#[test]
fn independent_sources_control_reproduces_three_os_refreshes() {
    let sources = [source(), source(), source()];
    profile(sources[0].clone(), "api-server").collect().unwrap();
    profile(sources[1].clone(), "runtime-extension-host")
        .collect()
        .unwrap();
    RuntimeProcessSampler::with_sample_source(sources[2].clone()).collect();
    assert_eq!(
        sources.iter().map(|s| s.raw_refresh_count()).sum::<usize>(),
        3
    );
}
#[test]
fn a_new_shared_sample_reports_a_real_delta_after_warmup() {
    let source = source();
    let collector = profile(source.clone(), "api-server");
    assert_eq!(
        collector.collect().unwrap().metrics.cpu.availability,
        RuntimeMetricAvailability::WarmingUp
    );
    thread::sleep(Duration::from_millis(1050));
    let next = collector.collect().unwrap();
    assert_eq!(source.raw_refresh_count(), 2);
    assert_eq!(
        next.metrics.cpu.availability,
        RuntimeMetricAvailability::Available
    );
    assert!(next.metrics.cpu.usage_percent.is_some());
    assert!(next
        .metrics
        .sample_interval_milliseconds
        .is_some_and(|n| n >= 1000));
}
