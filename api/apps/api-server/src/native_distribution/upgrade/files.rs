use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn size(path: &Path) -> Result<u64> {
    let info =
        fs::symlink_metadata(path).with_context(|| format!("Cannot inspect {}", path.display()))?;
    ensure!(
        !info.file_type().is_symlink(),
        "Symlinks are not supported in update files: {}",
        path.display()
    );
    if info.is_file() {
        return Ok(info.len());
    }
    ensure!(
        info.is_dir(),
        "Unsupported filesystem entry: {}",
        path.display()
    );
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes
            .checked_add(size(&entry?.path())?)
            .context("Snapshot size overflow")?;
    }
    Ok(bytes)
}

/// Flush staged file data to stable OS buffers; phase boundaries issue the full hardware barrier.
fn sync_staged_file(file: &fs::File) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;

        let fd = file.as_raw_fd();
        loop {
            // SAFETY: `file` keeps `fd` open and valid for the duration of `libc::fsync`.
            if unsafe { libc::fsync(fd) } == 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        file.sync_data()?;
        Ok(())
    }
}

fn copy_file_staged(source: &Path, destination: &Path) -> Result<()> {
    let info = fs::symlink_metadata(source)?;
    ensure!(
        !info.file_type().is_symlink(),
        "Refusing symlink: {}",
        source.display()
    );
    ensure!(info.is_file(), "Unsupported file: {}", source.display());
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    sync_staged_file(&output)?;
    drop(output);
    fs::set_permissions(destination, info.permissions())?;
    Ok(())
}

/// Copy a file or directory tree without symlinks and record created directories for phase-end sync.
pub fn copy_staged(
    source: &Path,
    destination: &Path,
    dirty_dirs: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let info = fs::symlink_metadata(source)?;
    ensure!(
        !info.file_type().is_symlink(),
        "Refusing symlink: {}",
        source.display()
    );
    if info.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_staged(
                &entry.path(),
                &destination.join(entry.file_name()),
                dirty_dirs,
            )?;
        }
        fs::set_permissions(destination, info.permissions())?;
        dirty_dirs.insert(destination.to_path_buf());
    } else {
        copy_file_staged(source, destination)?;
    }
    Ok(())
}

/// Record `dir` and its ancestors up to `root`, stopping once an ancestor is already tracked.
pub fn record_ancestors_within(root: &Path, dir: &Path, dirty_dirs: &mut BTreeSet<PathBuf>) {
    let mut current = dir;
    while current.starts_with(root) {
        if !dirty_dirs.insert(current.to_path_buf()) || current == root {
            break;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }
}

/// Return `true` only when `destination` is a regular non-symlink file matching `source`'s
/// byte length, permissions, and verified SHA-256 content.
pub fn matches_managed_file(
    source: &Path,
    destination: &Path,
    expected_hash: &str,
) -> Result<bool> {
    let Ok(dest_meta) = fs::symlink_metadata(destination) else {
        return Ok(false);
    };
    if dest_meta.file_type().is_symlink() || !dest_meta.is_file() {
        return Ok(false);
    }
    let source_meta = fs::symlink_metadata(source)?;
    ensure!(
        !source_meta.file_type().is_symlink() && source_meta.is_file(),
        "Invalid staged package file: {}",
        source.display()
    );
    if dest_meta.len() != source_meta.len() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if (dest_meta.permissions().mode() & 0o7777) != (source_meta.permissions().mode() & 0o7777)
        {
            return Ok(false);
        }
    }
    #[cfg(not(unix))]
    if dest_meta.permissions().readonly() != source_meta.permissions().readonly() {
        return Ok(false);
    }
    Ok(hash(destination)? == expected_hash)
}

pub fn remove(path: &Path) -> Result<()> {
    if !path.try_exists()? {
        return Ok(());
    }
    let info = fs::symlink_metadata(path)?;
    ensure!(
        !info.file_type().is_symlink(),
        "Refusing to remove symlink: {}",
        path.display()
    );
    #[cfg(windows)]
    if info.permissions().readonly() {
        let mut permissions = info.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    if info.is_dir() {
        for entry in fs::read_dir(path)? {
            remove(&entry?.path())?;
        }
        fs::remove_dir(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = super::super::settings::private_file(&temp)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        sync_directory(path.parent().context("Invalid metadata path")?)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn require_space(path: &Path, bytes: u64) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Cannot resolve update volume")?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk = disks
        .list()
        .iter()
        .filter_map(|disk| {
            disk.mount_point()
                .canonicalize()
                .ok()
                .filter(|mount| path.starts_with(mount))
                .map(|mount| (disk, mount))
        })
        .max_by_key(|(_, mount)| mount.components().count())
        .map(|(disk, _)| disk)
        .context("Cannot determine free disk space for upgrade")?;
    ensure!(
        disk.available_space() >= bytes.saturating_add(16 * 1024 * 1024),
        "Insufficient space for the new program files"
    );
    Ok(())
}

#[cfg(windows)]
pub fn check_unlocked(path: &Path) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let info = fs::symlink_metadata(path)?;
    ensure!(!info.file_type().is_symlink(), "Symlink in managed files");
    let mut options = fs::OpenOptions::new();
    options.read(true).share_mode(0);
    if info.is_dir() {
        options.custom_flags(0x02000000);
    } else if !info.permissions().readonly() {
        options.write(true);
    }
    drop(
        options.open(path).with_context(|| {
            format!("Close processes using {} before upgrading", path.display())
        })?,
    );
    if info.is_dir() {
        for entry in fs::read_dir(path)? {
            check_unlocked(&entry?.path())?;
        }
    }
    Ok(())
}

pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Synchronize modified directories once each and issue a full hardware barrier on `barrier_root`.
pub fn sync_directories(dirty_dirs: &BTreeSet<PathBuf>, barrier_root: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        for dir in dirty_dirs {
            if dir != barrier_root {
                let handle = fs::File::open(dir)?;
                sync_staged_file(&handle)?;
            }
        }
        sync_directory(barrier_root)?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for dir in dirty_dirs {
            sync_directory(dir)?;
        }
        if !dirty_dirs.contains(barrier_root) {
            sync_directory(barrier_root)?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (dirty_dirs, barrier_root);
        Ok(())
    }
}

/// Replace a program file atomically; callers synchronize `dirty_dirs` once at phase end.
pub fn replace_staged(
    source: &Path,
    destination: &Path,
    root: &Path,
    dirty_dirs: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let parent = destination.parent().context("Invalid program path")?;
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(destination)?;
    remove(&temporary)?;
    let result = (|| -> Result<()> {
        copy_file_staged(source, &temporary)?;
        #[cfg(windows)]
        if destination.try_exists()? {
            let mut permissions = fs::metadata(destination)?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(destination, permissions)?;
            }
        }
        fs::rename(&temporary, destination)?;
        record_ancestors_within(root, parent, dirty_dirs);
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn temporary_path(destination: &Path) -> Result<std::path::PathBuf> {
    Ok(destination
        .parent()
        .context("Invalid program path")?
        .join(format!(
            ".update-{}.tmp",
            destination
                .file_name()
                .context("Invalid file name")?
                .to_string_lossy()
        )))
}
