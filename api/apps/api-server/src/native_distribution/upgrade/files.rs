use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
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

/// Caller exclusively owns destination. No following links or copying a live instance lock.
pub fn copy(source: &Path, destination: &Path, data_root: bool) -> Result<()> {
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
            if data_root && entry.file_name() == "instance.lock" {
                continue;
            }
            copy(&entry.path(), &destination.join(entry.file_name()), false)?;
        }
        fs::set_permissions(destination, info.permissions())?;
        sync_directory(destination)?;
    } else {
        ensure!(info.is_file(), "Unsupported file: {}", source.display());
        let mut input = fs::File::open(source)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        drop(output);
        fs::set_permissions(destination, info.permissions())?;
    }
    Ok(())
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

/// Replace a program file atomically; this is a new-file staging copy, never an old-file backup.
pub fn replace(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().context("Invalid program path")?;
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(destination)?;
    remove(&temporary)?;
    let result = (|| -> Result<()> {
        copy(source, &temporary, false)?;
        #[cfg(windows)]
        if destination.try_exists()? {
            let mut permissions = fs::metadata(destination)?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(destination, permissions)?;
            }
        }
        fs::rename(&temporary, destination)?;
        sync_directory(parent)?;
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
