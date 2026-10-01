use super::*;

#[test]
fn console_sampler_retains_processes_without_enumerating_tasks() {
    let sampler = RuntimeProcessSampler::new();
    let snapshot = sampler.collect();
    let pid = std::process::id();
    let current = snapshot
        .processes
        .iter()
        .find(|p| p.pid == pid)
        .expect("current process");
    assert!(current.backend_process);
    assert!(!current.terminable);
    let inner = sampler.inner.lock().expect("sampler lock");
    assert!(inner
        .system
        .processes()
        .values()
        .all(|p| p.thread_kind().is_none()));
}
