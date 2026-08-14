use std::path::{Component, Path, PathBuf};

use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};
use uuid::Uuid;

use crate::{
    driver::FileStorageDriver,
    errors::{FileStorageError, FileStorageResult},
    types::{
        DeleteObjectInput, FileStorageHealthcheck, FileStorageObjectSnapshot, FileStoragePutInput,
        FileStoragePutResult, FileStoragePutStreamInput, GenerateAccessUrlInput, OpenReadInput,
        OpenReadResult, OpenReadStreamResult, VerifyReadUnchangedInput,
        FILE_STORAGE_STREAM_BUFFER_BYTES,
    },
};

#[derive(Debug, Default)]
pub struct LocalFileStorageDriver;

fn root_path(config_json: &serde_json::Value) -> FileStorageResult<PathBuf> {
    config_json
        .get("root_path")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(FileStorageError::InvalidConfig("root_path"))
}

fn resolve_object_path(root: &Path, object_path: &str) -> FileStorageResult<PathBuf> {
    let relative = Path::new(object_path);

    if object_path.trim().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FileStorageError::InvalidConfig("object_path"));
    }

    Ok(root.join(relative))
}

fn metadata_path(path: &Path) -> FileStorageResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(FileStorageError::InvalidConfig("object_path"))?;

    // The Task 2 contract expects content_type to survive a local read round-trip.
    Ok(path.with_file_name(format!("{file_name}.metadata.json")))
}

fn other_error(error: impl Into<AnyhowError>) -> FileStorageError {
    FileStorageError::Other(error.into())
}

fn temporary_sibling(path: &Path, purpose: &str) -> FileStorageResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(FileStorageError::InvalidConfig("object_path"))?;
    Ok(path.with_file_name(format!(".{file_name}.{purpose}.{}.tmp", Uuid::now_v7())))
}

#[cfg(unix)]
fn metadata_fingerprint(metadata: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;

    format!(
        "{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec()
    )
}

#[cfg(not(unix))]
fn metadata_fingerprint(metadata: &std::fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |value| value.as_nanos());
    format!("{}:{modified}", metadata.len())
}

async fn metadata_sidecar_fingerprint(path: &Path) -> FileStorageResult<String> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => Ok(metadata_fingerprint(&metadata)),
        Ok(_) => Err(FileStorageError::ObjectChanged),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("missing".to_string()),
        Err(error) => Err(other_error(error)),
    }
}

async fn local_snapshot(
    object_path: &Path,
    metadata_path: &Path,
) -> FileStorageResult<FileStorageObjectSnapshot> {
    let object_metadata = fs::metadata(object_path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => FileStorageError::ObjectChanged,
            _ => other_error(error),
        })?;
    if !object_metadata.is_file() {
        return Err(FileStorageError::ObjectChanged);
    }
    let sidecar_fingerprint = metadata_sidecar_fingerprint(metadata_path).await?;
    Ok(snapshot_from_metadata(
        &object_metadata,
        &sidecar_fingerprint,
    ))
}

fn snapshot_from_metadata(
    object_metadata: &std::fs::Metadata,
    sidecar_fingerprint: &str,
) -> FileStorageObjectSnapshot {
    FileStorageObjectSnapshot {
        content_length: object_metadata.len(),
        validator: format!(
            "local:{}:{sidecar_fingerprint}",
            metadata_fingerprint(object_metadata)
        ),
    }
}

async fn read_content_type(path: &Path) -> FileStorageResult<Option<String>> {
    match fs::read(path).await {
        Ok(metadata_bytes) => serde_json::from_slice::<serde_json::Value>(&metadata_bytes)
            .map_err(other_error)
            .map(|metadata| {
                metadata
                    .get("content_type")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(other_error(error)),
    }
}

async fn write_exact_stream(
    mut reader: crate::FileStorageStreamReader,
    mut writer: fs::File,
    expected_length: u64,
) -> FileStorageResult<()> {
    let mut remaining = expected_length;
    let mut buffer = vec![0_u8; FILE_STORAGE_STREAM_BUFFER_BYTES];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| FileStorageError::ObjectLengthMismatch)?;
        let read = reader
            .read(&mut buffer[..limit])
            .await
            .map_err(other_error)?;
        if read == 0 {
            return Err(FileStorageError::ObjectLengthMismatch);
        }
        writer
            .write_all(&buffer[..read])
            .await
            .map_err(other_error)?;
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if reader.read(&mut extra).await.map_err(other_error)? != 0 {
        return Err(FileStorageError::ObjectLengthMismatch);
    }
    writer.flush().await.map_err(other_error)?;
    writer.sync_all().await.map_err(other_error)
}

#[async_trait]
impl FileStorageDriver for LocalFileStorageDriver {
    fn driver_type(&self) -> &'static str {
        "local"
    }

    fn validate_config(&self, config_json: &serde_json::Value) -> FileStorageResult<()> {
        let _ = root_path(config_json)?;
        Ok(())
    }

    async fn healthcheck(
        &self,
        config_json: &serde_json::Value,
    ) -> FileStorageResult<FileStorageHealthcheck> {
        let root = root_path(config_json)?;
        fs::create_dir_all(&root).await.map_err(other_error)?;
        Ok(FileStorageHealthcheck {
            reachable: true,
            detail: Some(root.display().to_string()),
        })
    }

    async fn put_object(
        &self,
        input: FileStoragePutInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        let root = root_path(input.config_json)?;
        let full_path = resolve_object_path(&root, input.object_path)?;
        let metadata_path = metadata_path(&full_path)?;

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.map_err(other_error)?;
        }

        fs::write(&full_path, input.bytes)
            .await
            .map_err(other_error)?;
        fs::write(
            metadata_path,
            serde_json::to_vec(&serde_json::json!({
                "content_type": input.content_type,
            }))
            .map_err(other_error)?,
        )
        .await
        .map_err(other_error)?;

        let url = input
            .config_json
            .get("public_base_url")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), input.object_path));

        Ok(FileStoragePutResult {
            path: input.object_path.to_string(),
            url,
            metadata_json: serde_json::json!({
                "driver_type": "local",
                "content_type": input.content_type,
            }),
        })
    }

    async fn put_object_stream(
        &self,
        input: FileStoragePutStreamInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        let root = root_path(input.config_json)?;
        let full_path = resolve_object_path(&root, input.object_path)?;
        let sidecar_path = metadata_path(&full_path)?;
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.map_err(other_error)?;
        }

        let object_staging = temporary_sibling(&full_path, "object-stream")?;
        let metadata_staging = temporary_sibling(&sidecar_path, "metadata-stream")?;
        let staged = async {
            let writer = fs::File::create(&object_staging)
                .await
                .map_err(other_error)?;
            write_exact_stream(input.reader, writer, input.content_length).await?;
            fs::write(
                &metadata_staging,
                serde_json::to_vec(&serde_json::json!({
                    "content_type": input.content_type,
                }))
                .map_err(other_error)?,
            )
            .await
            .map_err(other_error)?;
            fs::rename(&object_staging, &full_path)
                .await
                .map_err(other_error)?;
            fs::rename(&metadata_staging, &sidecar_path)
                .await
                .map_err(other_error)
        }
        .await;
        if let Err(error) = staged {
            let _ = fs::remove_file(&object_staging).await;
            let _ = fs::remove_file(&metadata_staging).await;
            return Err(error);
        }

        let url = input
            .config_json
            .get("public_base_url")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), input.object_path));
        Ok(FileStoragePutResult {
            path: input.object_path.to_string(),
            url,
            metadata_json: serde_json::json!({
                "driver_type": "local",
                "content_type": input.content_type,
            }),
        })
    }

    async fn delete_object(&self, input: DeleteObjectInput<'_>) -> FileStorageResult<()> {
        let root = root_path(input.config_json)?;
        let full_path = resolve_object_path(&root, input.object_path)?;
        let metadata_path = metadata_path(&full_path)?;

        match fs::remove_file(&full_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(other_error(error)),
        }

        match fs::remove_file(metadata_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(other_error(error)),
        }

        Ok(())
    }

    async fn open_read(&self, input: OpenReadInput<'_>) -> FileStorageResult<OpenReadResult> {
        let mut opened = self
            .open_read_stream(OpenReadInput {
                config_json: input.config_json,
                object_path: input.object_path,
            })
            .await?;
        let mut bytes = Vec::new();
        opened
            .reader
            .read_to_end(&mut bytes)
            .await
            .map_err(other_error)?;
        self.verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: input.config_json,
            object_path: input.object_path,
            snapshot: &opened.snapshot,
        })
        .await?;
        Ok(OpenReadResult {
            bytes,
            content_type: opened.content_type,
        })
    }

    async fn open_read_stream(
        &self,
        input: OpenReadInput<'_>,
    ) -> FileStorageResult<OpenReadStreamResult> {
        let root = root_path(input.config_json)?;
        let full_path = resolve_object_path(&root, input.object_path)?;
        let sidecar_path = metadata_path(&full_path)?;
        let file = fs::File::open(&full_path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => FileStorageError::ObjectNotFound,
                _ => other_error(error),
            })?;
        let opened_metadata = file.metadata().await.map_err(other_error)?;
        if !opened_metadata.is_file() {
            return Err(FileStorageError::ObjectNotFound);
        }
        let sidecar_before = metadata_sidecar_fingerprint(&sidecar_path).await?;
        let content_type = read_content_type(&sidecar_path).await?;
        let sidecar_after = metadata_sidecar_fingerprint(&sidecar_path).await?;
        if sidecar_before != sidecar_after {
            return Err(FileStorageError::ObjectChanged);
        }
        let snapshot = snapshot_from_metadata(&opened_metadata, &sidecar_after);
        if local_snapshot(&full_path, &sidecar_path).await? != snapshot {
            return Err(FileStorageError::ObjectChanged);
        }
        Ok(OpenReadStreamResult {
            reader: Box::pin(file),
            content_type,
            snapshot,
        })
    }

    async fn verify_read_unchanged(
        &self,
        input: VerifyReadUnchangedInput<'_>,
    ) -> FileStorageResult<()> {
        let root = root_path(input.config_json)?;
        let full_path = resolve_object_path(&root, input.object_path)?;
        let sidecar_path = metadata_path(&full_path)?;
        let current = local_snapshot(&full_path, &sidecar_path).await?;
        if current == *input.snapshot {
            Ok(())
        } else {
            Err(FileStorageError::ObjectChanged)
        }
    }

    async fn generate_access_url(
        &self,
        input: GenerateAccessUrlInput<'_>,
    ) -> FileStorageResult<Option<String>> {
        let _ = resolve_object_path(&root_path(input.config_json)?, input.object_path)?;

        Ok(input
            .config_json
            .get("public_base_url")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), input.object_path)))
    }
}
