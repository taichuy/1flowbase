//! Native distribution owns a private PostgreSQL instance; the API remains one process.
mod paths;
mod postgres;
mod settings;
pub mod static_web;

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
    pub fn prepare() -> Result<Option<Self>> {
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
        let data = paths::data_root(&root)?;
        settings::private_dir(&data)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(data.join("instance.lock"))?;
        lock.try_lock()
            .context("1flowbase is already running for this data directory")?;
        let http_port = settings::port("FLOWBASE_PORT", 7800)?;
        let pg_port = settings::port("FLOWBASE_POSTGRES_PORT", 35432)?;
        anyhow::ensure!(
            http_port != pg_port,
            "HTTP and PostgreSQL ports must differ"
        );
        let http_probe = TcpListener::bind(("127.0.0.1", http_port))
            .with_context(|| format!("HTTP port {http_port} is occupied; set FLOWBASE_PORT"))?;
        let pg_probe = TcpListener::bind(("127.0.0.1", pg_port)).with_context(|| {
            format!("PostgreSQL port {pg_port} is occupied; set FLOWBASE_POSTGRES_PORT")
        })?;
        let settings = Settings::load_or_create(&data)?;
        let url = format!("http://127.0.0.1:{http_port}");
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
        settings.apply(&root, &data, http_port, pg_port)?;
        // Relative storage defaults now resolve under the persistent data directory.
        env::set_current_dir(&data)?;
        drop(http_probe);
        eprintln!("1flowbase data: {}", data.display());
        eprintln!(
            "Initial root credentials: {} (root_password)",
            data.join("settings.json").display()
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
