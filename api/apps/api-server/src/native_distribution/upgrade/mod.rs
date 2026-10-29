//! Offline, explicit-target maintenance. A complete cold snapshot precedes every mutation.
mod files;
mod manifest;

use super::settings::{private_dir, Settings};
use anyhow::{bail, ensure, Context, Result};
use manifest::Manifest;
#[cfg(test)]
use manifest::NAME;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const STATE: &str = ".upgrade-state.json";

#[derive(Serialize, Deserialize)]
struct Receipt {
    format_version: u32,
    target: PathBuf,
    data: PathBuf,
    old_version: String,
    new_version: String,
    managed_roots: BTreeSet<String>,
    snapshot: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct State {
    backup: PathBuf,
    phase: String,
    error: Option<String>,
}

pub fn handle_cli(
    arguments: &[String],
    validate_config: impl FnOnce(&super::env_file::Values) -> Result<()>,
) -> Result<bool> {
    let Some(mode) = arguments.first() else {
        return Ok(false);
    };
    if mode == "--check-startup" {
        ensure!(arguments.len() == 1, "Unexpected startup-check arguments");
        return Ok(false);
    }
    let source = std::env::current_exe()?
        .parent()
        .context("Executable has no directory")?
        .canonicalize()?;
    match mode.as_str() {
        "--upgrade" => {
            ensure!(arguments.len() <= 2, "Usage: upgrade <existing-installation>");
            let target = if let Some(path) = arguments.get(1) { PathBuf::from(path) } else {
                print!("Existing installation directory: "); io::stdout().flush()?;
                let mut path = String::new(); io::stdin().read_line(&mut path)?;
                ensure!(!path.trim().is_empty(), "No installation selected");
                PathBuf::from(path.trim().trim_matches('"'))
            };
            let backup = upgrade(&source, &target, validate_config, validate_startup)?;
            println!("Upgrade verified. Configuration and data preserved. Backup: {}\nStart {} to use the updated application.", backup.display(), target.join(Manifest::executable()).display());
        }
        "--restore" => {
            ensure!(arguments.len() == 3, "Usage: upgrade --restore <installation> <backup-directory>");
            restore(Path::new(&arguments[1]), Path::new(&arguments[2]))?;
            println!("Matching program, configuration and data restored. Start the application normally.");
        }
        _ => bail!("Unknown argument. Use --upgrade <installation>, --restore <installation> <backup>, or no arguments to start."),
    }
    Ok(true)
}

fn lock(path: &Path) -> Result<File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().with_context(|| {
        format!(
            "Instance or maintenance operation still running: {}",
            path.display()
        )
    })?;
    Ok(file)
}

struct StoppedGuard {
    _instance: File,
    _http: TcpListener,
    _postgres: TcpListener,
}

fn inspect_stopped(target: &Path) -> Result<(Settings, StoppedGuard)> {
    inspect_stopped_config(target, Settings::inspect(target)?)
}

fn inspect_stopped_config(target: &Path, settings: Settings) -> Result<(Settings, StoppedGuard)> {
    ensure!(
        settings.data.is_dir(),
        "Initialize this installation before upgrading it"
    );
    let data = settings.data.canonicalize()?;
    ensure!(
        !target.starts_with(&data) && (data == target.join("data") || !data.starts_with(target)),
        "Data must be data/ or a separate directory outside the program tree"
    );
    for key in [
        "API_BUSINESS_FILE_LOCAL_ROOT",
        "API_PROVIDER_INSTALL_ROOT",
        "API_HOST_EXTENSION_DROPIN_ROOT",
        "API_MCP_TEMPLATE_LIBRARY_ROOT",
        "API_SYSTEM_BACKUP_REPOSITORY_ROOT",
    ] {
        let path =
            super::settings::resolve(target, super::env_file::required(&settings.values, key)?);
        // Missing optional subdirectories are allowed, but lexical escape is not.
        ensure!(
            !path
                .components()
                .any(|c| c == std::path::Component::ParentDir)
                && path.starts_with(&settings.data),
            "{key} must be within FLOWBASE_DATA_DIR for a complete offline upgrade backup"
        );
    }
    let instance = lock(&data.join("instance.lock"))?;
    ensure!(
        !data.join("postgres/postmaster.pid").exists(),
        "PostgreSQL must be stopped normally before maintenance; do not delete its PID file"
    );
    let _http = TcpListener::bind(settings.address)
        .context("HTTP port is occupied; stop the old application first")?;
    let _pg = TcpListener::bind(("127.0.0.1", settings.postgres_port))
        .context("PostgreSQL port is occupied; stop the owned database first")?;
    if data.join("postgres/PG_VERSION").exists() {
        ensure!(
            fs::read_to_string(data.join("postgres/PG_VERSION"))?.trim() == "18",
            "Unsupported data cluster major version"
        );
    }
    Ok((
        settings,
        StoppedGuard {
            _instance: instance,
            _http,
            _postgres: _pg,
        },
    ))
}

fn copy_program(manifest: &Manifest, source: &Path, destination: &Path) -> Result<()> {
    if !destination.exists() {
        private_dir(destination)?;
    }
    for name in manifest.roots() {
        files::copy(&source.join(&name), &destination.join(&name), false)?;
    }
    Ok(())
}

fn set_state(target: &Path, backup: &Path, phase: &str, error: Option<String>) -> Result<()> {
    files::json(
        &target.join(STATE),
        &State {
            backup: backup.to_owned(),
            phase: phase.into(),
            error,
        },
    )
}

fn upgrade(
    source: &Path,
    target: &Path,
    validate_config: impl FnOnce(&super::env_file::Values) -> Result<()>,
    validate: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<PathBuf> {
    let source = source.canonicalize()?;
    let target = target.canonicalize()?;
    ensure!(
        !source.starts_with(&target) && !target.starts_with(&source),
        "Extract the new package outside the existing installation; paths must not overlap"
    );
    let _maintenance = lock(&target.join(".upgrade.lock"))?;
    ensure!(
        !target.join(STATE).exists(),
        "An earlier upgrade is incomplete; restore its recorded backup first"
    );
    let new = Manifest::read(&source)?;
    let old = Manifest::read(&target)?;
    new.compatible_with(&old)?;
    new.verify(&source)?;
    old.verify(&target)?;
    #[cfg(windows)]
    for name in old.roots() {
        files::check_unlocked(&target.join(name))?;
    }
    let (settings, instance) = inspect_stopped(&target)?;
    validate_config(&settings.values).context("Existing .env is not valid for the new application; update required configuration before upgrading")?;
    let data = settings.data.canonicalize()?;
    ensure!(
        data.join("postgres/PG_VERSION").is_file(),
        "Initialize the installation before upgrading it"
    );
    ensure!(
        !source.starts_with(&data) && !data.starts_with(&source),
        "New package overlaps the data directory"
    );
    // Required keys must already exist in the user file; installing defaults must never overwrite it.
    let parent = target
        .parent()
        .context("Installation cannot be a filesystem root")?;
    let backup_root = parent.join(format!(
        ".{}-upgrade-backups",
        target
            .file_name()
            .context("Invalid installation path")?
            .to_string_lossy()
    ));
    ensure!(
        !backup_root.starts_with(&data),
        "Backup location overlaps data"
    );
    let program_bytes = old
        .roots()
        .iter()
        .try_fold(0_u64, |n, name| -> Result<u64> {
            Ok(n.saturating_add(files::size(&target.join(name))?))
        })?;
    let source_bytes = new
        .roots()
        .iter()
        .try_fold(0_u64, |n, name| -> Result<u64> {
            Ok(n.saturating_add(files::size(&source.join(name))?))
        })?;
    files::require_space(
        parent,
        program_bytes
            .saturating_add(source_bytes)
            .saturating_add(files::size(&data)?)
            .saturating_add(files::size(&target.join(".env"))?),
    )?;
    if backup_root.try_exists()? {
        let info = fs::symlink_metadata(&backup_root)?;
        ensure!(
            info.is_dir() && !info.file_type().is_symlink(),
            "Backup root must be a real directory"
        );
    } else {
        private_dir(&backup_root)?;
    }
    files::sync_directory(parent)?;
    let backup = backup_root.join(uuid::Uuid::now_v7().to_string());
    fs::create_dir(&backup)?;
    private_dir(&backup)?;
    files::sync_directory(&backup_root)?;
    let snapshot = backup.join("snapshot");
    private_dir(&snapshot)?;
    copy_program(&old, &target, &snapshot.join("program"))?;
    files::copy(&target.join(".env"), &snapshot.join("program/.env"), false)?;
    files::copy(&data, &snapshot.join("data"), true)?;
    // Stage and re-verify before changing the installation, including source mutation during copy.
    let staged = backup.join("staged");
    copy_program(&new, &source, &staged)?;
    new.verify(&staged)?;
    let roots: BTreeSet<_> = old.roots().union(&new.roots()).cloned().collect();
    ensure!(
        !data.join("postgres/postmaster.pid").exists(),
        "Database started during snapshot; installation untouched"
    );
    files::sync_directory(&snapshot)?;
    let receipt = Receipt {
        format_version: 1,
        target: target.clone(),
        data: data.clone(),
        old_version: old.version,
        new_version: new.version.clone(),
        managed_roots: roots,
        snapshot: files::inventory(&snapshot)?,
    };
    files::json(&backup.join("receipt.json"), &receipt)?;
    // Receipt exists only after every snapshot file is durable; incomplete staging is never restorable.
    set_state(&target, &backup, "replacing", None)?;
    let outcome = (|| -> Result<()> {
        for name in &receipt.managed_roots {
            files::remove(&target.join(name))?;
        }
        for name in new.roots() {
            fs::rename(staged.join(&name), target.join(&name))?;
        }
        new.verify(&target)?;
        set_state(&target, &backup, "validating", None)?;
        drop(instance); // The validation child takes the same data lock; ordinary launch is blocked by STATE.
        validate(&target, &backup)?;
        let (_, _instance) = inspect_stopped(&target)?;
        files::json(
            &backup.join("completed.json"),
            &serde_json::json!({"version": new.version, "build_id": new.build_id}),
        )?;
        fs::remove_file(target.join(STATE))?;
        files::sync_directory(&target)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        let _ = set_state(&target, &backup, "failed", Some(format!("{error:#}")));
        bail!("Upgrade incomplete: {error:#}\nBackup: {}\nAfter confirming the application and PostgreSQL are stopped, run the NEW package's upgrade script with --restore \"{}\" \"{}\"", backup.display(), target.display(), backup.display());
    }
    Ok(backup)
}

fn validate_startup(target: &Path, backup: &Path) -> Result<()> {
    let log = File::create(backup.join("startup-validation.log"))?;
    let mut child = Command::new(target.join(Manifest::executable()))
        .arg("--check-startup")
        .current_dir(target)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if let Some(status) = child.try_wait()? {
            ensure!(
                status.success(),
                "New application initialization failed; see startup-validation.log"
            );
            return Ok(());
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            // This exact cluster was stopped before validation, and can only have been started by our child.
            let config = Settings::inspect(target)?;
            let _ = Command::new(target.join(format!(
                "postgres/bin/pg_ctl{}",
                std::env::consts::EXE_SUFFIX
            )))
            .args(["stop", "-w", "-t", "60", "-m", "fast", "-D"])
            .arg(config.data.join("postgres"))
            .status();
            bail!("Startup validation timed out; inspect the log and confirm PostgreSQL stopped before restoring");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn restore(target: &Path, backup: &Path) -> Result<()> {
    let target = target.canonicalize()?;
    let backup = backup.canonicalize()?;
    ensure!(
        !backup.starts_with(&target) && !target.starts_with(&backup),
        "Backup and installation must not overlap"
    );
    let _maintenance = lock(&target.join(".upgrade.lock"))?;
    let receipt: Receipt = serde_json::from_slice(&fs::read(backup.join("receipt.json"))?)?;
    ensure!(
        receipt.format_version == 1
            && receipt.target == target
            && receipt
                .managed_roots
                .iter()
                .all(|n| Manifest::allowed_root(n)),
        "Backup does not belong to this installation or has unsafe metadata"
    );
    let snapshot = backup.join("snapshot");
    ensure!(
        files::inventory(&snapshot)? == receipt.snapshot,
        "Backup is incomplete or corrupted; installation untouched"
    );
    let old = Manifest::read(&snapshot.join("program"))?;
    old.verify(&snapshot.join("program"))?;
    let original_config =
        Settings::from_values(&target, super::env_file::read(&snapshot.join("program"))?)?;
    let (settings, _instance) = inspect_stopped_config(&target, original_config)?;
    let data = settings.data.canonicalize()?;
    ensure!(
        receipt.data == data && !backup.starts_with(&data),
        "Configured data location changed; refusing to restore to a different directory"
    );
    files::size(&data)?; // Reject symlink targets before deletion.
    let current_program =
        receipt
            .managed_roots
            .iter()
            .try_fold(0_u64, |total, name| -> Result<u64> {
                let path = target.join(name);
                Ok(total.saturating_add(if path.try_exists()? {
                    files::size(&path)?
                } else {
                    0
                }))
            })?;
    // Restoration removes the working copies first; the immutable snapshot stays intact.
    // Requiring a second full database-sized free allocation would make recovery impossible
    // immediately after a successful preflight on a nearly full disk.
    let additional = files::size(&snapshot.join("program"))?
        .saturating_sub(current_program)
        .saturating_add(files::size(&snapshot.join("data"))?.saturating_sub(files::size(&data)?));
    files::require_space(target.parent().unwrap(), additional)?;
    files::require_space(data.parent().context("Invalid data location")?, additional)?;
    set_state(&target, &backup, "restoring", None)?;
    // The original snapshot is immutable. A second invocation can resume a partial restore.
    for name in &receipt.managed_roots {
        files::remove(&target.join(name))?;
    }
    copy_program(&old, &snapshot.join("program"), &target)?;
    files::remove(&target.join(".env"))?;
    files::copy(&snapshot.join("program/.env"), &target.join(".env"), false)?;
    for entry in fs::read_dir(&data)? {
        let entry = entry?;
        if entry.file_name() != "instance.lock" {
            files::remove(&entry.path())?;
        }
    }
    files::copy(&snapshot.join("data"), &data, true)?;
    old.verify(&target)?;
    ensure!(
        files::hash(&target.join(".env"))? == receipt.snapshot["program/.env"],
        "Restored configuration checksum mismatch"
    );
    let mut restored_data = files::inventory(&data)?;
    restored_data.remove("instance.lock");
    let expected_data: BTreeMap<_, _> = receipt
        .snapshot
        .iter()
        .filter_map(|(name, hash)| {
            name.strip_prefix("data/")
                .map(|name| (name.to_owned(), hash.clone()))
        })
        .collect();
    ensure!(
        restored_data == expected_data,
        "Restored data checksum mismatch"
    );
    fs::remove_file(target.join(STATE))?;
    files::sync_directory(&target)?;
    files::json(
        &backup.join("restored.json"),
        &serde_json::json!({"version": receipt.old_version}),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
