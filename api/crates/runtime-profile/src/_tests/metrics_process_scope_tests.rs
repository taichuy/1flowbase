use super::*;

#[test]
fn initialization_does_not_scan_processes_and_collection_excludes_tasks() {
    let mut sampler = RuntimeMetricSampler::new();
    assert!(sampler.system.processes().is_empty());
    let snapshot = sampler.collect();
    let pid = sysinfo::get_current_pid().expect("test process PID");
    assert!(sampler.system.process(pid).is_some());
    assert!(sampler
        .system
        .processes()
        .values()
        .all(|p| p.thread_kind().is_none()));
    assert!(snapshot.memory.process_bytes > 0);
    assert!(snapshot.memory.related_process_count >= 1);
    assert!(snapshot.cpu.logical_count > 0);
}

#[test]
fn process_only_collection_preserves_child_memory_and_ancestry() {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("owned child process");
    let result = std::panic::catch_unwind(|| {
        let mut sampler = RuntimeMetricSampler::new();
        let snapshot = sampler.collect();
        let child_pid = sysinfo::Pid::from_u32(child.id());
        let process = sampler
            .system
            .process(child_pid)
            .expect("child remains observable");
        assert_eq!(process.parent(), sysinfo::get_current_pid().ok());
        assert!(process.thread_kind().is_none());
        assert!(snapshot.memory.related_process_count >= 2);
        assert!(snapshot.memory.related_process_bytes >= snapshot.memory.process_bytes);
    });
    let _ = child.kill();
    let _ = child.wait();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
