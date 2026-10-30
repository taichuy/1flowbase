//! User-triggered updates replace program files in place. User configuration and data are never copied.
mod archive;
mod files;
mod manifest;
mod online;

use super::settings::{private_dir, Settings};
use anyhow::{bail, ensure, Context, Result};
use manifest::Manifest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const STATE: &str = ".upgrade-state.json";

#[derive(Serialize, Deserialize)]
struct State {
    previous: Manifest,
    managed_files: BTreeSet<String>,
    package: String,
    manifest_hash: String,
    build_id: String,
    phase: String,
    error: Option<String>,
}

impl State {
    fn validate(&self) -> Result<()> {
        self.previous.validate()?;
        let mut inventory = self.previous.clone();
        for name in &self.managed_files {
            inventory.files.insert(name.clone(), "0".repeat(64));
        }
        inventory.validate()
    }
}

pub fn handle_cli(
    arguments: &[String],
    validate_config: impl Fn(&super::env_file::Values) -> Result<()>,
    client: impl FnOnce() -> Result<reqwest::Client>,
) -> Result<bool> {
    let Some(mode) = arguments.first() else {
        return Ok(false);
    };
    match mode.as_str() {
        "--check-startup" => { ensure!(arguments.len() == 1, "Unexpected startup-check arguments"); return Ok(false); }
        "--upgrade" => {
            ensure!(arguments.len() == 2, "Run upgrade.cmd / upgrade.sh in your installation directory");
            let target = Path::new(&arguments[1]).canonicalize()?;
            let _maintenance = lock(&target.join(".upgrade.lock"))?;
            let (settings, stopped) = inspect_stopped(&target)?;
            validate_config(&settings.values)?;
            let work = workspace(&target)?;
            if target.join(STATE).exists() {
                let state: State = serde_json::from_slice(&fs::read(target.join(STATE))?)?;
                state.validate()?;
                let source = pending_package(&work, &state)?;
                let pending = Manifest::read(&source)?;
                let incoming = work.join(uuid::Uuid::now_v7().to_string());
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                let next = runtime.block_on(async { online::download(&client()?, &pending, &incoming, online::RELEASES).await });
                match next {
                    Ok(Some(candidate)) => {
                        let new = Manifest::read(&candidate)?;
                        preflight(&target, &candidate, &state.previous, &new, &settings)?;
                        install(&target, &work, &candidate, state.previous, new, stopped, validate_startup)?;
                    }
                    result => {
                        if let Err(error) = result { eprintln!("Cannot check newer releases: {error}. Retrying the existing verified package."); }
                        resume(&target, &work, stopped, validate_startup)?;
                    }
                }
            } else {
                let old = Manifest::read(&target)?;
                old.verify(&target)?;
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                let candidate = runtime.block_on(async {
                    online::download(&client()?, &old, &work, online::RELEASES).await
                })?;
                let Some(candidate) = candidate else { println!("Already up to date / 当前已是最新版本"); return Ok(true); };
                let new = Manifest::read(&candidate)?;
                preflight(&target, &candidate, &old, &new, &settings)?;
                install(&target, &work, &candidate, old, new, stopped, validate_startup)?;
            }
            println!("Update complete. Configuration and data preserved; no backup created.\n更新完成，配置和数据已保留，未创建备份。请正常启动 1flowbase。");
        }
        // Explicit package application is also used by the cross-platform real-package tests.
        "--apply-update" => {
            ensure!(arguments.len() == 3, "Usage: --apply-update <new-package> <installation>");
            apply_from(Path::new(&arguments[1]), Path::new(&arguments[2]), validate_config, validate_startup)?;
        }
        _ => bail!("Unknown argument. Launch normally, or run upgrade.cmd / upgrade.sh to update this installation."),
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
    file.try_lock()
        .with_context(|| format!("Instance or update still running: {}", path.display()))?;
    Ok(file)
}

struct StoppedGuard {
    _instance: File,
    _http: TcpListener,
    _postgres: TcpListener,
}
fn inspect_stopped(target: &Path) -> Result<(Settings, StoppedGuard)> {
    let settings = Settings::inspect(target)?;
    private_dir(&settings.data)?;
    let data = settings.data.canonicalize()?;
    ensure!(
        !target.starts_with(&data) && (data == target.join("data") || !data.starts_with(target)),
        "Data must be data/ or outside the program directory"
    );
    let work = workspace(target)?;
    ensure!(
        !data.starts_with(&work) && !work.starts_with(&data),
        "Update workspace overlaps data"
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
        ensure!(
            !path
                .components()
                .any(|c| c == std::path::Component::ParentDir)
                && (!path.starts_with(target) || path.starts_with(&settings.data))
                && !path.starts_with(&work),
            "{key} must not overlap managed program files or the update workspace"
        );
    }
    let instance = lock(&data.join("instance.lock"))?;
    ensure!(
        !data.join("postgres/postmaster.pid").exists(),
        "Stop 1flowbase and its PostgreSQL normally before updating"
    );
    let http = TcpListener::bind(settings.address)
        .context("Stop 1flowbase before updating: HTTP port is occupied")?;
    let pg = TcpListener::bind(("127.0.0.1", settings.postgres_port))
        .context("Stop PostgreSQL before updating: its port is occupied")?;
    if data.join("postgres/PG_VERSION").exists() {
        ensure!(
            fs::read_to_string(data.join("postgres/PG_VERSION"))?.trim() == "18",
            "PostgreSQL major version migration is not supported"
        );
    }
    Ok((
        settings,
        StoppedGuard {
            _instance: instance,
            _http: http,
            _postgres: pg,
        },
    ))
}

fn workspace(target: &Path) -> Result<PathBuf> {
    Ok(target
        .parent()
        .context("Invalid installation directory")?
        .join(format!(
            ".{}-update",
            target
                .file_name()
                .context("Invalid installation directory")?
                .to_string_lossy()
        )))
}
const WORKSPACE_MARKER: &str = "1flowbase-native-update-v1\n";
fn check_workspace(work: &Path) -> Result<()> {
    let info = fs::symlink_metadata(work)?;
    ensure!(
        info.is_dir() && !info.file_type().is_symlink(),
        "Update workspace must be a real directory"
    );
    ensure!(
        fs::read_to_string(work.join(".update-owner"))
            .ok()
            .as_deref()
            == Some(WORKSPACE_MARKER),
        "Existing directory is not owned by the updater: {}",
        work.display()
    );
    Ok(())
}
fn fresh_workspace(work: &Path) -> Result<()> {
    if fs::symlink_metadata(work).is_ok() {
        check_workspace(work)?;
        files::remove(work)?;
    }
    private_dir(work)?;
    fs::write(work.join(".update-owner"), WORKSPACE_MARKER)?;
    Ok(())
}

fn copy_program(manifest: &Manifest, source: &Path, destination: &Path) -> Result<()> {
    private_dir(destination)?;
    for name in manifest.roots() {
        files::copy(&source.join(&name), &destination.join(&name), false)?;
    }
    Ok(())
}
fn program_path(target: &Path, name: &str) -> Result<PathBuf> {
    let file = target.join(name);
    let mut path = file.as_path();
    while path != target {
        if let Ok(info) = fs::symlink_metadata(path) {
            ensure!(
                !info.file_type().is_symlink(),
                "Symlink in installation: {}",
                path.display()
            );
        }
        path = path.parent().context("Invalid program path")?;
    }
    Ok(file)
}
fn preflight(
    target: &Path,
    source: &Path,
    old: &Manifest,
    new: &Manifest,
    settings: &Settings,
) -> Result<()> {
    new.compatible_with(old)?;
    new.verify(source)?;
    ensure!(
        !source.starts_with(&settings.data) && !settings.data.starts_with(source),
        "New package overlaps user data"
    );
    for name in old.files.keys().chain(new.files.keys()) {
        program_path(target, name)?;
    }
    #[cfg(windows)]
    for name in old.roots() {
        if target.join(&name).exists() {
            files::check_unlocked(&target.join(name))?;
        }
    }
    files::require_space(target, files::size(source)?)?;
    Ok(())
}
fn apply_from(
    source: &Path,
    target: &Path,
    validate_config: impl Fn(&super::env_file::Values) -> Result<()>,
    validate: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let source = source.canonicalize()?;
    let target = target.canonicalize()?;
    ensure!(
        !source.starts_with(&target) && !target.starts_with(&source),
        "New package and installation must not overlap"
    );
    let _maintenance = lock(&target.join(".upgrade.lock"))?;
    let (settings, stopped) = inspect_stopped(&target)?;
    validate_config(&settings.values)?;
    let work = workspace(&target)?;
    if target.join(STATE).exists() {
        let state: State = serde_json::from_slice(&fs::read(target.join(STATE))?)?;
        state.validate()?;
        let new = Manifest::read(&source)?;
        if new.build_id == state.build_id {
            return resume(&target, &work, stopped, validate);
        }
        let pending = pending_package(&work, &state)?;
        ensure!(
            semver::Version::parse(&new.version)?
                >= semver::Version::parse(&Manifest::read(&pending)?.version)?,
            "Cannot downgrade a partially updated installation"
        );
        preflight(&target, &source, &state.previous, &new, &settings)?;
        let staged = work.join(uuid::Uuid::now_v7().to_string()).join("package");
        copy_program(&new, &source, &staged)?;
        return install(
            &target,
            &work,
            &staged,
            state.previous,
            new,
            stopped,
            validate,
        );
    }
    let old = Manifest::read(&target)?;
    let new = Manifest::read(&source)?;
    old.verify(&target)?;
    preflight(&target, &source, &old, &new, &settings)?;
    ensure!(
        !source.starts_with(&work) && !work.starts_with(&source),
        "New package overlaps reserved update workspace"
    );
    files::require_space(
        target.parent().context("Invalid installation location")?,
        files::size(&source)?.saturating_mul(2),
    )?;
    fresh_workspace(&work)?;
    copy_program(&new, &source, &work.join("package"))?;
    new.verify(&work.join("package"))?;
    install(
        &target,
        &work,
        &work.join("package"),
        old,
        new,
        stopped,
        validate,
    )
}
fn pending_package(work: &Path, state: &State) -> Result<PathBuf> {
    check_workspace(work)?;
    let components: Vec<_> = state.package.split('/').collect();
    ensure!(
        components == ["package"]
            || (components.len() == 2
                && uuid::Uuid::parse_str(components[0]).is_ok()
                && components[1] == "package"),
        "Invalid pending package location"
    );
    let source = program_path(work, &state.package)?;
    ensure!(
        files::hash(&source.join(manifest::NAME))? == state.manifest_hash,
        "Pending package manifest changed"
    );
    Ok(source)
}
fn resume(
    target: &Path,
    work: &Path,
    stopped: StoppedGuard,
    validate: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let state: State = serde_json::from_slice(&fs::read(target.join(STATE))?)?;
    state.validate()?;
    ensure!(
        !fs::symlink_metadata(work)?.file_type().is_symlink(),
        "Symlink in update workspace"
    );
    let source = pending_package(work, &state)?;
    let new = Manifest::read(&source)?;
    ensure!(
        state.build_id == new.build_id,
        "Pending package changed; cannot resume update"
    );
    let settings = Settings::inspect(target)?;
    preflight(target, &source, &state.previous, &new, &settings)?;
    println!("Continuing the interrupted update / 继续上次未完成的更新");
    install(
        target,
        work,
        &source,
        state.previous,
        new,
        stopped,
        validate,
    )
}
fn install(
    target: &Path,
    work: &Path,
    source: &Path,
    old: Manifest,
    new: Manifest,
    stopped: StoppedGuard,
    validate: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    new.verify(source)?;
    let mut managed_files: BTreeSet<_> =
        old.files.keys().chain(new.files.keys()).cloned().collect();
    if target.join(STATE).exists() {
        let prior: State = serde_json::from_slice(&fs::read(target.join(STATE))?)?;
        prior.validate()?;
        managed_files.extend(prior.managed_files);
    }
    let mut state = State {
        managed_files,
        package: source
            .strip_prefix(work)?
            .to_str()
            .context("Non-UTF8 package path")?
            .replace('\\', "/"),
        manifest_hash: files::hash(&source.join(manifest::NAME))?,
        previous: old,
        build_id: new.build_id.clone(),
        phase: "replacing".into(),
        error: None,
    };
    files::json(&target.join(STATE), &state)?;
    let outcome = (|| -> Result<()> {
        for name in new.files.keys() {
            let destination = program_path(target, name)?;
            files::replace(&source.join(name), &destination)?;
        }
        for name in state
            .managed_files
            .iter()
            .filter(|name| !new.files.contains_key(*name))
        {
            let file = program_path(target, name)?;
            if file.exists() {
                fs::remove_file(&file)?;
            }
            let temporary = files::temporary_path(&file)?;
            if temporary.exists() {
                files::remove(&temporary)?;
            }
        }
        files::replace(&source.join(manifest::NAME), &target.join(manifest::NAME))?;
        new.verify(target)?;
        state.phase = "validating".into();
        files::json(&target.join(STATE), &state)?;
        drop(stopped);
        validate(target, work)?;
        let (_, _stopped) = inspect_stopped(target)?;
        fs::remove_file(target.join(STATE))?;
        files::sync_directory(target)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        state.phase = "failed".into();
        state.error = Some(format!("{error:#}"));
        let _ = files::json(&target.join(STATE), &state);
        bail!("Update incomplete: {error:#}\nNo backup or rollback was created. Keep {} and run upgrade.cmd / upgrade.sh again to continue. Startup log: {}", work.display(), work.join("startup-validation.log").display());
    }
    // Cleanup cannot turn a verified installation into a failed update.
    if let Err(error) = files::remove(work) {
        eprintln!("Update succeeded; temporary new-package cleanup failed: {error}");
    }
    Ok(())
}

fn validate_startup(target: &Path, work: &Path) -> Result<()> {
    let log = File::create(work.join("startup-validation.log"))?;
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
            let config = Settings::inspect(target)?;
            let _ = Command::new(target.join(format!(
                "postgres/bin/pg_ctl{}",
                std::env::consts::EXE_SUFFIX
            )))
            .args(["stop", "-w", "-t", "60", "-m", "fast", "-D"])
            .arg(config.data.join("postgres"))
            .status();
            bail!("Startup validation timed out; inspect the log and confirm PostgreSQL stopped before retrying");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
