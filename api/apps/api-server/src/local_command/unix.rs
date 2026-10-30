use control_plane::local_command::{LocalCommandOutput, LocalCommandPolicy};
use std::os::unix::process::ExitStatusExt;
use std::{io, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command, time::Instant};

struct ProcessGroup(Option<i32>);
impl ProcessGroup {
    fn terminate(&self) -> io::Result<()> {
        let Some(pgid) = self.0 else {
            return Ok(());
        };
        // The child leads its own process group; never signal the API's group.
        if unsafe { libc::kill(-pgid, libc::SIGKILL) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
    fn exited(&mut self) -> bool {
        let Some(pgid) = self.0 else {
            return true;
        };
        if (unsafe { libc::kill(-pgid, 0) }) == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            // Stop addressing this number once the group is gone; the OS can reuse it.
            self.0 = None;
            true
        } else {
            false
        }
    }
}
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[derive(Default)]
struct Capture {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    discarded: u64,
}
impl Capture {
    fn append(&mut self, bytes: &[u8], stderr: bool, limit: usize) -> io::Result<()> {
        let retained = bytes
            .len()
            .min(limit.saturating_sub(self.stdout.len() + self.stderr.len()));
        let target = if stderr {
            &mut self.stderr
        } else {
            &mut self.stdout
        };
        target.try_reserve(retained).map_err(io::Error::other)?;
        target.extend_from_slice(&bytes[..retained]);
        self.discarded = self
            .discarded
            .saturating_add((bytes.len() - retained) as u64);
        Ok(())
    }
}

pub(super) async fn execute(
    command: String,
    policy: LocalCommandPolicy,
) -> anyhow::Result<LocalCommandOutput> {
    let start = Instant::now();
    let deadline = start
        .checked_add(policy.timeout)
        .ok_or_else(|| anyhow::anyhow!("invalid local command deadline"))?;
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()?;
    let mut group = ProcessGroup(Some(i32::try_from(
        child.id().expect("spawned child has a PID"),
    )?));
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let mut capture = Capture::default();
    // Chunk size controls I/O granularity, not total accepted output.
    let mut out_buf = [0u8; 8192];
    let mut err_buf = [0u8; 8192];
    let (mut out_eof, mut err_eof) = (false, false);
    let mut status = None;
    let mut wait_failed = false;
    let mut timed_out = false;
    let mut stopping = false;
    let mut cleanup_deadline = deadline;
    let mut io_failed = false;
    let mut termination_failed = false;
    let mut poll = tokio::time::interval(Duration::from_millis(10));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        if stopping && status.is_some() && out_eof && err_eof && group.exited() {
            break;
        }
        tokio::select! {
            result = stdout.read(&mut out_buf), if !out_eof => {
                match result {
                    Ok(0) => out_eof = true,
                    Ok(n) => { if capture.append(&out_buf[..n], false, policy.max_output_bytes).is_err() { io_failed = true; } },
                    Err(_) => { io_failed = true; out_eof = true; },
                }
            }
            result = stderr.read(&mut err_buf), if !err_eof => {
                match result {
                    Ok(0) => err_eof = true,
                    Ok(n) => { if capture.append(&err_buf[..n], true, policy.max_output_bytes).is_err() { io_failed = true; } },
                    Err(_) => { io_failed = true; err_eof = true; },
                }
            }
            result = child.wait(), if status.is_none() && !wait_failed => {
                match result { Ok(value) => status = Some(value), Err(_) => { io_failed = true; wait_failed = true; } }
                if !stopping {
                    // A finished shell must not leave background children holding our pipes open.
                    stopping = true;
                    termination_failed = group.terminate().is_err();
                    cleanup_deadline = Instant::now() + policy.timeout;
                }
            }
            _ = tokio::time::sleep_until(deadline), if !stopping => {
                timed_out = true;
                stopping = true;
                termination_failed = group.terminate().is_err();
                cleanup_deadline = Instant::now() + policy.timeout;
            }
            _ = tokio::time::sleep_until(cleanup_deadline), if stopping => { break; }
            _ = poll.tick(), if stopping => {}
        }
        if io_failed && !stopping {
            stopping = true;
            termination_failed = group.terminate().is_err();
            cleanup_deadline = Instant::now() + policy.timeout;
        }
    }
    let cleanup_complete = status.is_some() && group.exited() && !termination_failed;
    let output_complete = out_eof && err_eof && !io_failed;
    let encoding_lossy = std::str::from_utf8(&capture.stdout).is_err()
        || std::str::from_utf8(&capture.stderr).is_err();
    Ok(LocalCommandOutput {
        stdout: String::from_utf8_lossy(&capture.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&capture.stderr).into_owned(),
        exit_code: status.and_then(|value| value.code()),
        signal: status.and_then(|value| value.signal()),
        duration_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        timed_out,
        truncated: capture.discarded > 0,
        discarded_bytes: capture.discarded,
        encoding_lossy,
        output_complete,
        cleanup_complete,
    })
}
