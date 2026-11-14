//! Backend-host filesystem execution. Paths use native platform semantics; this is not a sandbox.
pub mod dto;
mod files;
mod search;
pub(crate) use files::{list, media, range, resource, stat, text};
pub(crate) use search::search;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub(crate) fn invalid(message: &'static str) -> FileError {
    FileError::InvalidInput(message)
}
pub(crate) struct Budget {
    deadline: Instant,
}
impl Budget {
    pub(crate) fn new(timeout_ms: u64) -> Result<Self, FileError> {
        if timeout_ms == 0 {
            return Err(invalid("terminal_timeout_invalid"));
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(timeout_ms))
            .ok_or_else(|| invalid("terminal_timeout_invalid"))?;
        Ok(Self { deadline })
    }
    pub(crate) fn check(&self) -> Result<(), FileError> {
        if Instant::now() >= self.deadline {
            return Err(invalid("terminal_timeout"));
        }
        Ok(())
    }
}
pub(crate) fn path(value: &str) -> Result<PathBuf, FileError> {
    let path = Path::new(value);
    if !path.is_absolute() || value.contains('\0') {
        return Err(invalid("terminal_absolute_path_required"));
    }
    Ok(path.to_owned())
}
pub(crate) fn info() -> dto::InfoOutput {
    dto::InfoOutput {
        os: std::env::consts::OS.into(),
        shell: if cfg!(windows) {
            "cmd.exe".into()
        } else {
            "/bin/sh".into()
        },
        execution_location: "backend".into(),
        capabilities: vec!["files".into(), "jobs".into(), "execute".into()],
        pty: false,
    }
}
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/files.rs"]
mod tests;
