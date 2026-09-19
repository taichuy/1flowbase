use super::super::{RuntimeProcessSampler, RuntimeProcessTerminationOutcome};

#[test]
fn collect_reports_the_current_process_with_bounded_snapshot() {
    let sampler = RuntimeProcessSampler::new();
    let snapshot = sampler.collect();

    assert!(
        snapshot.total >= snapshot.processes.len(),
        "total process count must cover the bounded sample list",
    );
    assert!(
        snapshot.processes.len() <= super::super::MAX_RUNTIME_PROCESS_SAMPLES,
        "process sample list must respect the response bound",
    );

    let current_pid = std::process::id();
    let current = snapshot
        .processes
        .iter()
        .find(|process| process.pid == current_pid)
        .expect("the current test process must be observable through the OS");

    assert!(
        !current.name.is_empty(),
        "observed process name must not be empty",
    );
    assert!(
        !current.status.is_empty(),
        "observed process status must map to a stable label",
    );
    assert!(
        current.memory_usage_percent >= 0.0,
        "memory usage percent must be non-negative",
    );
    assert!(
        !current.terminable,
        "the current process must never be marked terminable",
    );
    assert!(
        current.backend_process,
        "the current process must anchor the 1flowbase backend process tree",
    );
    assert!(
        snapshot
            .processes
            .iter()
            .all(|process| (0.0..=100.0).contains(&process.cpu_usage_percent)),
        "process CPU usage must be normalised to the whole machine (0-100%)",
    );
}

#[test]
fn collect_reuses_the_cached_snapshot_within_the_ttl() {
    let sampler = RuntimeProcessSampler::new();
    let first = sampler.collect();
    let second = sampler.collect();

    assert_eq!(
        first, second,
        "a second collection inside the TTL must reuse the cached snapshot",
    );
}

#[test]
fn terminate_refuses_the_current_process() {
    let sampler = RuntimeProcessSampler::new();
    assert_eq!(
        sampler.terminate(std::process::id()),
        RuntimeProcessTerminationOutcome::Forbidden,
        "the console process must never signal itself",
    );
}

#[test]
fn terminate_reports_a_missing_pid_as_not_observable() {
    let sampler = RuntimeProcessSampler::new();
    assert_eq!(
        sampler.terminate(u32::MAX),
        RuntimeProcessTerminationOutcome::NotObservable,
    );
}
