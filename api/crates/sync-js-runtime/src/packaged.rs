use crate::{ErrorKind, ExecutionError};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

/// Compiled, release-paired descriptor, NOT caller-supplied request input.
/// `path` is a logical slash-separated selector inside the executable version root.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagedScript {
    pub version: String,
    pub path: String,
    pub sha256: String,
    /// Exact byte length compiled with this release, not a capacity guess.
    pub bytes: u64,
}

/// Load only from current_exe()/../.. (the installed version's bin parent).
/// The version directory must be immutable for the duration of loading/execution.
/// Caller anchors BOTH arguments in its compiled release descriptor, never guest input.
pub fn load_packaged_script(
    expected_version: &str,
    descriptor: &PackagedScript,
) -> Result<String, ExecutionError> {
    let exe = std::env::current_exe().map_err(|_| ErrorKind::ArtifactMissing)?;
    load_from_executable(&exe, expected_version, descriptor)
}

fn load_from_executable(
    exe: &Path,
    expected_version: &str,
    descriptor: &PackagedScript,
) -> Result<String, ExecutionError> {
    let bin = exe
        .parent()
        .filter(|p| p.file_name().is_some_and(|s| s == "bin"))
        .ok_or(ErrorKind::ArtifactDescriptorInvalid)?;
    let root = bin.parent().ok_or(ErrorKind::ArtifactDescriptorInvalid)?;
    load_from_version_root(root, expected_version, descriptor)
}

fn load_from_version_root(
    root: &Path,
    expected_version: &str,
    descriptor: &PackagedScript,
) -> Result<String, ExecutionError> {
    if !valid_component(expected_version) || !valid_component(&descriptor.version) {
        return Err(ErrorKind::ArtifactDescriptorInvalid.into());
    }
    if descriptor.version != expected_version
        || root.file_name().is_none_or(|s| s != expected_version)
    {
        return Err(ErrorKind::ArtifactVersionMismatch.into());
    }
    if descriptor.path.is_empty() || !descriptor.path.split('/').all(valid_component) {
        return Err(ErrorKind::ArtifactPathEscape.into());
    }
    if descriptor.sha256.len() != 64
        || !descriptor
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ErrorKind::ArtifactDescriptorInvalid.into());
    }
    let read_limit = descriptor
        .bytes
        .checked_add(1)
        .ok_or(ErrorKind::ArtifactDescriptorInvalid)?;
    usize::try_from(read_limit).map_err(|_| ErrorKind::ArtifactDescriptorInvalid)?;
    let root = root
        .canonicalize()
        .map_err(|_| ErrorKind::ArtifactMissing)?;
    // Reject a version-root alias pointing at another installed version as well.
    if root.file_name().is_none_or(|s| s != expected_version) {
        return Err(ErrorKind::ArtifactVersionMismatch.into());
    }
    let selected = root
        .join(&descriptor.path)
        .canonicalize()
        .map_err(|_| ErrorKind::ArtifactMissing)?;
    if !selected.starts_with(&root) {
        return Err(ErrorKind::ArtifactPathEscape.into());
    }
    let metadata = selected
        .metadata()
        .map_err(|_| ErrorKind::ArtifactMissing)?;
    if !metadata.is_file() {
        return Err(ErrorKind::ArtifactInvalid.into());
    }
    if metadata.len() != descriptor.bytes {
        return Err(ErrorKind::ArtifactSizeMismatch.into());
    }
    // Metadata check before open also avoids blocking on an installed FIFO. The
    // immutable-version-directory precondition excludes concurrent replacement races.
    let file = File::open(&selected).map_err(|_| ErrorKind::ArtifactMissing)?;
    let metadata = file.metadata().map_err(|_| ErrorKind::ArtifactInvalid)?;
    if !metadata.is_file() {
        return Err(ErrorKind::ArtifactInvalid.into());
    }
    if metadata.len() != descriptor.bytes {
        return Err(ErrorKind::ArtifactSizeMismatch.into());
    }
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorKind::ArtifactInvalid)?;
    if bytes.len() as u64 != descriptor.bytes {
        return Err(ErrorKind::ArtifactSizeMismatch.into());
    }
    let actual = format!("{:x}", base16ct::HexDisplay(&Sha256::digest(&bytes)));
    if actual != descriptor.sha256 {
        return Err(ErrorKind::ArtifactIntegrityMismatch.into());
    }
    // Execute exactly the bytes hashed above; never reopen after verification.
    String::from_utf8(bytes).map_err(|_| ErrorKind::ArtifactInvalid.into())
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
#[path = "_tests/packaged.rs"]
mod tests;
