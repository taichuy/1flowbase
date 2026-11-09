use super::{collect, Cancellation, Collected, Spool};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::{
    io,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

// A successful group signal, a reaped leader, and an absent group are different facts.
// Darwin filters zombies in killpg1 and may return EPERM for the remaining group:
// https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_sig.c#L1709-L1718
#[derive(Default)]
struct GroupState {
    signalled: AtomicBool,
    leader_reaped: AtomicBool,
    absent: AtomicBool,
}
impl GroupState {
    fn signal_result(&self, result: io::Result<()>) -> io::Result<()> {
        match result {
            Ok(()) => {
                self.signalled.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
                self.absent.store(true, Ordering::Release);
                Ok(())
            }
            Err(error)
                if error.raw_os_error() == Some(libc::EPERM)
                    && self.signalled.load(Ordering::Acquire)
                    && self.leader_reaped.load(Ordering::Acquire) =>
            {
                // The first SIGKILL succeeded and wait proved the owned shell exited.
                // A later denial does not undo those facts or discard collected output.
                // Do not claim the group is absent: exited() must establish that separately.
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    fn probe_result(&self, result: io::Result<()>) -> bool {
        let absent = result.is_err_and(|error| error.raw_os_error() == Some(libc::ESRCH));
        if absent {
            self.absent.store(true, Ordering::Release);
        }
        absent
    }
}

struct Group {
    pid: i32,
    state: GroupState,
}
impl Group {
    fn signal(&self, signal: i32) -> io::Result<()> {
        if unsafe { libc::kill(-self.pid, signal) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // Native CI must retain the actual syscall error if a platform branch fails.
        #[cfg(test)]
        if signal != 0 && error.raw_os_error() != Some(libc::ESRCH) {
            eprintln!(
                "local-command killpg pgid={} signal={} signalled={} leader_reaped={} error={:?}",
                self.pid,
                signal,
                self.state.signalled.load(Ordering::Acquire),
                self.state.leader_reaped.load(Ordering::Acquire),
                error,
            );
        }
        Err(error)
    }
    fn stop(&self) -> io::Result<()> {
        // Signal again after reaping: a successful killpg alone is not proof of an empty
        // group, and the shell may have been forking when the first signal was sent.
        if self.state.absent.load(Ordering::Acquire) {
            return Ok(());
        }
        self.state.signal_result(self.signal(libc::SIGKILL))
    }
    fn exited(&self) -> bool {
        self.state.probe_result(self.signal(0))
    }
    fn leader_reaped(&self) {
        self.state.leader_reaped.store(true, Ordering::Release);
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
struct OwnedChild<'a> {
    child: Child,
    group: &'a Group,
}
impl Drop for OwnedChild<'_> {
    fn drop(&mut self) {
        let _ = self.group.stop();
        if self.child.wait().is_ok() {
            self.group.leader_reaped();
        }
    }
}

pub(super) fn run(
    command: &str,
    stdout: Spool,
    stderr: Spool,
    cancellation: &Cancellation,
) -> io::Result<Collected> {
    let child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let group = Group {
        pid: child.id() as i32,
        state: GroupState::default(),
    };
    let mut child = OwnedChild {
        child,
        group: &group,
    };
    let out = child.child.stdout.take().expect("piped stdout");
    let err = child.child.stderr.take().expect("piped stderr");
    let mut result = collect(out, err, stdout, stderr, &|| group.stop(), || {
        loop {
            if cancellation.is_cancelled() {
                group.stop()?;
                // Reap after signalling instead of signalling on every poll. On
                // macOS a group containing only an unreaped zombie can return
                // EPERM; that is not a failure to terminate the owned child.
                let status = child.child.wait()?;
                group.leader_reaped();
                return Ok((status.code().map(i64::from), status.signal()));
            }
            if let Some(status) = child.child.try_wait()? {
                group.leader_reaped();
                return Ok((status.code().map(i64::from), status.signal()));
            }
            std::thread::sleep(Duration::from_millis(10)); // Cancellation polling, no command deadline.
        }
    })?;
    result.cleanup_complete = group.exited();
    Ok(result)
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/unix.rs"]
mod tests;
