//! Isolated preinstalled OpenSSH only; never reads or changes a system SSH config/key.
//! Owned PIDs are start-time checked, including sessions which leave the daemon group.
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    net::TcpListener,
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

type Owners = Arc<Mutex<BTreeMap<u32, String>>>;
pub struct LoopbackSsh {
    root: PathBuf,
    child: Option<Child>,
    owners: Owners,
    tracker: Option<tokio::task::JoinHandle<()>>,
    pub port: u16,
    pub username: String,
    pub fingerprint: String,
}
fn identity(pid: u32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Fields after comm start at field 3; starttime is field 22.
    Some(stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.into())
}
fn observe(pid: u32, owners: &mut BTreeMap<u32, String>) {
    let Ok(children) = fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) else {
        return;
    };
    for pid in children
        .split_whitespace()
        .filter_map(|pid| pid.parse().ok())
    {
        if let Some(start) = identity(pid) {
            owners.insert(pid, start);
            observe(pid, owners);
        }
    }
}
fn signal_owned(owners: &BTreeMap<u32, String>, signal: i32) {
    for (&pid, start) in owners {
        if identity(pid).as_ref() == Some(start) {
            // Only start-time-checked descendants of our freshly spawned daemon.
            unsafe {
                libc::kill(pid as i32, signal);
            }
        }
    }
}
async fn command(file: &str, args: &[&str]) -> Result<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(file)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("isolated SSH prerequisite timed out")??;
    ensure!(
        output.status.success(),
        "isolated SSH prerequisite failed: {file}: {}",
        String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(4000)])
    );
    Ok(String::from_utf8(output.stdout)?.trim().into())
}
fn quoted(path: &Path) -> Result<String> {
    let text = path.to_str().context("SSH path must be UTF8")?;
    ensure!(
        !text.contains(['\r', '\n', '"', '\\']),
        "unsupported isolated SSH path"
    );
    Ok(format!("\"{text}\""))
}
impl LoopbackSsh {
    pub async fn start() -> Result<Self> {
        for executable in [
            "/usr/sbin/sshd",
            "/usr/bin/ssh-keygen",
            "/usr/bin/python3",
            "/usr/bin/id",
        ] {
            ensure!(
                fs::metadata(executable)
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0),
                "BLOCKED: preinstalled {executable} unavailable; do not install or alter the system"
            );
        }
        let parent = PathBuf::from(std::env::var("TMPDIR")?).canonicalize()?;
        ensure!(
            fs::metadata(&parent)?.permissions().mode() & 0o077 == 0,
            "private TMPDIR required"
        );
        let root = parent.join(format!("archive-loopback-ssh-{}", Uuid::now_v7()));
        // Claim the directory atomically before constructing the cleanup guard. A failed
        // create must never remove a pre-existing path, even in a UUID collision.
        std::fs::DirBuilder::new().mode(0o700).create(&root)?;
        // Guard exists before keys/config/processes; errors and cancellation clean only this root.
        let mut fixture = Self {
            root,
            child: None,
            owners: Arc::new(Mutex::new(BTreeMap::new())),
            tracker: None,
            port: 0,
            username: String::new(),
            fingerprint: String::new(),
        };
        for name in ["host_ed25519", "user_ed25519"] {
            let path = fixture.root.join(name);
            command(
                "/usr/bin/ssh-keygen",
                &[
                    "-q",
                    "-t",
                    "ed25519",
                    "-N",
                    "",
                    "-C",
                    "R6-loopback-only",
                    "-f",
                    path.to_str().context("key path")?,
                ],
            )
            .await?;
        }
        let host_key = fixture.root.join("host_ed25519");
        let public_key = fixture.root.join("host_ed25519.pub");
        let fingerprint = command(
            "/usr/bin/ssh-keygen",
            &[
                "-l",
                "-E",
                "sha256",
                "-f",
                public_key.to_str().context("host key path")?,
            ],
        )
        .await?;
        fixture.fingerprint = fingerprint
            .split_whitespace()
            .nth(1)
            .context("host fingerprint")?
            .into();
        ensure!(
            fixture.fingerprint.starts_with("SHA256:"),
            "pinned SHA256 fingerprint required"
        );
        fixture.username = command("/usr/bin/id", &["-un"]).await?;
        ensure!(
            !fixture.username.is_empty()
                && fixture
                    .username
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
            "unsupported SSH username"
        );
        let authorized = fixture.root.join("authorized_keys");
        fs::write(
            &authorized,
            format!(
                "restrict {}",
                fs::read_to_string(fixture.root.join("user_ed25519.pub"))?
            ),
        )?;
        fs::set_permissions(&authorized, fs::Permissions::from_mode(0o600))?;
        let reservation = TcpListener::bind("127.0.0.1:0")?;
        fixture.port = reservation.local_addr()?.port();
        let config = fixture.root.join("sshd_config");
        fs::write(
            &config,
            format!(
                "Port {}\nListenAddress 127.0.0.1\nHostKey {}\nPidFile {}\nAuthorizedKeysFile {}\nAllowUsers {}\n\
             AuthenticationMethods publickey\nPubkeyAuthentication yes\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\n\
             StrictModes no\nUseDNS no\nPermitRootLogin prohibit-password\nPermitUserEnvironment no\nAllowTcpForwarding no\nAllowAgentForwarding no\n\
             X11Forwarding no\nPermitTunnel no\nPermitTTY no\nPrintMotd no\nPrintLastLog no\nMaxAuthTries 2\nMaxSessions 4\nLogLevel VERBOSE\nSetEnv PATH=/usr/bin:/bin\n",
                fixture.port,
                quoted(&host_key)?,
                quoted(&fixture.root.join("sshd.pid"))?,
                quoted(&authorized)?,
                fixture.username
            ),
        )?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600))?;
        command(
            "/usr/sbin/sshd",
            &["-t", "-f", config.to_str().context("config path")?],
        )
        .await?;
        let log = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(fixture.root.join("sshd.log"))?;
        drop(reservation); // A bind race fails readiness; never attach to an unrelated daemon.
        let child = Command::new("/usr/sbin/sshd")
            .args(["-D", "-e", "-f"])
            .arg(&config)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &fixture.root)
            .env("TMPDIR", &fixture.root)
            .env("LANG", "C.UTF-8")
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .process_group(0)
            .spawn()?;
        let pid = child.id();
        fixture.child = Some(child);
        fixture
            .owners
            .lock()
            .unwrap()
            .insert(pid, identity(pid).context("owned sshd identity missing")?);
        let owners = fixture.owners.clone();
        fixture.tracker = Some(tokio::spawn(async move {
            loop {
                {
                    let mut owned = owners.lock().unwrap();
                    observe(pid, &mut owned);
                }
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
        }));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                ensure!(
                    fixture.child.as_mut().unwrap().try_wait()?.is_none(),
                    "BLOCKED: isolated sshd exited; no system changes attempted"
                );
                if fixture.log()?.contains(&format!(
                    "Server listening on 127.0.0.1 port {}.",
                    fixture.port
                )) {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
        })
        .await
        .context("isolated loopback sshd readiness timed out")??;
        Ok(fixture)
    }
    pub fn private_key(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join("user_ed25519"))?)
    }
    fn log(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join("sshd.log"))?)
    }
    pub fn sessions(&self) -> Result<usize> {
        Ok(self.log()?.matches("Starting session: command").count())
    }
    pub fn assert_sample(&self, sample: &Value, host: &str, since: i64) -> Result<()> {
        ensure!(
            sample["host_id"] == host
                && sample["status"] == "online"
                && sample["warming_up"] == false,
            "real SSH snapshot identity/readiness mismatch"
        );
        let now = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let captured = sample["captured_at_unix_ms"]
            .as_i64()
            .context("sample timestamp missing")?;
        ensure!(
            captured >= since && i128::from(captured) <= now + 1000,
            "sample not from this request"
        );
        ensure!(
            sample["machine"]["hostname"]
                == fs::read_to_string("/proc/sys/kernel/hostname")?.trim(),
            "snapshot must describe real loopback target"
        );
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        let total: u64 = meminfo
            .lines()
            .find_map(|line| line.strip_prefix("MemTotal:"))
            .context("MemTotal missing")?
            .split_whitespace()
            .next()
            .context("MemTotal value missing")?
            .parse()?;
        ensure!(
            sample["memory"]["total_bytes"].as_f64() == Some((total * 1024) as f64),
            "snapshot memory differs from actual loopback /proc"
        );
        let available = sample["memory"]["available_bytes"]
            .as_f64()
            .context("available memory missing")?;
        ensure!(
            (0.0..=(total * 1024) as f64).contains(&available),
            "invalid memory measurement"
        );
        let usage = sample["cpu"]["usage_percent"]
            .as_f64()
            .context("actual second-sample CPU delta missing")?;
        ensure!((0.0..=100.0).contains(&usage), "invalid CPU delta");
        ensure!(
            sample["cpu"]["per_cpu_usage_percent"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "per-CPU samples missing"
        );
        for key in ["storage", "network", "disk_io", "gpus", "sensors"] {
            ensure!(sample[key].is_array(), "missing real probe array {key}");
        }
        ensure!(
            sample["pressure"].is_object(),
            "missing real pressure projection"
        );
        let log = self.log()?;
        ensure!(
            log.contains("Accepted publickey") && log.contains("Starting session: command"),
            "owned sshd must observe real key authentication and command sessions"
        );
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        if let Some(tracker) = self.tracker.take() {
            tracker.abort();
        }
        if let Some(mut child) = self.child.take() {
            let mut owned = self.owners.lock().unwrap();
            observe(child.id(), &mut owned);
            signal_owned(&owned, libc::SIGKILL);
            // The direct child is still ours even if /proc observation failed.
            let _ = child.kill();
            child.wait().context("reap owned loopback sshd")?;
        }
        if self.root.exists() {
            fs::remove_dir_all(&self.root).context("remove only owned SSH scratch/keys/config")?;
        }
        Ok(())
    }
}
impl Drop for LoopbackSsh {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
