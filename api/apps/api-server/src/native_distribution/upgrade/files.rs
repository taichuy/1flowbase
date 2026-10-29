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
        "Symlinks are not supported in upgrade snapshots: {}",
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
        "Insufficient space for program staging and complete data backup"
    );
    Ok(())
}

pub fn inventory(root: &Path) -> Result<std::collections::BTreeMap<String, String>> {
    fn walk(
        root: &Path,
        path: &Path,
        out: &mut std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        if path != root && path.strip_prefix(root)? == Path::new("instance.lock") {
            return Ok(());
        }
        let info = fs::symlink_metadata(path)?;
        ensure!(!info.file_type().is_symlink(), "Symlink in snapshot");
        if info.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(root, &entry?.path(), out)?;
            }
        } else {
            ensure!(info.is_file(), "Unsupported snapshot file");
            out.insert(
                path.strip_prefix(root)?
                    .to_str()
                    .context("Non-UTF8 snapshot path")?
                    .replace('\\', "/"),
                hash(path)?,
            );
        }
        Ok(())
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
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
