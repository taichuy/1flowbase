//! Native distribution owns a private PostgreSQL instance; the API remains one process.
mod env_file;
mod postgres;
mod settings;
pub mod static_web;
pub mod upgrade;

use anyhow::{Context, Result};
use settings::Settings;
use std::{
    env,
    fs::{self, File},
    net::TcpListener,
    path::PathBuf,
    process::Command,
};

pub struct NativeDistribution {
    pub web_root: PathBuf,
    postgres: postgres::Postgres,
    _instance_lock: File,
    url: String,
    pg_port: u16,
    postgres_password: String,
}

impl NativeDistribution {
    pub fn prepare(check_startup: bool) -> Result<Option<Self>> {
        let root = env::current_exe()?
            .parent()
            .context("executable has no parent")?
            .to_path_buf();
        // Only packaged executables opt in. Ordinary API / Docker startup stays unchanged.
        if !root.join("native-distribution.json").is_file() {
            return Ok(None);
        }
        anyhow::ensure!(
            root.join("web/index.html").is_file(),
            "Incomplete native package: web/index.html"
        );
        anyhow::ensure!(
            check_startup || !root.join(".upgrade-state.json").exists(),
            "An update is incomplete; run upgrade.cmd / upgrade.sh again to continue (no backup was created)"
        );
        let configuration = Settings::inspect(&root)?;
        let data = configuration.data.clone();
        settings::private_dir(&data)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(data.join("instance.lock"))?;
        lock.try_lock()
            .context("1flowbase is already running for this data directory")?;
        let http_port = configuration.address.port();
        let pg_port = configuration.postgres_port;
        anyhow::ensure!(
            http_port != pg_port,
            "HTTP and PostgreSQL ports must differ"
        );
        let http_probe = TcpListener::bind(configuration.address).with_context(|| {
            format!("HTTP port {http_port} is occupied; edit API_SERVER_ADDR in .env")
        })?;
        let pg_probe = TcpListener::bind(("127.0.0.1", pg_port)).with_context(|| {
            format!("PostgreSQL port {pg_port} is occupied; edit API_DATABASE_URL in .env")
        })?;
        let settings = Settings::load(&root)?;
        let mut browser_address = configuration.address;
        if browser_address.ip().is_unspecified() {
            browser_address.set_ip(if browser_address.is_ipv6() {
                std::net::Ipv6Addr::LOCALHOST.into()
            } else {
                std::net::Ipv4Addr::LOCALHOST.into()
            });
        }
        let url = format!("http://{browser_address}");
        let postgres = postgres::Postgres::new(&root.join("postgres"), &data);
        postgres.validate()?;
        let native = Self {
            web_root: root.join("web"),
            postgres,
            _instance_lock: lock,
            url,
            pg_port,
            postgres_password: settings.postgres_password.clone(),
        };
        drop(pg_probe);
        settings.apply(&root)?;
        // Relative storage defaults now resolve under the persistent data directory.
        env::set_current_dir(&data)?;
        drop(http_probe);
        eprintln!("1flowbase data: {}", data.display());
        eprintln!(
            "Initial root credentials: {} (BOOTSTRAP_ROOT_ACCOUNT / BOOTSTRAP_ROOT_PASSWORD)",
            root.join(".env").display()
        );
        Ok(Some(native))
    }

    pub fn start_postgres(mut self) -> Result<Self> {
        self.postgres.start(self.pg_port, &self.postgres_password)?;
        Ok(self)
    }

    pub fn announce_ready(&self) {
        eprintln!("1flowbase ready: {} (Ctrl+C to stop)", self.url);
        if env::var("FLOWBASE_OPEN_BROWSER").as_deref() == Ok("false") {
            return;
        }
        #[cfg(target_os = "windows")]
        let command = {
            let mut c = Command::new("rundll32.exe");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(target_os = "macos")]
        let command = Command::new("open");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let command = Command::new("xdg-open");
        // Browser opening is optional on headless hosts; reap the helper process.
        let url = self.url.clone();
        std::thread::spawn(move || {
            let mut command = command;
            let _ = command.arg(url).status();
        });
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
