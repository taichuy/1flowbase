use async_trait::async_trait;
use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
use aws_sdk_s3::{
    config::{Builder as S3ConfigBuilder, Credentials, Region},
    operation::get_object::GetObjectError,
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
    Client,
};
use tokio::io::AsyncReadExt;

use crate::{
    driver::FileStorageDriver,
    errors::{FileStorageError, FileStorageResult},
    types::{
        DeleteObjectInput, FileStorageHealthcheck, FileStorageObjectSnapshot, FileStoragePutInput,
        FileStoragePutResult, FileStoragePutStreamInput, GenerateAccessUrlInput, OpenReadInput,
        OpenReadResult, OpenReadStreamResult, VerifyReadUnchangedInput,
    },
};

#[derive(Debug, Default)]
pub struct RustfsFileStorageDriver;

pub const RUSTFS_MULTIPART_CHUNK_BYTES: usize = 8 * 1024 * 1024;
const RUSTFS_MAX_MULTIPART_PARTS: u64 = 10_000;

#[derive(Debug, Clone)]
struct RustfsConfig {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
    force_path_style: bool,
    public_base_url: Option<String>,
}

fn required_string(
    config_json: &serde_json::Value,
    field: &'static str,
) -> FileStorageResult<String> {
    config_json
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(FileStorageError::InvalidConfig(field))
}

fn optional_string(config_json: &serde_json::Value, field: &'static str) -> Option<String> {
    config_json
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_bool(config_json: &serde_json::Value, field: &'static str) -> Option<bool> {
    config_json.get(field).and_then(|value| value.as_bool())
}

fn parse_config(config_json: &serde_json::Value) -> FileStorageResult<RustfsConfig> {
    Ok(RustfsConfig {
        endpoint: required_string(config_json, "endpoint")?,
        bucket: required_string(config_json, "bucket")?,
        access_key: required_string(config_json, "access_key")?,
        secret_key: required_string(config_json, "secret_key")?,
        region: optional_string(config_json, "region").unwrap_or_else(|| "us-east-1".to_string()),
        force_path_style: optional_bool(config_json, "force_path_style")
            .or_else(|| optional_bool(config_json, "path_style"))
            .unwrap_or(true),
        public_base_url: optional_string(config_json, "public_base_url"),
    })
}

fn other_error(error: impl Into<anyhow::Error>) -> FileStorageError {
    FileStorageError::Other(error.into())
}

async fn build_client(config: &RustfsConfig) -> FileStorageResult<Client> {
    let region = Region::new(config.region.clone());
    let shared = aws_config::defaults(BehaviorVersion::latest())
        .region(RegionProviderChain::first_try(region.clone()))
        .credentials_provider(Credentials::new(
            config.access_key.clone(),
            config.secret_key.clone(),
            None,
            None,
            "rustfs-driver",
        ))
        .load()
        .await;

    let s3_config = S3ConfigBuilder::from(&shared)
        .region(region)
        .endpoint_url(config.endpoint.clone())
        .force_path_style(config.force_path_style)
        .build();

    Ok(Client::from_conf(s3_config))
}

fn rustfs_snapshot(
    content_length: Option<i64>,
    e_tag: Option<&str>,
    version_id: Option<&str>,
    content_type: Option<&str>,
) -> FileStorageResult<FileStorageObjectSnapshot> {
    let content_length = content_length
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(FileStorageError::ObjectSnapshotUnavailable)?;
    let e_tag = e_tag.map(str::trim).filter(|value| !value.is_empty());
    let version_id = version_id.map(str::trim).filter(|value| !value.is_empty());
    if e_tag.is_none() && version_id.is_none() {
        return Err(FileStorageError::ObjectSnapshotUnavailable);
    }
    let validator = serde_json::to_string(&serde_json::json!({
        "driver": "rustfs",
        "etag": e_tag,
        "version_id": version_id,
        "content_type": content_type,
    }))
    .map_err(other_error)?;
    Ok(FileStorageObjectSnapshot {
        content_length,
        validator,
    })
}

pub(crate) async fn read_declared_part(
    reader: &mut crate::FileStorageStreamReader,
    length: usize,
) -> FileStorageResult<Vec<u8>> {
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => FileStorageError::ObjectLengthMismatch,
            _ => other_error(error),
        })?;
    Ok(bytes)
}

async fn ensure_stream_finished(
    reader: &mut crate::FileStorageStreamReader,
) -> FileStorageResult<()> {
    let mut extra = [0_u8; 1];
    if reader.read(&mut extra).await.map_err(other_error)? == 0 {
        Ok(())
    } else {
        Err(FileStorageError::ObjectLengthMismatch)
    }
}

fn rustfs_put_result(config: &RustfsConfig, object_path: &str) -> FileStoragePutResult {
    let url = config
        .public_base_url
        .as_ref()
        .map(|base| format!("{}/{}", base.trim_end_matches('/'), object_path));
    FileStoragePutResult {
        path: object_path.to_string(),
        url,
        metadata_json: serde_json::json!({
            "driver_type": "rustfs",
            "bucket": config.bucket,
        }),
    }
}

#[async_trait]
impl FileStorageDriver for RustfsFileStorageDriver {
    fn driver_type(&self) -> &'static str {
        "rustfs"
    }

    fn validate_config(&self, config_json: &serde_json::Value) -> FileStorageResult<()> {
        let _ = parse_config(config_json)?;
        Ok(())
    }

    async fn healthcheck(
        &self,
        config_json: &serde_json::Value,
    ) -> FileStorageResult<FileStorageHealthcheck> {
        let config = parse_config(config_json)?;
        let client = build_client(&config).await?;
        client
            .head_bucket()
            .bucket(&config.bucket)
            .send()
            .await
            .map_err(other_error)?;
        Ok(FileStorageHealthcheck {
            reachable: true,
            detail: Some(config.bucket),
        })
    }

    async fn put_object(
        &self,
        input: FileStoragePutInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        let config = parse_config(input.config_json)?;
        let client = build_client(&config).await?;
        client
            .put_object()
            .bucket(&config.bucket)
            .key(input.object_path)
            .body(input.bytes.to_vec().into())
            .set_content_type(input.content_type.map(str::to_string))
            .send()
            .await
            .map_err(other_error)?;

        Ok(rustfs_put_result(&config, input.object_path))
    }

    async fn put_object_stream(
        &self,
        input: FileStoragePutStreamInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        if input.content_length > (RUSTFS_MULTIPART_CHUNK_BYTES as u64) * RUSTFS_MAX_MULTIPART_PARTS
        {
            return Err(FileStorageError::ObjectTooLarge);
        }
        let config = parse_config(input.config_json)?;
        let client = build_client(&config).await?;
        let mut reader = input.reader;
        if input.content_length == 0 {
            ensure_stream_finished(&mut reader).await?;
            client
                .put_object()
                .bucket(&config.bucket)
                .key(input.object_path)
                .body(ByteStream::from_static(&[]))
                .set_content_type(input.content_type.map(str::to_string))
                .send()
                .await
                .map_err(other_error)?;
            return Ok(rustfs_put_result(&config, input.object_path));
        }

        let created = client
            .create_multipart_upload()
            .bucket(&config.bucket)
            .key(input.object_path)
            .set_content_type(input.content_type.map(str::to_string))
            .send()
            .await
            .map_err(other_error)?;
        let upload_id = created
            .upload_id()
            .ok_or(FileStorageError::ObjectSnapshotUnavailable)?
            .to_string();
        let upload_result = async {
            let mut remaining = input.content_length;
            let mut part_number = 1_i32;
            let mut completed_parts = Vec::new();
            while remaining > 0 {
                let part_length =
                    usize::try_from(remaining.min(RUSTFS_MULTIPART_CHUNK_BYTES as u64))
                        .map_err(|_| FileStorageError::ObjectTooLarge)?;
                let bytes = read_declared_part(&mut reader, part_length).await?;
                let uploaded = client
                    .upload_part()
                    .bucket(&config.bucket)
                    .key(input.object_path)
                    .upload_id(&upload_id)
                    .part_number(part_number)
                    .body(ByteStream::from(bytes))
                    .send()
                    .await
                    .map_err(other_error)?;
                let e_tag = uploaded
                    .e_tag()
                    .ok_or(FileStorageError::ObjectSnapshotUnavailable)?;
                completed_parts.push(
                    CompletedPart::builder()
                        .part_number(part_number)
                        .e_tag(e_tag)
                        .build(),
                );
                remaining -= part_length as u64;
                part_number = part_number
                    .checked_add(1)
                    .ok_or(FileStorageError::ObjectTooLarge)?;
            }
            ensure_stream_finished(&mut reader).await?;
            client
                .complete_multipart_upload()
                .bucket(&config.bucket)
                .key(input.object_path)
                .upload_id(&upload_id)
                .multipart_upload(
                    CompletedMultipartUpload::builder()
                        .set_parts(Some(completed_parts))
                        .build(),
                )
                .send()
                .await
                .map_err(other_error)?;
            Ok::<(), FileStorageError>(())
        }
        .await;
        if let Err(error) = upload_result {
            let _ = client
                .abort_multipart_upload()
                .bucket(&config.bucket)
                .key(input.object_path)
                .upload_id(&upload_id)
                .send()
                .await;
            return Err(error);
        }
        Ok(rustfs_put_result(&config, input.object_path))
    }

    async fn delete_object(&self, input: DeleteObjectInput<'_>) -> FileStorageResult<()> {
        let config = parse_config(input.config_json)?;
        let client = build_client(&config).await?;
        client
            .delete_object()
            .bucket(&config.bucket)
            .key(input.object_path)
            .send()
            .await
            .map_err(other_error)?;
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
        let config = parse_config(input.config_json)?;
        let client = build_client(&config).await?;
        let output = client
            .get_object()
            .bucket(&config.bucket)
            .key(input.object_path)
            .send()
            .await
            .map_err(|error| {
                if error
                    .as_service_error()
                    .is_some_and(GetObjectError::is_no_such_key)
                {
                    FileStorageError::ObjectNotFound
                } else {
                    other_error(error)
                }
            })?;
        let snapshot = rustfs_snapshot(
            output.content_length(),
            output.e_tag(),
            output.version_id(),
            output.content_type(),
        )?;
        let content_type = output.content_type().map(str::to_string);
        Ok(OpenReadStreamResult {
            reader: Box::pin(output.body.into_async_read()),
            content_type,
            snapshot,
        })
    }

    async fn verify_read_unchanged(
        &self,
        input: VerifyReadUnchangedInput<'_>,
    ) -> FileStorageResult<()> {
        let config = parse_config(input.config_json)?;
        let client = build_client(&config).await?;
        let output = client
            .head_object()
            .bucket(&config.bucket)
            .key(input.object_path)
            .send()
            .await
            .map_err(|_| FileStorageError::ObjectChanged)?;
        let current = rustfs_snapshot(
            output.content_length(),
            output.e_tag(),
            output.version_id(),
            output.content_type(),
        )?;
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
        let config = parse_config(input.config_json)?;
        Ok(config
            .public_base_url
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), input.object_path)))
    }
}
