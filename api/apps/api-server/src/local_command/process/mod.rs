//! OS-native one-shot execution. Also compiled directly by the native-platform fixture.
use serde::Serialize;
use std::{
    fs::File,
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

mod spool;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
use spool::Spool;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Serialize)]
pub struct ExecutionMetadata {
    pub exit_code: Option<i64>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub encoding_lossy: bool,
    pub output_complete: bool,
    pub cleanup_complete: bool,
}

pub struct CompletedCommand {
    pub metadata: ExecutionMetadata,
    stdout: Spool,
    stderr: Spool,
}

pub enum ResponsePart {
    Bytes(Vec<u8>),
    File(File),
}
impl CompletedCommand {
    /// Same ordinary JSON envelope as other console APIs, without materializing stdout/stderr.
    pub fn into_json_parts(self) -> io::Result<Vec<ResponsePart>> {
        let metadata = serde_json::to_vec(&self.metadata)?;
        let mut suffix = b"\",".to_vec();
        suffix.extend_from_slice(&metadata[1..]);
        suffix.extend_from_slice(b",\"meta\":null}");
        Ok(vec![
            ResponsePart::Bytes(b"{\"data\":{\"stdout\":\"".to_vec()),
            ResponsePart::File(self.stdout.into_file()?),
            ResponsePart::Bytes(b"\",\"stderr\":\"".to_vec()),
            ResponsePart::File(self.stderr.into_file()?),
            ResponsePart::Bytes(suffix),
        ])
    }
}

pub(super) struct Collected {
    stdout: Spool,
    stderr: Spool,
    exit_code: Option<i64>,
    signal: Option<i32>,
    cleanup_complete: bool,
}

pub fn execute(command: &str, cancellation: &Cancellation) -> io::Result<CompletedCommand> {
    if command.trim().is_empty() || command.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid local command",
        ));
    }
    if cancellation.is_cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let start = Instant::now();
    // Reserve the output carriers before starting any command side effects.
    let stdout = Spool::new()?;
    let stderr = Spool::new()?;
    #[cfg(unix)]
    let result = unix::run(command, stdout, stderr, cancellation)?;
    #[cfg(windows)]
    let result = windows::run(command, stdout, stderr, cancellation)?;
    #[cfg(not(any(unix, windows)))]
    compile_error!("local commands require Unix or Windows");
    if cancellation.is_cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    Ok(CompletedCommand {
        metadata: ExecutionMetadata {
            exit_code: result.exit_code,
            signal: result.signal,
            duration_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            timed_out: false,
            truncated: false,
            discarded_bytes: 0,
            encoding_lossy: result.stdout.encoding_lossy || result.stderr.encoding_lossy,
            output_complete: true,
            cleanup_complete: result.cleanup_complete,
        },
        stdout: result.stdout,
        stderr: result.stderr,
    })
}

fn collect<R: Read + Send, E: Read + Send>(
    stdout: R,
    stderr: E,
    stdout_file: Spool,
    stderr_file: Spool,
    stop: &(impl Fn() -> io::Result<()> + Sync),
    wait: impl FnOnce() -> io::Result<(Option<i64>, Option<i32>)>,
) -> io::Result<Collected> {
    std::thread::scope(|scope| {
        let out = std::thread::Builder::new()
            .name("command-stdout".into())
            .spawn_scoped(scope, || {
                spool::capture(stdout, stdout_file).inspect_err(|_| {
                    let _ = stop();
                })
            })
            .inspect_err(|_| {
                let _ = stop();
            })?;
        let err = std::thread::Builder::new()
            .name("command-stderr".into())
            .spawn_scoped(scope, || {
                spool::capture(stderr, stderr_file).inspect_err(|_| {
                    let _ = stop();
                })
            })
            .inspect_err(|_| {
                let _ = stop();
            })?;
        let status = wait();
        // A completed shell must not leave ordinary background children holding its pipes.
        let cleanup = stop();
        let stdout = out
            .join()
            .map_err(|_| io::Error::other("stdout reader panicked"))?;
        let stderr = err
            .join()
            .map_err(|_| io::Error::other("stderr reader panicked"))?;
        let (exit_code, signal) = status?;
        cleanup?;
        Ok(Collected {
            stdout: stdout?,
            stderr: stderr?,
            exit_code,
            signal,
            cleanup_complete: false,
        })
    })
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
