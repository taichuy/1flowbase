use super::*;

#[test]
fn initialization_does_not_scan_processes_and_collection_excludes_tasks() {
    let mut sampler = RuntimeMetricSampler::new();
    assert!(sampler.system.processes().is_empty());
    // Keep an owned user thread alive through collection so the scope check
    // cannot pass merely because this test process happens to be single-threaded.
    let barrier = std::sync::Barrier::new(2);
    let (sender, receiver) = std::sync::mpsc::channel();
    let (snapshot, thread_pid) = std::thread::scope(|scope| {
        scope.spawn(|| {
            let path = std::fs::read_link("/proc/thread-self").expect("owned thread PID");
            let tid = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .parse::<u32>()
                .unwrap();
            sender.send(sysinfo::Pid::from_u32(tid)).unwrap();
            barrier.wait();
        });
        let thread_pid = receiver.recv().unwrap();
        let snapshot = sampler.collect();
        barrier.wait();
        (snapshot, thread_pid)
    });
    assert!(sampler.system.process(thread_pid).is_none());
    let pid = sysinfo::get_current_pid().expect("test process PID");
    assert!(sampler.system.process(pid).is_some());
    assert!(sampler
        .system
        .processes()
        .values()
        .all(|p| p.thread_kind() != Some(sysinfo::ThreadKind::Userland)));
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

#[test]
fn late_process_metadata_does_not_shorten_the_cpu_measurement_interval() {
    let mut child = std::process::Command::new("sh")
        .args(["-c", "while :; do :; done"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("owned CPU probe");
    let result = std::panic::catch_unwind(|| {
        let pid = sysinfo::Pid::from_u32(child.id());
        // A zero-tick newly spawned process legitimately warms up on its next
        // observation. Start both controls after the owned probe has run.
        std::thread::sleep(Duration::from_millis(100));
        let mut subject = RuntimeMetricSampler::new();
        let mut control = RuntimeMetricSampler::new();
        subject.collect();
        control.collect();
        std::thread::sleep(Duration::from_millis(230));
        subject.request_process_metadata();
        subject.ensure_process_metadata();
        std::thread::sleep(Duration::from_millis(230));
        subject.collect();
        control.collect();
        let subject_cpu = subject.system.process(pid).unwrap().cpu_usage();
        let control_cpu = control.system.process(pid).unwrap().cpu_usage();
        assert!(control_cpu > 0.0, "owned probe must consume CPU");
        assert!(subject_cpu >= control_cpu * 0.8,
            "late metadata must retain the full CPU interval: subject={subject_cpu}, control={control_cpu}");
    });
    let _ = child.kill();
    let _ = child.wait();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
