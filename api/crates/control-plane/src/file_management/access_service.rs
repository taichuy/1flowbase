use std::sync::Arc;

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::rngs::SysRng;
use rand_core::TryRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{EphemeralKvStore, FileManagementRepository, ModelDefinitionRepository},
};

/// Only this irreversible digest crosses the anonymous invocation boundary.
pub struct FileAccessKey(String);

impl FileAccessKey {
    pub fn from_token(token: &str) -> Result<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| denied())?;
        if bytes.len() != 32 || URL_SAFE_NO_PAD.encode(&bytes) != token {
            return Err(denied().into());
        }
        Ok(Self(format!("temporary-file:{}", hex_digest(token))))
    }
}

fn hex_digest(token: &str) -> String {
    format!(
        "{:x}",
        base16ct::HexDisplay(&Sha256::digest(token.as_bytes()))
    )
}

fn denied() -> ControlPlaneError {
    ControlPlaneError::PermissionDenied("temporary_file_access_invalid")
}

#[derive(Serialize, Deserialize)]
struct TemporaryFileGrant {
    file_table_id: Uuid,
    record_id: Uuid,
    scope_id: Uuid,
    storage_id: Uuid,
    path: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

#[derive(Serialize)]
pub struct FileShareLink {
    pub url: String,
    pub preview_url: Option<String>,
    pub expires_at: String,
}

pub struct FileContent {
    pub preview: bool,
    pub filename: String,
    pub content_type: String,
    pub content_length: u64,
    /// One-shot transfer across a Sync invocation result; IO starts only after projection.
    pub reader: std::sync::Mutex<storage_object::FileStorageStreamReader>,
}

pub struct FileAccessService<R> {
    repository: R,
    registry: Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    grants: Arc<dyn EphemeralKvStore>,
}

impl<R> FileAccessService<R>
where
    R: FileManagementRepository + ModelDefinitionRepository + Clone,
{
    pub fn new(
        repository: R,
        registry: Arc<storage_object::FileStorageDriverRegistry>,
        runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
        grants: Arc<dyn EphemeralKvStore>,
    ) -> Self {
        Self {
            repository,
            registry,
            runtime_engine,
            grants,
        }
    }

    async fn authorized_record(
        &self,
        actor: domain::ActorContext,
        file_table_id: Uuid,
        record_id: Uuid,
    ) -> Result<serde_json::Value> {
        let table = self
            .repository
            .get_file_table(file_table_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("file_table"))?;
        if table.status != "active" {
            return Err(ControlPlaneError::NotFound("file_table").into());
        }
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, table.model_definition_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let scope_grant =
            crate::model_definition::ModelDefinitionService::new(self.repository.clone())
                .load_runtime_scope_grant(
                    &actor,
                    model.id,
                    runtime_core::runtime_acl::RuntimeDataAction::View,
                )
                .await?;
        self.runtime_engine
            .get_record(runtime_core::runtime_engine::RuntimeGetInput {
                actor,
                model_code: model.code,
                record_id: record_id.to_string(),
                scope_grant,
            })
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("runtime_record").into())
    }

    pub async fn read(
        &self,
        actor: domain::ActorContext,
        file_table_id: Uuid,
        record_id: Uuid,
    ) -> Result<FileContent> {
        self.read_with_preview(actor, file_table_id, record_id, false)
            .await
    }

    pub async fn read_with_preview(
        &self,
        actor: domain::ActorContext,
        file_table_id: Uuid,
        record_id: Uuid,
        preview: bool,
    ) -> Result<FileContent> {
        let record = self
            .authorized_record(actor, file_table_id, record_id)
            .await?;
        self.open_record(&record, preview).await
    }

    pub async fn create_share_link(
        &self,
        actor: domain::ActorContext,
        file_table_id: Uuid,
        record_id: Uuid,
        expires_in_seconds: i64,
    ) -> Result<FileShareLink> {
        if !actor.has_permission("files.access_links.create") {
            return Err(ControlPlaneError::PermissionDenied("files.access_links.create").into());
        }
        if expires_in_seconds <= 0 {
            return Err(ControlPlaneError::InvalidInput("expires_in_seconds").into());
        }
        let now = OffsetDateTime::now_utc();
        let expires_at = now
            .checked_add(Duration::seconds(expires_in_seconds))
            .ok_or(ControlPlaneError::InvalidInput("expires_in_seconds"))?;
        // Validate serialization before creating any authorization state.
        let expires_at_text = expires_at
            .format(&Rfc3339)
            .map_err(|_| ControlPlaneError::InvalidInput("expires_in_seconds"))?;
        let record = self
            .authorized_record(actor, file_table_id, record_id)
            .await?;
        let preview_available = self.preview_available(&record).await?;
        let grant = TemporaryFileGrant {
            file_table_id,
            record_id,
            scope_id: Uuid::parse_str(record_text(&record, "scope_id")?)?,
            storage_id: record_storage_id(&record)?,
            path: record_text(&record, "path")?.into(),
            expires_at,
        };
        let mut bytes = [0u8; 32];
        SysRng.try_fill_bytes(&mut bytes)?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let key = FileAccessKey::from_token(&token)?;
        self.grants
            .set_json(
                &key.0,
                serde_json::to_value(grant)?,
                Some(Duration::seconds(expires_in_seconds)),
            )
            .await?;
        Ok(FileShareLink {
            url: format!("/api/public/files/content?token={token}"),
            preview_url: preview_available
                .then(|| format!("/api/public/files/content?token={token}&preview=true")),
            expires_at: expires_at_text,
        })
    }

    pub async fn read_temporary(&self, key: FileAccessKey) -> Result<FileContent> {
        self.read_temporary_with_preview(key, false).await
    }

    pub async fn read_temporary_with_preview(
        &self,
        key: FileAccessKey,
        preview: bool,
    ) -> Result<FileContent> {
        let value = self.grants.get_json(&key.0).await?.ok_or_else(denied)?;
        let grant: TemporaryFileGrant = serde_json::from_value(value)?;
        if grant.expires_at <= OffsetDateTime::now_utc() {
            self.grants.delete(&key.0).await?;
            return Err(denied().into());
        }
        // This is a scoped capability, not an impersonation of its issuer.
        let record = self
            .repository
            .get_file_record_for_access(grant.file_table_id, grant.record_id, grant.scope_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("runtime_record"))?;
        if record_storage_id(&record)? != grant.storage_id
            || record_text(&record, "path")? != grant.path
        {
            return Err(denied().into());
        }
        self.open_record(&record, preview).await
    }

    async fn preview_available(&self, record: &serde_json::Value) -> Result<bool> {
        let storage = self
            .repository
            .get_file_storage(record_storage_id(record)?)
            .await?
            .ok_or(ControlPlaneError::NotFound("file_storage"))?;
        Ok(
            domain::file_management::local_image_preview_enabled(&storage)
                && record
                    .get("mimetype")
                    .and_then(|v| v.as_str())
                    .and_then(domain::file_management::local_image_preview_mime)
                    .is_some(),
        )
    }

    async fn open_record(&self, record: &serde_json::Value, preview: bool) -> Result<FileContent> {
        let storage = self
            .repository
            .get_file_storage(record_storage_id(record)?)
            .await?
            .ok_or(ControlPlaneError::NotFound("file_storage"))?;
        let preview_mime = record
            .get("mimetype")
            .and_then(|v| v.as_str())
            .and_then(domain::file_management::local_image_preview_mime);
        if preview
            && (!domain::file_management::local_image_preview_enabled(&storage)
                || preview_mime.is_none())
        {
            return Err(ControlPlaneError::Conflict("file_preview_unavailable").into());
        }
        let driver = self
            .registry
            .get(&storage.driver_type)
            .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;
        let open = driver
            .open_read_stream(storage_object::OpenReadInput {
                config_json: &storage.config_json,
                object_path: record_text(record, "path")?,
            })
            .await?;
        Ok(FileContent {
            preview,
            filename: record_text(record, "filename")?.into(),
            content_type: if preview {
                preview_mime.expect("validated preview MIME").to_owned()
            } else {
                open.content_type
                    .or_else(|| {
                        record
                            .get("mimetype")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "application/octet-stream".into())
            },
            content_length: open.snapshot.content_length,
            reader: std::sync::Mutex::new(open.reader),
        })
    }
}

fn record_text<'a>(record: &'a serde_json::Value, field: &'static str) -> Result<&'a str> {
    record
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ControlPlaneError::InvalidInput(field).into())
}

fn record_storage_id(record: &serde_json::Value) -> Result<Uuid> {
    Uuid::parse_str(record_text(record, "storage_id")?)
        .map_err(|_| ControlPlaneError::InvalidInput("storage_id").into())
}
