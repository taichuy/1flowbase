use super::*;

#[cfg(test)]
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

#[cfg(test)]
static STAGED_ARTIFACT_REMOVAL_FAILURES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn fail_staged_artifact_removal_for(original_path: &Path) {
    STAGED_ARTIFACT_REMOVAL_FAILURES
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .expect("staged artifact removal failure registry must not be poisoned")
        .insert(original_path.to_path_buf());
}

pub(super) fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }

    Ok(())
}

/// A locally staged removal that can be restored until its durable state change commits.
///
/// The artifact remains outside its executable path while the caller records the unavailable
/// state, but a rejected durable commit can still restore the exact original path.
pub(super) struct StagedArtifactRemoval {
    original_path: PathBuf,
    tombstone_path: Option<PathBuf>,
}

impl StagedArtifactRemoval {
    fn stage(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                original_path: path,
                tombstone_path: None,
            });
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin-artifact");
        let tombstone_path =
            path.with_file_name(format!(".{file_name}.uninstalling-{}", Uuid::now_v7()));
        fs::rename(&path, &tombstone_path).with_context(|| {
            format!(
                "failed to stage plugin artifact removal at {}",
                path.display()
            )
        })?;
        Ok(Self {
            original_path: path,
            tombstone_path: Some(tombstone_path),
        })
    }

    fn original_path(&self) -> &Path {
        &self.original_path
    }

    pub(super) fn tombstone_path(&self) -> Option<&Path> {
        self.tombstone_path.as_deref()
    }

    pub(super) fn restore(&mut self) -> Result<()> {
        let Some(tombstone_path) = self.tombstone_path.as_ref() else {
            return Ok(());
        };
        if !tombstone_path.exists() {
            return Ok(());
        }
        if self.original_path.exists() {
            anyhow::bail!(
                "cannot restore staged plugin artifact because the original path is occupied: {}",
                self.original_path.display()
            );
        }
        fs::rename(tombstone_path, &self.original_path).with_context(|| {
            format!(
                "failed to restore plugin artifact to {} after durable commit rejection",
                self.original_path.display()
            )
        })?;
        self.tombstone_path = None;
        Ok(())
    }

    pub(super) fn remove(mut self) -> Result<()> {
        #[cfg(test)]
        if STAGED_ARTIFACT_REMOVAL_FAILURES
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .expect("staged artifact removal failure registry must not be poisoned")
            .remove(&self.original_path)
        {
            anyhow::bail!(
                "injected staged plugin artifact removal failure at {}",
                self.original_path.display()
            );
        }
        if let Some(tombstone_path) = self.tombstone_path.take() {
            remove_path_if_exists(&tombstone_path).with_context(|| {
                format!(
                    "failed to remove staged plugin artifact at {}",
                    tombstone_path.display()
                )
            })?;
        }
        Ok(())
    }
}

pub(super) fn stage_artifact_removals(
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<StagedArtifactRemoval>> {
    let mut candidates = paths.into_iter().collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates.sort_by_key(|path| path.components().count());

    let mut removals = Vec::with_capacity(candidates.len());
    for path in candidates {
        // An artifact package may be recorded both as a root and as a nested package path.
        // Staging the containing root already removes the nested path from runtime discovery.
        if removals
            .iter()
            .any(|removal: &StagedArtifactRemoval| path.starts_with(removal.original_path()))
        {
            continue;
        }
        match StagedArtifactRemoval::stage(path) {
            Ok(removal) => removals.push(removal),
            Err(error) => {
                let restore_error = restore_artifact_removals(&mut removals).err();
                return match restore_error {
                    Some(restore_error) => Err(error.context(format!(
                        "also failed to restore previously staged plugin artifacts: {restore_error}"
                    ))),
                    None => Err(error),
                };
            }
        }
    }
    Ok(removals)
}

pub(super) fn restore_artifact_removals(removals: &mut [StagedArtifactRemoval]) -> Result<()> {
    for removal in removals.iter_mut().rev() {
        removal.restore()?;
    }
    Ok(())
}

pub(super) fn copy_installation_artifact(source_root: &Path, target_root: &Path) -> Result<()> {
    if target_root.exists() {
        fs::remove_dir_all(target_root).with_context(|| {
            format!(
                "failed to remove previous installation artifact at {}",
                target_root.display()
            )
        })?;
    }
    fs::create_dir_all(target_root).with_context(|| {
        format!(
            "failed to create installation artifact root {}",
            target_root.display()
        )
    })?;
    copy_dir(source_root, target_root)
}

pub(super) struct StagedArtifactPath {
    final_path: PathBuf,
    staged_path: PathBuf,
    backup_path: PathBuf,
    activated: bool,
}

impl StagedArtifactPath {
    pub(super) fn prepare_directory(source_root: &Path, final_path: &Path) -> Result<Self> {
        let staged = Self::new(final_path);
        remove_path_if_exists(&staged.staged_path)?;
        copy_installation_artifact(source_root, &staged.staged_path)?;
        Ok(staged)
    }

    pub(super) fn prepare_file(bytes: &[u8], final_path: &Path) -> Result<Self> {
        let staged = Self::new(final_path);
        remove_path_if_exists(&staged.staged_path)?;
        if let Some(parent) = staged.staged_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&staged.staged_path, bytes)?;
        Ok(staged)
    }

    fn new(final_path: &Path) -> Self {
        let nonce = Uuid::now_v7();
        Self {
            final_path: final_path.to_path_buf(),
            staged_path: final_path.with_extension(format!("installing-{nonce}")),
            backup_path: final_path.with_extension(format!("rollback-{nonce}")),
            activated: false,
        }
    }

    pub(super) fn staged_path(&self) -> &Path {
        &self.staged_path
    }

    pub(super) fn activate(&mut self) -> Result<()> {
        if let Some(parent) = self.final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        remove_path_if_exists(&self.backup_path)?;
        if self.final_path.exists() {
            fs::rename(&self.final_path, &self.backup_path)?;
        }
        if let Err(error) = fs::rename(&self.staged_path, &self.final_path) {
            if self.backup_path.exists() {
                let _ = fs::rename(&self.backup_path, &self.final_path);
            }
            return Err(error.into());
        }
        self.activated = true;
        Ok(())
    }

    pub(super) fn rollback(&mut self) -> Result<()> {
        if self.activated {
            remove_path_if_exists(&self.final_path)?;
            if self.backup_path.exists() {
                fs::rename(&self.backup_path, &self.final_path)?;
            }
            self.activated = false;
        } else {
            remove_path_if_exists(&self.staged_path)?;
        }
        Ok(())
    }

    pub(super) fn finish(mut self) {
        self.activated = false;
        let _ = remove_path_if_exists(&self.backup_path);
        let _ = remove_path_if_exists(&self.staged_path);
    }
}

impl Drop for StagedArtifactPath {
    fn drop(&mut self) {
        if self.activated {
            let _ = remove_path_if_exists(&self.final_path);
            if self.backup_path.exists() {
                let _ = fs::rename(&self.backup_path, &self.final_path);
            }
            return;
        }
        let _ = remove_path_if_exists(&self.staged_path);
    }
}

fn copy_dir(source_root: &Path, target_root: &Path) -> Result<()> {
    for entry in fs::read_dir(source_root)
        .with_context(|| format!("failed to read {}", source_root.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target_root.join(entry.file_name());
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if source_path.is_dir() {
            if matches!(name.as_ref(), "demo" | "scripts") {
                continue;
            }
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            copy_dir(&source_path, &target_path)?;
            continue;
        }

        fs::copy(&source_path, &target_path).with_context(|| {
            format!(
                "failed to copy installation artifact {} -> {}",
                source_path.display(),
                target_path.display()
            )
        })?;
    }
    Ok(())
}
