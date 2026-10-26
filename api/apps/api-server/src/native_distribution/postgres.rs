use super::settings::private_file;
use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(super) struct Postgres {
    root: PathBuf,
    data: PathBuf,
    started: bool,
}

impl Postgres {
    pub fn new(root: &Path, data: &Path) -> Self {
        Self {
            root: root.to_owned(),
            data: data.to_owned(),
            started: false,
        }
    }
    fn command(&self, name: &str) -> Command {
        let mut command = Command::new(
            self.root
                .join("bin")
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        );
        command.stdin(Stdio::null());
        // Do not inherit an operator's unrelated PostgreSQL connection/service configuration.
        for key in [
            "PGHOST",
            "PGPORT",
            "PGDATABASE",
            "PGUSER",
            "PGPASSWORD",
            "PGSERVICE",
            "PGSERVICEFILE",
            "PGOPTIONS",
        ] {
            command.env_remove(key);
        }
        command
    }
    pub fn validate(&self) -> Result<()> {
        for name in [
            "postgres",
            "pg_ctl",
            "initdb",
            "psql",
            "createdb",
            "pg_dump",
            "pg_restore",
        ] {
            anyhow::ensure!(
                self.root
                    .join("bin")
                    .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
                    .is_file(),
                "Missing bundled PostgreSQL tool: {name}"
            );
        }
        let output = self.command("pg_ctl").arg("--version").output()?;
        anyhow::ensure!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains(" 18."),
            "Native distribution requires PostgreSQL 18.x"
        );
        let version = self.data.join("postgres/PG_VERSION");
        if version.exists() {
            anyhow::ensure!(
                fs::read_to_string(version)?.trim() == "18",
                "Database major version differs; migrate explicitly before upgrading"
            );
        }
        Ok(())
    }
    pub fn start(&mut self, port: u16, password: &str) -> Result<()> {
        let pgdata = self.data.join("postgres");
        // Never adopt another PostgreSQL process, even if it uses the expected directory.
        anyhow::ensure!(
            !self
                .command("pg_ctl")
                .arg("status")
                .arg("-D")
                .arg(&pgdata)
                .output()?
                .status
                .success(),
            "PostgreSQL is already running for this data directory; stop/recover it explicitly"
        );
        if !pgdata.join("PG_VERSION").is_file() {
            let password_file = self
                .data
                .join(format!("initdb-{}.password", std::process::id()));
            let mut file = private_file(&password_file)?;
            file.write_all(password.as_bytes())?;
            drop(file);
            let result = self
                .command("initdb")
                .arg("-D")
                .arg(&pgdata)
                .args([
                    "-U",
                    "postgres",
                    "--encoding=UTF8",
                    "--locale=C",
                    "--auth=scram-sha-256",
                ])
                .arg(format!("--pwfile={}", password_file.display()))
                .status();
            let _ = fs::remove_file(password_file);
            anyhow::ensure!(
                result
                    .context("Could not execute bundled initdb")?
                    .success(),
                "PostgreSQL initialization failed; inspect the data directory before retrying"
            );
        }
        let log = self.data.join("postgres.log");
        self.started = true;
        let status = self
            .command("pg_ctl")
            .args(["start", "-w", "-t", "60"])
            .arg("-D")
            .arg(&pgdata)
            .arg("-l")
            .arg(&log)
            .arg("-o")
            .arg(format!(
                "-p {port} -h 127.0.0.1 -c unix_socket_directories=''"
            ))
            .status()?;
        anyhow::ensure!(
            status.success(),
            "PostgreSQL start failed; see {}",
            log.display()
        );
        let port = port.to_string();
        let output = self
            .command("psql")
            .args([
                "-X",
                "-w",
                "-h",
                "127.0.0.1",
                "-p",
                &port,
                "-U",
                "postgres",
                "-d",
                "postgres",
                "-Atc",
                "SELECT 1 FROM pg_database WHERE datname = '1flowbase'",
            ])
            .env("PGPASSWORD", password)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "Cannot inspect bundled database: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if String::from_utf8_lossy(&output.stdout).trim() != "1" {
            anyhow::ensure!(
                self.command("createdb")
                    .args([
                        "-w",
                        "-h",
                        "127.0.0.1",
                        "-p",
                        &port,
                        "-U",
                        "postgres",
                        "1flowbase"
                    ])
                    .env("PGPASSWORD", password)
                    .status()?
                    .success(),
                "Could not create 1flowbase database"
            );
        }
        Ok(())
    }
}

impl Drop for Postgres {
    fn drop(&mut self) {
        if self.started {
            match self.command("pg_ctl").args(["stop", "-w", "-t", "60", "-m", "fast"])
                .arg("-D").arg(self.data.join("postgres")).status() {
                Ok(status) if status.success() => {},
                result => eprintln!("PostgreSQL shutdown did not complete: {result:?}; preserve the data directory and inspect postgres.log"),
            }
        }
    }
}
