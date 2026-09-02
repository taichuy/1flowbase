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
