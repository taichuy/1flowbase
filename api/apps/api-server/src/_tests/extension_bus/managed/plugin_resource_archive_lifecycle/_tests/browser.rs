//! Root owns the business assertions. This runner only supplies a live API and enforces
//! mandatory completion, bounded output/time, and ownership-scoped process cleanup.
use super::loopback_ssh::{identity, observe, signal_owned};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Child,
    task::JoinHandle,
};

pub struct BrowserConfig {
    script: PathBuf,
    pub output_dir: PathBuf,
}
impl BrowserConfig {
    pub fn load() -> Result<Self> {
        let script = PathBuf::from(std::env::var_os("PLUGIN_RESOURCE_BROWSER_SCRIPT").context(
            "R7 requires PLUGIN_RESOURCE_BROWSER_SCRIPT; browser skipping is forbidden",
        )?)
        .canonicalize()
        .context("Root browser script must exist")?;
        ensure!(script.is_file(), "Root browser script is not a file");
        let output_dir = match std::env::var_os("PLUGIN_RESOURCE_BROWSER_OUTPUT_DIR") {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(std::env::var_os("TMPDIR").context("private TMPDIR required")?)
                .join(format!("r7-browser-evidence-{}", uuid::Uuid::now_v7())),
        };
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&output_dir)?;
        let output_dir = output_dir.canonicalize()?;
        ensure!(
            fs::metadata(&output_dir)?.permissions().mode() & 0o077 == 0,
            "browser evidence output directory must be owner-private"
        );
        Ok(Self { script, output_dir })
    }
    pub async fn run(&self, input: Value) -> Result<()> {
        let mut command = tokio::process::Command::new("node");
        command
            .arg(&self.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command.as_std_mut().process_group(0);
        let child = command
            .spawn()
            .context("start mandatory Root browser subprocess")?;
        // Construct the owner before any fallible/asynchronous work after spawn.
        let mut owner = BrowserOwner::new(child);
        let stdout = owner
            .child
            .stdout
            .take()
            .context("browser stdout pipe missing")?;
        let stderr = owner
            .child
            .stderr
            .take()
            .context("browser stderr pipe missing")?;
        owner.readers.push(tokio::spawn(capture(stdout)));
        owner.readers.push(tokio::spawn(capture(stderr)));
        let execution = tokio::time::timeout(Duration::from_secs(180), async {
            let mut stdin = owner
                .child
                .stdin
                .take()
                .context("browser stdin pipe missing")?;
            let mut json = serde_json::to_vec(&input)?;
            json.push(b'\n');
            stdin.write_all(&json).await?;
            stdin.shutdown().await?;
            drop(stdin);
            owner.wait_until_exited().await
        })
        .await;
        // Always kill/reap owned residual descendants, including Chromium, before waiting
        // on pipes (a leaked grandchild can otherwise hold a pipe open indefinitely).
        let cleanup = owner.stop().await;
        let mut outputs = Vec::new();
        for reader in &mut owner.readers {
            let output = tokio::time::timeout(Duration::from_secs(5), reader).await;
            outputs.push(
                output
                    .context("bounded browser output drain timed out")??
                    .context("browser output exceeded 64KiB or could not be read")?,
            );
        }
        for (name, bytes) in ["browser.stdout.log", "browser.stderr.log"]
            .into_iter()
            .zip(&outputs)
        {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(self.output_dir.join(name))?;
            file.write_all(bytes)?;
        }
        let status = cleanup?;
        execution.context("mandatory Root browser acceptance timed out")??;
        let stdout = String::from_utf8_lossy(&outputs[0]);
        let stderr = String::from_utf8_lossy(&outputs[1]);
        ensure!(
            status.success(),
            "Root browser subprocess failed ({status}); bounded stderr: {}",
            stderr.chars().take(4000).collect::<String>()
        );
        ensure!(
            stdout
                .lines()
                .any(|line| line.trim().starts_with("R7_BROWSER_ACCEPTANCE")),
            "Root browser exited0 without R7_BROWSER_ACCEPTANCE; no browser acceptance evidence"
        );
        eprintln!(
            "R7_BROWSER_ACCEPTANCE: required Root script exited0; private output: {}",
            self.output_dir.display()
        );
        Ok(())
    }
}
async fn capture(mut pipe: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
    // Hard memory/output bound, not an unbounded Command::output allocation.
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let count = pipe.read(&mut buffer).await?;
        if count == 0 {
            return Ok(bytes);
        }
        ensure!(
            bytes.len() + count <= 64 * 1024,
            "browser output limit exceeded"
        );
        bytes.extend_from_slice(&buffer[..count]);
    }
}
struct BrowserOwner {
    child: Child,
    pid: u32,
    owners: Arc<Mutex<BTreeMap<u32, String>>>,
    tracker: JoinHandle<()>,
    readers: Vec<JoinHandle<Result<Vec<u8>>>>,
}
impl BrowserOwner {
    fn new(child: Child) -> Self {
        let pid = child.id().expect("new browser child has PID");
        let mut observed = BTreeMap::new();
        if let Some(start) = identity(pid) {
            observed.insert(pid, start);
        }
        let owners = Arc::new(Mutex::new(observed));
        let tracking = owners.clone();
        let tracker = tokio::spawn(async move {
            loop {
                {
                    let mut owners = tracking.lock().unwrap();
                    // Observe already-owned descendants too, even after Node exits/reparents.
                    let pids: Vec<_> = owners.keys().copied().collect();
                    for pid in pids {
                        if owners.get(&pid) == identity(pid).as_ref() {
                            observe(pid, &mut owners);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        });
        Self {
            child,
            pid,
            owners,
            tracker,
            readers: Vec::new(),
        }
    }
    fn kill_owned(&mut self) {
        let mut owners = self.owners.lock().unwrap();
        if owners.get(&self.pid) == identity(self.pid).as_ref() {
            observe(self.pid, &mut owners);
        }
        // Only our own still-identifiable process group, never an arbitrary/reused PID.
        if owners.iter().any(|(&pid, start)| {
            identity(pid).as_ref() == Some(start)
                && unsafe { libc::getpgid(pid as i32) } == self.pid as i32
        }) {
            unsafe {
                libc::kill(-(self.pid as i32), libc::SIGKILL);
            }
        }
        signal_owned(&owners, libc::SIGKILL);
        let _ = self.child.start_kill();
    }
    async fn wait_until_exited(&self) -> Result<()> {
        loop {
            // WNOWAIT retains our child/PID/process-group identity until stop() kills
            // residual descendants. Reaping first would lose the group ownership fence.
            let mut status: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid,
                    &mut status,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error()).context("observe owned browser exit");
            }
            if unsafe { status.si_pid() } == self.pid as i32 {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    async fn stop(&mut self) -> Result<std::process::ExitStatus> {
        self.kill_owned();
        let status = self
            .child
            .wait()
            .await
            .context("reap owned browser subprocess")?;
        // Let killed grandchildren disappear/reap before declaring cleanup complete.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let alive = self.owners.lock().unwrap().iter().any(|(&pid, start)| {
                    identity(pid).as_ref() == Some(start)
                        && fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                            stat.rsplit_once(')')
                                .is_some_and(|(_, rest)| !rest.trim_start().starts_with('Z'))
                        })
                });
                if !alive {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .context("owned browser descendant cleanup timed out")?;
        self.tracker.abort();
        Ok(status)
    }
}
impl Drop for BrowserOwner {
    fn drop(&mut self) {
        self.kill_owned();
        self.tracker.abort();
        for reader in &self.readers {
            reader.abort();
        }
        // kill_on_drop covers cancellation while awaiting wait(); Tokio owns reaping.
    }
}
