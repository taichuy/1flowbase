use super::{collect, Cancellation, Collected, Spool};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::{
    io,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

struct Group {
    pid: i32,
    stopped: AtomicBool,
}
impl Group {
    fn stop(&self) -> io::Result<()> {
        // A reader failure can signal while the shell is forking. Signal again after
        // reaping the shell; one successful killpg is not proof that the group is empty.
        if self.stopped.load(Ordering::Acquire) {
            return Ok(());
        }
        if unsafe { libc::kill(-self.pid, libc::SIGKILL) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            self.stopped.store(true, Ordering::Release);
            Ok(())
        } else {
            Err(error)
        }
    }
    fn exited(&self) -> bool {
        let exited = (unsafe { libc::kill(-self.pid, 0) }) == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if exited {
            self.stopped.store(true, Ordering::Release);
        }
        exited
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
        let _ = self.child.wait();
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
        stopped: AtomicBool::new(false),
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
            }
            if let Some(status) = child.child.try_wait()? {
                return Ok((status.code().map(i64::from), status.signal()));
            }
            std::thread::sleep(Duration::from_millis(10)); // Cancellation polling, no command deadline.
        }
    })?;
    result.cleanup_complete = group.exited();
    Ok(result)
}
