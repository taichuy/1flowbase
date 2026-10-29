use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_EXPANDED: u64 = 4 * 1024 * 1024 * 1024;
fn destination(root: &Path, name: &Path, package: &str) -> Result<PathBuf> {
    let text = name.to_str().context("Non-UTF8 archive path")?;
    ensure!(
        !text.contains(['\\', ':']) && name.components().all(|c| matches!(c, Component::Normal(_))),
        "Unsafe archive path: {text}"
    );
    let relative = name
        .strip_prefix(package)
        .context("Unexpected archive root")?;
    if let Some(first) = relative.components().next() {
        let first = first.as_os_str().to_str().context("Invalid archive path")?;
        ensure!(
            first == ".env" || super::manifest::Manifest::allowed_root(first),
            "Unmanaged archive entry: {first}"
        );
    }
    Ok(root.join(name))
}
fn write_file(input: &mut impl Read, path: &Path, bytes: u64, total: &mut u64) -> Result<()> {
    *total = total.checked_add(bytes).context("Archive size overflow")?;
    ensure!(*total <= MAX_EXPANDED, "Expanded archive is too large");
    fs::create_dir_all(path.parent().context("Invalid archive path")?)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let copied = std::io::copy(&mut input.take(bytes + 1), &mut output)?;
    ensure!(copied == bytes, "Archive entry size mismatch");
    output.flush()?;
    Ok(())
}
pub fn extract(archive: &Path, root: &Path, package: &str, windows: bool) -> Result<PathBuf> {
    fs::create_dir(root)?;
    let mut total = 0;
    if windows {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        ensure!(zip.len() <= 100_000, "Too many archive entries");
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            // Windows .NET ZipFile emits backslashes; normalize before validating components.
            let name = file.name()?.replace('\\', "/");
            let destination = destination(root, Path::new(&name), package)?;
            let kind = file.unix_mode().unwrap_or(0) & 0o170000;
            ensure!(
                matches!(kind, 0 | 0o100000 | 0o040000),
                "Archive links/special files are not supported"
            );
            if name.ends_with('/') {
                fs::create_dir_all(destination)?;
            } else {
                let size = file.size();
                write_file(&mut file, &destination, size, &mut total)?;
            }
        }
    } else {
        let input = flate2::read::GzDecoder::new(fs::File::open(archive)?);
        let mut tar = tar::Archive::new(input);
        for (i, file) in tar.entries()?.enumerate() {
            ensure!(i < 100_000, "Too many archive entries");
            let mut file = file?;
            let destination = destination(root, &file.path()?, package)?;
            let kind = file.header().entry_type();
            ensure!(
                kind.is_dir() || kind.is_file(),
                "Archive links/special files are not supported"
            );
            if kind.is_dir() {
                fs::create_dir_all(destination)?;
            } else {
                let mode = file.header().mode()?;
                let size = file.size();
                write_file(&mut file, &destination, size, &mut total)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(destination, fs::Permissions::from_mode(mode & 0o777))?;
                }
                #[cfg(not(unix))]
                let _ = mode;
            }
        }
    }
    let package = root.join(package);
    super::manifest::Manifest::read(&package)?.verify(&package)?;
    Ok(package)
}
