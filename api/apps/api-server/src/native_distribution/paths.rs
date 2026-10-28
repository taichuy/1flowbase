use std::{
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

pub(super) fn data_root(package_root: &Path) -> io::Result<PathBuf> {
    resolve_data_root(
        package_root,
        env::var_os("FLOWBASE_DATA_DIR").as_deref(),
        legacy_data_root().as_deref(),
    )
}

fn legacy_data_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    root.filter(|path| path.is_absolute())
        .map(|path| path.join("1flowbase"))
}

fn has_state(directory: &Path) -> io::Result<bool> {
    Ok(directory.join("settings.json").try_exists()? || directory.join("postgres").try_exists()?)
}

fn resolve_data_root(
    package_root: &Path,
    explicit: Option<&OsStr>,
    legacy: Option<&Path>,
) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FLOWBASE_DATA_DIR is empty",
            ));
        }
        return std::path::absolute(path);
    }
    let data = package_root.join("data");
    if !has_state(&data)? {
        if let Some(old) = legacy {
            if has_state(old)? {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!(
                    "Existing 1flowbase data found at {}. Stop the old instance and copy the entire directory (including settings.json) to {}, or explicitly set FLOWBASE_DATA_DIR to the old location. No data was moved or replaced.",
                    old.display(), data.display()
                )));
            }
        }
    }
    Ok(data)
}

#[cfg(test)]
#[path = "_tests/paths.rs"]
mod tests;
