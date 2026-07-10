use super::*;

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
