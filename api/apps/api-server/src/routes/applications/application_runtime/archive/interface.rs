use std::sync::Arc;

use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    errors::ControlPlaneError,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use time::OffsetDateTime;
use uuid::Uuid;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ApplicationRuntimeArchiveInput {
    ExportOne {
        application_id: Uuid,
        run_id: Uuid,
        archive_version: Option<i32>,
    },
    ExportMany {
        application_id: Uuid,
        body: ApplicationRunArchiveBody,
    },
    CreateUploadSession {
        application_id: Uuid,
        body: RunArchiveUploadSessionCreateBody,
    },
    UploadChunk {
        application_id: Uuid,
        session_id: Uuid,
        chunk_index: i32,
        body: Vec<u8>,
        expected_sha256: String,
    },
    CompleteUploadSession {
        application_id: Uuid,
        session_id: Uuid,
    },
    GetImportJob {
        application_id: Uuid,
        job_id: Uuid,
    },
}

impl InterfaceContract for ApplicationRuntimeArchiveInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportOne")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
                (
                    "archive_version",
                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportMany")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "run_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "archive_version",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateUploadSession")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("total_size_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "expected_sha256",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "chunk_size_bytes",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UploadChunk")),
                ("application_id", mp::text_schema()),
                ("chunk_index", serde_json::json!({"type":"integer"})),
                (
                    "body",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "expected_sha256",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CompleteUploadSession")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetImportJob")),
                ("application_id", mp::text_schema()),
                ("job_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ExportOne {
                application_id: _field_application_id,
                run_id: _field_run_id,
                archive_version: _field_archive_version,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("ExportOne".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
                (
                    "archive_version",
                    match (_field_archive_version).as_ref() {
                        Some(item) => serde_json::json!(*(item)),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::ExportMany {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ExportMany".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        ("run_ids", {
                            if (&(_field_body).run_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).run_ids)
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "archive_version",
                            match (&(_field_body).archive_version).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::CreateUploadSession {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateUploadSession".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "total_size_bytes",
                            serde_json::json!(*(&(_field_body).total_size_bytes)),
                        ),
                        (
                            "expected_sha256",
                            match (&(_field_body).expected_sha256).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "chunk_size_bytes",
                            match (&(_field_body).chunk_size_bytes).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::UploadChunk {
                application_id: _field_application_id,
                chunk_index: _field_chunk_index,
                body: _field_body,
                expected_sha256: _field_expected_sha256,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UploadChunk".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("chunk_index", serde_json::json!(*(_field_chunk_index))),
                (
                    "body",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_body).len()))]),
                ),
                (
                    "expected_sha256",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((_field_expected_sha256).len()),
                    )]),
                ),
            ]),
            Self::CompleteUploadSession {
                application_id: _field_application_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CompleteUploadSession".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
            ]),
            Self::GetImportJob {
                application_id: _field_application_id,
                job_id: _field_job_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetImportJob".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "job_id",
                    serde_json::Value::String((_field_job_id).to_string()),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-archive-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ArchiveDownload {
    pub(crate) filename: String,
    pub(crate) body: Vec<u8>,
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed archive output is projected immediately into the console response"
)]
pub(crate) enum ApplicationRuntimeArchiveOutput {
    Download(ArchiveDownload),
    UploadSession(RunArchiveUploadSessionResponse),
    Chunk(RunArchiveChunkUploadResponse),
    ImportJob(RunArchiveImportJobResponse),
}

impl InterfaceContract for ApplicationRuntimeArchiveOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Download")),
                (
                    "0",
                    mp::object_schema(&[(
                        "body",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UploadSession")),
                (
                    "0",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        ("status", mp::text_schema()),
                        ("total_size_bytes", serde_json::json!({"type":"integer"})),
                        ("received_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "expected_sha256",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Chunk")),
                (
                    "0",
                    mp::object_schema(&[
                        ("chunk_index", serde_json::json!({"type":"integer"})),
                        ("chunk_size_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "chunk_sha256",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("received_bytes", serde_json::json!({"type":"integer"})),
                        ("status", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportJob")),
                (
                    "0",
                    mp::object_schema(&[
                        ("job_id", mp::text_schema()),
                        ("application_id", mp::text_schema()),
                        ("status", mp::text_schema()),
                        (
                            "archive_version",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "archive_sha256",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("run_count", serde_json::json!({"type":"integer"})),
                        ("imported_run_count", serde_json::json!({"type":"integer"})),
                        (
                            "source_to_target_run_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source_run_id",mp::text_schema()), ("target_run_id",mp::text_schema())])}),
                        ),
                        (
                            "error_payload",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        ("result_payload", mp::json_summary_schema()),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                        (
                            "started_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "finished_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Download(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Download".to_owned())),
                (
                    "0",
                    mp::object_value(&[(
                        "body",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_0).body).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::UploadSession(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UploadSession".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("application_id", mp::text(&(_field_0).application_id)?),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "total_size_bytes",
                            serde_json::json!(*(&(_field_0).total_size_bytes)),
                        ),
                        (
                            "received_bytes",
                            serde_json::json!(*(&(_field_0).received_bytes)),
                        ),
                        (
                            "expected_sha256",
                            match (&(_field_0).expected_sha256).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::Chunk(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Chunk".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("chunk_index", serde_json::json!(*(&(_field_0).chunk_index))),
                        (
                            "chunk_size_bytes",
                            serde_json::json!(*(&(_field_0).chunk_size_bytes)),
                        ),
                        (
                            "chunk_sha256",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).chunk_sha256).len()),
                            )]),
                        ),
                        (
                            "received_bytes",
                            serde_json::json!(*(&(_field_0).received_bytes)),
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                    ]),
                ),
            ]),
            Self::ImportJob(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("ImportJob".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("job_id", mp::text(&(_field_0).job_id)?),
                        ("application_id", mp::text(&(_field_0).application_id)?),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "archive_version",
                            match (&(_field_0).archive_version).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "archive_sha256",
                            match (&(_field_0).archive_sha256).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("run_count", serde_json::json!(*(&(_field_0).run_count))),
                        (
                            "imported_run_count",
                            serde_json::json!(*(&(_field_0).imported_run_count)),
                        ),
                        ("source_to_target_run_ids", {
                            if (&(_field_0).source_to_target_run_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).source_to_target_run_ids)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("source_run_id", mp::text(&(item).source_run_id)?),
                                            ("target_run_id", mp::text(&(item).target_run_id)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "error_payload",
                            match (&(_field_0).error_payload).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "result_payload",
                            mp::json_summary(&(_field_0).result_payload),
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                        (
                            "started_at",
                            match (&(_field_0).started_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "finished_at",
                            match (&(_field_0).finished_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-archive-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct ApplicationRuntimeArchiveDependencies {
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
}

pub(crate) fn dependencies(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> ApplicationRuntimeArchiveDependencies {
    ApplicationRuntimeArchiveDependencies {
        store,
        file_storage_registry,
    }
}

struct ApplicationRuntimeArchiveAdapter {
    dependencies: ApplicationRuntimeArchiveDependencies,
}

fn ensure_run_archive_version(version: Option<i32>) -> Result<(), ApiError> {
    if version.unwrap_or(RUN_ARCHIVE_VERSION) == RUN_ARCHIVE_VERSION {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("unsupported_archive_version").into())
    }
}

impl ApplicationRuntimeArchiveAdapter {
    async fn application(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        operation: ApplicationNonCrudConsoleOperation,
    ) -> Result<domain::ApplicationRecord, ApiError> {
        let actor = principal.actor();
        Ok(
            ApplicationService::new(self.dependencies.store.for_actor(actor.clone()))
                .load_application_for_non_crud_console_operation(
                    actor.user_id,
                    application_id,
                    operation,
                )
                .await?,
        )
    }

    async fn export(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        run_ids: Vec<Uuid>,
        archive_version: Option<i32>,
    ) -> Result<ApplicationRuntimeArchiveOutput, ApiError> {
        ensure_run_archive_version(archive_version)?;
        if run_ids.is_empty() {
            return Err(ControlPlaneError::InvalidInput("run_ids").into());
        }
        let actor = principal.actor();
        let application = self
            .application(
                principal,
                application_id,
                ApplicationNonCrudConsoleOperation::LogsExport,
            )
            .await?;
        let archive = build_run_archive_v1_document(
            self.dependencies.store.clone(),
            self.dependencies.file_storage_registry.clone(),
            actor.current_workspace_id,
            actor.user_id,
            &application,
            run_ids,
            OffsetDateTime::now_utc(),
        )
        .await?;
        let filename = application_run_archive_filename(
            &archive.source.application_name,
            &archive.exported_at,
            archive.entries.len(),
        );
        Ok(ApplicationRuntimeArchiveOutput::Download(ArchiveDownload {
            filename,
            body: serde_json::to_vec_pretty(&archive)?,
        }))
    }

    async fn create_upload_session(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        body: RunArchiveUploadSessionCreateBody,
    ) -> Result<ApplicationRuntimeArchiveOutput, ApiError> {
        let actor = principal.actor();
        let application = self
            .application(
                principal,
                application_id,
                ApplicationNonCrudConsoleOperation::LogsImport,
            )
            .await?;
        if body.total_size_bytes <= 0 {
            return Err(ControlPlaneError::InvalidInput("total_size_bytes").into());
        }
        if body.total_size_bytes > RUN_ARCHIVE_UPLOAD_MAX_BYTES {
            return Err(ControlPlaneError::InvalidInput("archive_size").into());
        }
        let expected_sha256 = body
            .expected_sha256
            .as_deref()
            .ok_or(ControlPlaneError::InvalidInput("expected_sha256"))?;
        ensure_sha256_value(expected_sha256, "expected_sha256")?;
        let chunk_size_bytes = body
            .chunk_size_bytes
            .ok_or(ControlPlaneError::InvalidInput("chunk_size_bytes"))?;
        if chunk_size_bytes <= 0 || chunk_size_bytes > RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES {
            return Err(ControlPlaneError::InvalidInput("chunk_size_bytes").into());
        }
        if expected_archive_chunk_count(body.total_size_bytes, chunk_size_bytes)?
            > RUN_ARCHIVE_UPLOAD_MAX_CHUNKS
        {
            return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
        }
        let session_id = Uuid::now_v7();
        persist_run_archive_upload_session(
            &self.dependencies.store,
            PersistRunArchiveUploadSessionInput {
                session_id,
                scope_id: application.workspace_id,
                application_id: application.id,
                actor_user_id: actor.user_id,
                filename: body.filename.as_deref(),
                total_size_bytes: body.total_size_bytes,
                expected_sha256,
                chunk_size_bytes,
            },
        )
        .await?;
        let session =
            load_run_archive_upload_session(&self.dependencies.store, application_id, session_id)
                .await?;
        Ok(ApplicationRuntimeArchiveOutput::UploadSession(
            to_upload_session_response(session),
        ))
    }

    async fn upload_chunk(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        session_id: Uuid,
        chunk_index: i32,
        body: Vec<u8>,
        expected_sha256: String,
    ) -> Result<ApplicationRuntimeArchiveOutput, ApiError> {
        self.application(
            principal,
            application_id,
            ApplicationNonCrudConsoleOperation::LogsImport,
        )
        .await?;
        if chunk_index < 0 || body.is_empty() {
            return Err(ControlPlaneError::InvalidInput("archive_chunk").into());
        }
        let session =
            load_run_archive_upload_session(&self.dependencies.store, application_id, session_id)
                .await?;
        if session.status != "uploading" {
            return Err(ControlPlaneError::Conflict("archive_upload_session").into());
        }
        if i64::try_from(body.len()).unwrap_or(i64::MAX) > session.chunk_size_bytes {
            return Err(ControlPlaneError::InvalidInput("chunk_size_bytes").into());
        }
        if i64::from(chunk_index)
            >= expected_archive_chunk_count(session.total_size_bytes, session.chunk_size_bytes)?
        {
            return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
        }
        let actual_sha256 = sha256_bytes(&body);
        ensure_sha256_value(&expected_sha256, "chunk_sha256")?;
        if normalize_sha256(&expected_sha256) != normalize_sha256(&actual_sha256) {
            return Err(ControlPlaneError::InvalidInput("chunk_sha256").into());
        }
        let received_bytes = persist_run_archive_chunk(
            &self.dependencies.store,
            PersistRunArchiveChunkInput {
                chunk_id: Uuid::now_v7(),
                scope_id: session.scope_id,
                session_id,
                chunk_index,
                content: &body,
                chunk_sha256: &actual_sha256,
                actor_user_id: principal.actor().user_id,
                total_size_bytes: session.total_size_bytes,
            },
        )
        .await?;
        Ok(ApplicationRuntimeArchiveOutput::Chunk(
            RunArchiveChunkUploadResponse {
                session_id: session_id.to_string(),
                chunk_index,
                chunk_size_bytes: i64::try_from(body.len()).unwrap_or(i64::MAX),
                chunk_sha256: actual_sha256,
                received_bytes,
                status: "uploading".to_string(),
            },
        ))
    }

    async fn complete_upload_session(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        session_id: Uuid,
    ) -> Result<ApplicationRuntimeArchiveOutput, ApiError> {
        let application = self
            .application(
                principal,
                application_id,
                ApplicationNonCrudConsoleOperation::LogsImport,
            )
            .await?;
        let session =
            load_run_archive_upload_session(&self.dependencies.store, application_id, session_id)
                .await?;
        if session.status != "uploading" {
            return Err(ControlPlaneError::Conflict("archive_upload_session").into());
        }
        let archive_bytes =
            load_upload_session_archive_bytes(&self.dependencies.store, session_id).await?;
        if i64::try_from(archive_bytes.len()).unwrap_or(i64::MAX) != session.total_size_bytes {
            return Err(ControlPlaneError::InvalidInput("archive_size").into());
        }
        let archive_sha256 = sha256_bytes(&archive_bytes);
        let expected_sha256 = session
            .expected_sha256
            .as_deref()
            .ok_or(ControlPlaneError::InvalidInput("expected_sha256"))?;
        ensure_sha256_value(expected_sha256, "expected_sha256")?;
        if normalize_sha256(expected_sha256) != normalize_sha256(&archive_sha256) {
            return Err(ControlPlaneError::InvalidInput("archive_sha256").into());
        }
        let archive = parse_run_archive_v1(&archive_bytes)?;
        let actor = principal.actor();
        let job_id = create_run_archive_import_job(
            &self.dependencies.store,
            CreateRunArchiveImportJobInput {
                workspace_id: application.workspace_id,
                application_id: application.id,
                actor_user_id: actor.user_id,
                session_id,
                archive_version: archive.archive_version,
                archive_sha256: &archive_sha256,
                run_count: i32::try_from(archive.entries.len()).unwrap_or(i32::MAX),
            },
        )
        .await?;
        mark_upload_session_completed(&self.dependencies.store, session_id).await?;
        cleanup_run_archive_upload_chunks(&self.dependencies.store, session_id).await?;
        let store = self.dependencies.store.clone();
        let restore_store = store.clone();
        let restore_actor = actor.clone();
        tokio::spawn(async move {
            if let Err(error) = restore_run_archive_v1(
                restore_store.clone(),
                &application,
                restore_actor,
                job_id,
                archive,
            )
            .await
            {
                tracing::error!("run archive restore failed: {}", error.0);
                if let Err(mark_error) =
                    mark_run_archive_import_job_failed(&restore_store, job_id, error.0.to_string())
                        .await
                {
                    tracing::error!(
                        "failed to mark run archive import job failed: {}",
                        mark_error.0
                    );
                }
            }
        });
        let job = load_run_archive_import_job(&store, application_id, job_id).await?;
        Ok(ApplicationRuntimeArchiveOutput::ImportJob(
            to_import_job_response(&store, job).await?,
        ))
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeArchiveInput,
    ) -> Result<ApplicationRuntimeArchiveOutput, ApiError> {
        match input {
            ApplicationRuntimeArchiveInput::ExportOne {
                application_id,
                run_id,
                archive_version,
            } => {
                self.export(principal, application_id, vec![run_id], archive_version)
                    .await
            }
            ApplicationRuntimeArchiveInput::ExportMany {
                application_id,
                body,
            } => {
                self.export(
                    principal,
                    application_id,
                    body.run_ids,
                    body.archive_version,
                )
                .await
            }
            ApplicationRuntimeArchiveInput::CreateUploadSession {
                application_id,
                body,
            } => {
                self.create_upload_session(principal, application_id, body)
                    .await
            }
            ApplicationRuntimeArchiveInput::UploadChunk {
                application_id,
                session_id,
                chunk_index,
                body,
                expected_sha256,
            } => {
                self.upload_chunk(
                    principal,
                    application_id,
                    session_id,
                    chunk_index,
                    body,
                    expected_sha256,
                )
                .await
            }
            ApplicationRuntimeArchiveInput::CompleteUploadSession {
                application_id,
                session_id,
            } => {
                self.complete_upload_session(principal, application_id, session_id)
                    .await
            }
            ApplicationRuntimeArchiveInput::GetImportJob {
                application_id,
                job_id,
            } => {
                self.application(
                    principal,
                    application_id,
                    ApplicationNonCrudConsoleOperation::LogsImport,
                )
                .await?;
                let job =
                    load_run_archive_import_job(&self.dependencies.store, application_id, job_id)
                        .await?;
                Ok(ApplicationRuntimeArchiveOutput::ImportJob(
                    to_import_job_response(&self.dependencies.store, job).await?,
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ApplicationRuntimeArchiveInput, ApplicationRuntimeArchiveOutput>
    for ApplicationRuntimeArchiveAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeArchiveInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeArchiveOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.run.export",
        binding_id: "http.console.applications.runtime.archive.run.export.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/archive",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.runs.export",
        binding_id: "http.console.applications.runtime.archive.runs.export.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/runs/archive",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.upload-sessions.create",
        binding_id: "http.console.applications.runtime.archive.upload-sessions.create.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/runs/archive/import-sessions",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.upload-chunks.upsert",
        binding_id: "http.console.applications.runtime.archive.upload-chunks.upsert.v1",
        method: "PUT",
        path: "/api/console/applications/:id/logs/runs/archive/import-sessions/:session_id/chunks/:chunk_index",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.upload-sessions.complete",
        binding_id: "http.console.applications.runtime.archive.upload-sessions.complete.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/runs/archive/import-sessions/:session_id/complete",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.archive.import-jobs.get",
        binding_id: "http.console.applications.runtime.archive.import-jobs.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/archive/import-jobs/:job_id",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: ApplicationRuntimeArchiveDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-archive",
        "graph:console-application-runtime-archive-v1",
        DECLARATIONS,
        Arc::new(ApplicationRuntimeArchiveAdapter { dependencies }),
    )
}
