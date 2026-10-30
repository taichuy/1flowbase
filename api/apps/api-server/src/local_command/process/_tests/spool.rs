use super::*;

#[test]
fn local_command_output_write_failure_stops_native_execution() {
    // A genuinely non-writable file makes the real collector fail; no mocked process.
    let path = std::env::temp_dir().join(format!("command-write-error-{}", uuid::Uuid::now_v7()));
    std::fs::write(&path, []).unwrap();
    let stdout = Spool {
        file: File::open(&path).unwrap(),
        encoding_lossy: false,
    };
    let stderr = Spool::new().unwrap();
    let token = super::super::Cancellation::default();
    let guard = token.clone();
    let (done, stopped) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if stopped
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_err()
        {
            guard.cancel();
        }
    });
    let start = std::time::Instant::now();
    #[cfg(unix)]
    let result = super::super::unix::run("printf output; sleep 30", stdout, stderr, &token);
    #[cfg(windows)]
    let result = super::super::windows::run(
        "echo output & ping -n 30 127.0.0.1 >nul",
        stdout,
        stderr,
        &token,
    );
    let _ = done.send(());
    watchdog.join().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        result.is_err(),
        "write failure must not be reported as complete output"
    );
    assert!(
        !token.is_cancelled(),
        "collector must terminate the command without the test watchdog"
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
}
