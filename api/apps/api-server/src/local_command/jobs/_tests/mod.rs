use super::*;

fn owner() -> JobOwner {
    JobOwner {
        user_id: Uuid::now_v7(),
        tenant_id: Uuid::now_v7(),
        current_workspace_id: Uuid::now_v7(),
    }
}
fn shell<'a>(unix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) {
        windows
    } else {
        unix
    }
}
fn input(command: &str) -> StartInput {
    StartInput {
        request_id: Uuid::now_v7(),
        command: command.into(),
        timeout_ms: 15_000,
        retention_ms: 60_000,
        max_output_bytes: 1024 * 1024,
    }
}
fn finished(registry: &JobRegistry, owner: &JobOwner, id: Uuid) -> JobSnapshot {
    let limit = Instant::now() + Duration::from_secs(20);
    loop {
        let snapshot = registry.get(owner, id).unwrap();
        if snapshot.state != JobState::Running {
            return snapshot;
        }
        assert!(Instant::now() < limit, "job did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn read(
    registry: &JobRegistry,
    owner: &JobOwner,
    id: Uuid,
    stream: OutputStream,
    offset: u64,
    max_bytes: usize,
) -> ReadOutput {
    registry
        .read(
            owner,
            id,
            ReadInput {
                stream,
                offset,
                max_bytes,
            },
        )
        .unwrap()
}

#[test]
fn jobs_idempotency_conflict_isolation_and_unknown_execution() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let request = input(shell("printf hello", "<nul set /p =hello& exit /b 0"));
    let initial = registry.start(owner.clone(), request.clone()).unwrap();
    let duplicate = registry.start(owner.clone(), request.clone()).unwrap();
    assert_eq!(initial.execution_id, duplicate.execution_id);
    let terminal = finished(&registry, &owner, initial.execution_id);
    assert_eq!(terminal.state, JobState::Completed);
    assert!(terminal.output_complete);
    assert!(
        terminal.expires_at_unix_ms.unwrap() >= terminal.created_at_unix_ms + request.retention_ms
    );
    assert_eq!(
        registry
            .start(owner.clone(), request.clone())
            .unwrap()
            .execution_id,
        initial.execution_id
    );
    let mut changed = request.clone();
    changed.command.push(' ');
    assert!(matches!(
        registry.start(owner.clone(), changed),
        Err(JobError::Conflict)
    ));
    for outsider in [
        JobOwner {
            user_id: Uuid::now_v7(),
            ..owner.clone()
        },
        JobOwner {
            tenant_id: Uuid::now_v7(),
            ..owner.clone()
        },
        JobOwner {
            current_workspace_id: Uuid::now_v7(),
            ..owner.clone()
        },
    ] {
        assert!(matches!(
            registry.get(&outsider, initial.execution_id),
            Err(JobError::NotFound)
        ));
        assert!(matches!(
            registry.cancel(&outsider, initial.execution_id),
            Err(JobError::NotFound)
        ));
        assert!(matches!(
            registry.read(
                &outsider,
                initial.execution_id,
                ReadInput {
                    stream: OutputStream::Stdout,
                    offset: 0,
                    max_bytes: 100
                }
            ),
            Err(JobError::NotFound)
        ));
        let independent = registry.start(outsider.clone(), request.clone()).unwrap();
        assert_ne!(independent.execution_id, initial.execution_id);
        finished(&registry, &outsider, independent.execution_id);
    }
    assert!(matches!(
        registry.get(&owner, Uuid::now_v7()),
        Err(JobError::NotFound)
    ));
    let restarted = JobRegistry::default();
    assert!(matches!(
        restarted.get(&owner, initial.execution_id),
        Err(JobError::NotFound)
    ));
}

#[test]
fn jobs_raw_cursor_preserves_utf8_and_rejects_too_small_budget() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let start = registry
        .start(
            owner.clone(),
            input(shell("printf 'A你好Z'", "<nul set /p =A你好Z")),
        )
        .unwrap();
    finished(&registry, &owner, start.execution_id);
    let first = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        0,
        3,
    );
    assert_eq!(first.text, "A");
    assert_eq!(first.next_offset, 1);
    assert!(!first.encoding_lossy);
    assert!(matches!(
        registry.read(
            &owner,
            start.execution_id,
            ReadInput {
                stream: OutputStream::Stdout,
                offset: 1,
                max_bytes: 2
            }
        ),
        Err(JobError::InvalidInput(_))
    ));
    let second = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        first.next_offset,
        4,
    );
    assert_eq!(second.text, "你");
    assert_eq!(second.next_offset, 4);
    assert!(!second.encoding_lossy);
    let last = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        second.next_offset,
        4,
    );
    assert_eq!(last.text, "好Z");
    assert!(last.eof);
    assert!(!last.encoding_lossy);
    assert!(matches!(
        registry.read(
            &owner,
            start.execution_id,
            ReadInput {
                stream: OutputStream::Stdout,
                offset: u64::MAX,
                max_bytes: 1
            }
        ),
        Err(JobError::InvalidInput(_))
    ));
    assert!(matches!(
        registry.read(
            &owner,
            start.execution_id,
            ReadInput {
                stream: OutputStream::Stdout,
                offset: 0,
                max_bytes: 0
            }
        ),
        Err(JobError::InvalidInput(_))
    ));
}

#[test]
fn jobs_budget_is_shared_and_nonzero_exit_is_failed() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let mut request = input(shell(
        "printf 1234567890; printf abcdefghij >&2; exit 7",
        "<nul set /p =1234567890& <nul set /p =abcdefghij>&2& exit /b 7",
    ));
    request.max_output_bytes = 7;
    let start = registry.start(owner.clone(), request).unwrap();
    let end = finished(&registry, &owner, start.execution_id);
    assert_eq!(end.state, JobState::Failed);
    assert_eq!(end.exit_code, Some(7));
    assert!(end.output_complete);
    assert!(end.truncated);
    assert_eq!(end.discarded_bytes, 13);
    assert_eq!(end.stdout_bytes + end.stderr_bytes, 7);
    let out = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        0,
        100,
    );
    let err = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stderr,
        0,
        100,
    );
    assert_eq!(out.text.len() + err.text.len(), 7);
    assert!(out.eof && err.eof);
}

#[test]
fn jobs_cancel_and_timeout_are_terminal_only_after_collection() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let command = shell(
        "printf ready; sleep 30",
        "<nul set /p =ready& ping -n 30 127.0.0.1 >nul",
    );
    let start = registry.start(owner.clone(), input(command)).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let output = read(
            &registry,
            &owner,
            start.execution_id,
            OutputStream::Stdout,
            0,
            100,
        );
        assert!(!output.eof);
        if output.text == "ready" {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let cancellation = registry.cancel(&owner, start.execution_id).unwrap();
    assert!(cancellation.cancel_requested);
    let end = finished(&registry, &owner, start.execution_id);
    assert_eq!(end.state, JobState::Cancelled);
    assert!(end.output_complete);
    assert_eq!(
        registry.cancel(&owner, start.execution_id).unwrap().state,
        JobState::Cancelled
    );
    let mut request = input(command);
    request.timeout_ms = 300;
    let timeout = registry.start(owner.clone(), request).unwrap();
    let end = finished(&registry, &owner, timeout.execution_id);
    assert_eq!(end.state, JobState::TimedOut);
    assert!(!end.cancel_requested);
}

#[test]
fn jobs_retention_cleans_automatically_and_releases_request_id() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let mut request = input(shell("printf done", "<nul set /p =done"));
    request.retention_ms = 150;
    let start = registry.start(owner.clone(), request.clone()).unwrap();
    finished(&registry, &owner, start.execution_id);
    let until = Instant::now() + Duration::from_secs(3);
    // Inspect the actual OS ownership table, without get/start opportunistic pruning.
    while !lock(&registry.jobs).is_empty() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        registry.get(&owner, start.execution_id),
        Err(JobError::NotFound)
    ));
    let next = registry.start(owner.clone(), request).unwrap();
    assert_ne!(next.execution_id, start.execution_id);
    finished(&registry, &owner, next.execution_id);
}

#[test]
fn jobs_reject_invalid_positive_budgets_and_overflow() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    for (timeout_ms, retention_ms, max_output_bytes) in [
        (0, 1, 1),
        (1, 0, 1),
        (1, 1, 0),
        (u64::MAX, 1, 1),
        (1, u64::MAX, 1),
    ] {
        let mut request = input("echo should-not-run");
        request.timeout_ms = timeout_ms;
        request.retention_ms = retention_ms;
        request.max_output_bytes = max_output_bytes;
        assert!(matches!(
            registry.start(owner.clone(), request),
            Err(JobError::InvalidInput(_))
        ));
    }
    assert!(lock(&registry.jobs).is_empty());
}

#[cfg(unix)]
#[test]
fn jobs_partial_live_utf8_is_not_replaced_and_invalid_bytes_are_reported() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let start = registry
        .start(
            owner.clone(),
            input(r"printf '\344'; sleep 1; printf '\275\240\377'"),
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while registry
        .get(&owner, start.execution_id)
        .unwrap()
        .stdout_bytes
        == 0
    {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    let partial = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        0,
        3,
    );
    assert_eq!(partial.text, "");
    assert_eq!(partial.next_offset, 0);
    assert!(!partial.encoding_lossy);
    assert!(!partial.eof);
    finished(&registry, &owner, start.execution_id);
    let full = read(
        &registry,
        &owner,
        start.execution_id,
        OutputStream::Stdout,
        0,
        10,
    );
    assert_eq!(full.text, "你�");
    assert!(full.encoding_lossy);
    assert!(full.eof);
}

#[test]
fn jobs_cancel_timeout_and_registry_drop_stop_descendants() {
    for mode in ["cancel", "timeout", "drop"] {
        let registry = Arc::new(JobRegistry::default());
        let owner = owner();
        let directory = std::env::temp_dir().join(format!("job-descendant-{}", Uuid::now_v7()));
        std::fs::create_dir(&directory).unwrap();
        let ready = directory.join("ready");
        let survived = directory.join("survived");
        let command = if cfg!(windows) {
            let child = directory.join("child.ps1");
            let parent = directory.join("parent.ps1");
            let quote = |path: &std::path::Path| path.display().to_string().replace('\'', "''");
            let child_script = format!("Set-Content -LiteralPath '{}' -Value ready; Start-Sleep -Seconds 8; Set-Content -LiteralPath '{}' -Value survived", quote(&ready), quote(&survived));
            let parent_script = format!(
                r#"$a = '-NoProfile -NonInteractive -File "{}"'; Start-Process powershell.exe -ArgumentList $a -NoNewWindow; Start-Sleep -Seconds 30"#,
                quote(&child)
            );
            std::fs::write(&child, format!("\u{feff}{child_script}")).unwrap();
            std::fs::write(&parent, format!("\u{feff}{parent_script}")).unwrap();
            format!(
                "powershell.exe -NoProfile -NonInteractive -File \"{}\"",
                parent.display()
            )
        } else {
            format!(
                "(echo ready > '{}'; sleep 2; echo survived > '{}') & wait",
                ready.display(),
                survived.display()
            )
        };
        let mut request = input(&command);
        if mode == "timeout" {
            request.timeout_ms = if cfg!(windows) { 5000 } else { 500 };
        }
        let start = registry.start(owner.clone(), request).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        if mode == "drop" {
            let weak = Arc::downgrade(&registry);
            drop(registry);
            assert!(weak.upgrade().is_none());
        } else {
            if mode == "cancel" {
                registry.cancel(&owner, start.execution_id).unwrap();
            }
            let end = finished(&registry, &owner, start.execution_id);
            assert_eq!(
                end.state,
                if mode == "cancel" {
                    JobState::Cancelled
                } else {
                    JobState::TimedOut
                }
            );
        }
        std::thread::sleep(Duration::from_millis(if cfg!(windows) {
            8200
        } else {
            2200
        }));
        assert!(!survived.exists(), "descendant survived {mode}");
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[test]
fn jobs_running_execution_survives_retention_and_concurrent_retries() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let mut request = input(shell(
        "printf ready; sleep 30",
        "<nul set /p =ready& ping -n 30 127.0.0.1 >nul",
    ));
    request.retention_ms = 50;
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let mut callers = Vec::new();
    for _ in 0..8 {
        let (registry, owner, request, barrier) = (
            registry.clone(),
            owner.clone(),
            request.clone(),
            barrier.clone(),
        );
        callers.push(std::thread::spawn(move || {
            barrier.wait();
            registry.start(owner, request).unwrap().execution_id
        }));
    }
    let ids: Vec<_> = callers
        .into_iter()
        .map(|caller| caller.join().unwrap())
        .collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        registry.get(&owner, ids[0]).unwrap().state,
        JobState::Running
    );
    registry.cancel(&owner, ids[0]).unwrap();
    assert_eq!(
        finished(&registry, &owner, ids[0]).state,
        JobState::Cancelled
    );
}

#[test]
fn jobs_budget_exhaustion_still_drains_large_stdout_and_stderr() {
    let registry = Arc::new(JobRegistry::default());
    let owner = owner();
    let mut request = input(shell(
        "(head -c 2097152 /dev/zero >&2) & head -c 2097152 /dev/zero; wait",
        "powershell.exe -NoProfile -NonInteractive -Command \"$s='x'*2097152; [Console]::Out.Write($s); [Console]::Error.Write($s)\"",
    ));
    request.max_output_bytes = 17;
    let start = registry.start(owner.clone(), request).unwrap();
    let end = finished(&registry, &owner, start.execution_id);
    assert_eq!(end.state, JobState::Completed);
    assert!(end.output_complete);
    assert_eq!(end.stdout_bytes + end.stderr_bytes, 17);
    assert_eq!(end.discarded_bytes, 4194304 - 17);
}
