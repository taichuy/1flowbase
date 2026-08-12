use std::{collections::BTreeMap, convert::Infallible, sync::Arc};

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    system_recovery::{recovery_plan_digest, ConfirmedRecoveryIntent, RecoveryPlan},
};
use domain::{BackupSetId, ContentDigest, RecoveryJobId};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::{ApiError, ApiServiceUnavailable},
    middleware::{require_csrf::require_csrf, require_session::require_session},
    recovery_authorization::{
        consume_reauth_challenge, issue_reauth_challenge, recovery_intent_ttl,
    },
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

const CREATE: &str = "system_backups.create";
const DELETE: &str = "system_backups.delete";
const DETAIL: &str = "system_backups.detail";
const DOWNLOAD: &str = "system_backups.download";
const IMPORT: &str = "system_backups.import";
const LIST: &str = "system_backups.list";
const RECOVERY_INTENT: &str = "system_backups.recovery.intent";
const RECOVERY_PREFLIGHT: &str = "system_backups.recovery.preflight";
const RECOVERY_REAUTH: &str = "system_backups.recovery.reauth";
const RECOVERY_STATUS: &str = "system_backups.recovery.status";
const VERIFY: &str = "system_backups.verify";

fn require_system_backup(
    state: &ApiState,
) -> Result<Arc<crate::system_backup::SystemBackupRuntime>, ApiError> {
    state
        .system_backup
        .clone()
        .ok_or_else(|| ApiServiceUnavailable("system_backup_unavailable").into())
}

fn owned(operation: &str) -> access_control::ConsoleRouteOwnership {
    access_control::ConsoleRouteOwnership::ConsoleOperation(operation.to_owned())
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/settings/system-backups",
            console_get(list_backups, owned(LIST))
                .post(create_backup, owned(CREATE))
                .coordinator_control(),
        )
        .route(
            "/settings/system-backups/import",
            console_post(import_backup, owned(IMPORT)),
        )
        .route(
            "/settings/system-backups/recovery/reauth",
            console_post(reauthenticate_recovery, owned(RECOVERY_REAUTH)).coordinator_control(),
        )
        .route(
            "/settings/system-backups/recovery/status",
            console_get(get_recovery_status, owned(RECOVERY_STATUS)),
        )
        .route(
            "/settings/system-backups/:backup_set_id",
            console_get(get_backup, owned(DETAIL)).delete(delete_backup, owned(DELETE)),
        )
        .route(
            "/settings/system-backups/:backup_set_id/verify",
            console_post(verify_backup, owned(VERIFY)),
        )
        .route(
            "/settings/system-backups/:backup_set_id/download",
            console_get(download_backup, owned(DOWNLOAD)),
        )
        .route(
            "/settings/system-backups/:backup_set_id/recovery/preflight",
            console_post(preflight_recovery, owned(RECOVERY_PREFLIGHT)).coordinator_control(),
        )
        .route(
            "/settings/system-backups/:backup_set_id/recovery/intents",
            console_post(create_recovery_intent, owned(RECOVERY_INTENT)).coordinator_control(),
        )
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupSetSummaryResponse {
    pub backup_set_id: BackupSetId,
    pub exact_backup_name: String,
    pub created_at: String,
    pub availability: domain::BackupSetAvailability,
    pub total_size_bytes: u64,
    pub envelope_digest: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupSetListResponse {
    pub items: Vec<BackupSetSummaryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupSetDetailResponse {
    pub backup_set_id: BackupSetId,
    pub exact_backup_name: String,
    pub created_at: String,
    pub content: BackupContentSummaryResponse,
    pub components: Vec<BackupComponentDetailResponse>,
    pub compatibility: BackupCompatibilityResponse,
    pub verification: BackupVerificationDetailResponse,
    pub creation_journal: Vec<BackupCreationJournalEntryResponse>,
    pub recovery_history: Vec<BackupRecoveryHistoryEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupContentSummaryResponse {
    pub component_count: u64,
    pub postgresql_count: u64,
    pub business_object_count: u64,
    pub extension_artifact_count: u64,
    pub mcp_artifact_count: u64,
    pub embedded_component_count: u64,
    pub identity_only_component_count: u64,
    pub total_size_bytes: u64,
    #[schema(value_type = Vec<String>)]
    pub excluded_domains: Vec<domain::BackupExcludedDomain>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupComponentDetailResponse {
    pub component_id: String,
    pub kind: domain::BackupComponentKind,
    pub source_identity: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub content_digest: String,
    pub disposition: domain::BackupComponentDisposition,
    pub rebuildability: domain::ArtifactRebuildability,
    pub restore_target: domain::BackupComponentRestoreTarget,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupCompatibilityResponse {
    pub compatible: bool,
    pub failures: Vec<domain::BackupIncompatibility>,
    pub format_version: u32,
    pub application_build: String,
    pub migration_head: String,
    pub master_key_fingerprint: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupVerificationDetailResponse {
    pub verified: Option<bool>,
    pub checked_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupCreationJournalEntryResponse {
    pub sequence: u64,
    pub occurred_at: String,
    pub state: Option<domain::BackupJobState>,
    pub component_id: Option<String>,
    pub failure_code: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupRecoveryHistoryEntryResponse {
    pub recovery_job_id: RecoveryJobId,
    pub status: Option<domain::RecoveryJobState>,
    pub started_at: String,
    pub updated_at: String,
    pub failure_code: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupMutationResponse {
    pub backup_set_id: BackupSetId,
    pub exact_backup_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupVerificationResponse {
    pub backup_set_id: BackupSetId,
    pub verified: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecoveryPreflightResponse {
    pub backup_set_id: BackupSetId,
    pub plan_digest: String,
    pub compatible: bool,
    pub required_space_bytes: u64,
    pub available_space_bytes: u64,
    #[schema(value_type = Object)]
    pub impact: serde_json::Value,
    pub failures: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecoveryReauthRequest {
    pub backup_set_id: BackupSetId,
    pub exact_backup_name: String,
    pub plan_digest: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecoveryReauthResponse {
    pub challenge_token: Uuid,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRecoveryIntentRequest {
    pub challenge_token: Uuid,
    pub exact_backup_name: String,
    pub plan_digest: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecoveryIntentResponse {
    pub intent_id: Uuid,
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub status: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecoveryStatusResponse {
    pub phase: String,
    pub recovery_job_id: Option<RecoveryJobId>,
    pub active_write_count: u64,
    pub started_at: Option<String>,
    pub target_backup_set_id: Option<BackupSetId>,
    pub safety_backup_set_id: Option<BackupSetId>,
    pub plan_digest: Option<String>,
    pub journal_state: Option<domain::RecoveryJobState>,
    #[schema(value_type = Vec<Object>)]
    pub journal_events: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecoveryStatusQuery {
    pub recovery_job_id: Option<Uuid>,
}

#[utoipa::path(get, path = "/api/console/settings/system-backups", responses((status = 200, body = BackupSetListResponse)))]
pub async fn list_backups(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<BackupSetListResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let items = require_system_backup(&state)?
        .list()
        .await?
        .into_iter()
        .map(|entry| BackupSetSummaryResponse {
            exact_backup_name: canonical_backup_name(entry.backup_set_id),
            backup_set_id: entry.backup_set_id,
            created_at: entry.created_at.to_string(),
            availability: entry.availability,
            total_size_bytes: entry.total_size_bytes,
            envelope_digest: entry.envelope_digest.map(|value| value.as_str().to_owned()),
        })
        .collect();
    Ok(Json(ApiSuccess::new(BackupSetListResponse { items })))
}

#[utoipa::path(post, path = "/api/console/settings/system-backups", responses((status = 200, body = BackupMutationResponse)))]
pub async fn create_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<BackupMutationResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let sealed = require_system_backup(&state)?
        .create(context.actor.user_id)
        .await?;
    Ok(Json(ApiSuccess::new(mutation_response(
        sealed.manifest().backup_set_id(),
    ))))
}

#[utoipa::path(get, path = "/api/console/settings/system-backups/{backup_set_id}", params(("backup_set_id" = Uuid, Path)), responses((status = 200, body = BackupSetDetailResponse)))]
pub async fn get_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<BackupSetDetailResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let id = BackupSetId::from_uuid(backup_set_id);
    let detail = require_system_backup(&state)?.detail(id).await?;
    Ok(Json(ApiSuccess::new(detail_response(id, detail))))
}

#[utoipa::path(delete, path = "/api/console/settings/system-backups/{backup_set_id}", params(("backup_set_id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    require_system_backup(&state)?
        .delete(BackupSetId::from_uuid(backup_set_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/settings/system-backups/{backup_set_id}/verify", params(("backup_set_id" = Uuid, Path)), responses((status = 200, body = BackupVerificationResponse)))]
pub async fn verify_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<BackupVerificationResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let id = BackupSetId::from_uuid(backup_set_id);
    require_system_backup(&state)?.verify(id).await?;
    Ok(Json(ApiSuccess::new(BackupVerificationResponse {
        backup_set_id: id,
        verified: true,
    })))
}

#[utoipa::path(post, path = "/api/console/settings/system-backups/import", request_body(content = String, content_type = "application/octet-stream"), responses((status = 200, body = BackupMutationResponse)))]
pub async fn import_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<ApiSuccess<BackupMutationResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let runtime = require_system_backup(&state)?;
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/octet-stream")
    {
        return Err(ControlPlaneError::InvalidInput("backup_content_type").into());
    }
    let (mut reader, mut writer) = tokio::io::duplex(256 * 1024);
    tokio::spawn(async move {
        let mut stream = body.into_data_stream();
        while let Some(frame) = stream.next().await {
            match frame {
                Ok(bytes) if writer.write_all(&bytes).await.is_ok() => {}
                _ => break,
            }
        }
        let _ = writer.shutdown().await;
    });
    let sealed = runtime.import(&mut reader).await?;
    Ok(Json(ApiSuccess::new(mutation_response(
        sealed.manifest().backup_set_id(),
    ))))
}

#[utoipa::path(get, path = "/api/console/settings/system-backups/{backup_set_id}/download", params(("backup_set_id" = Uuid, Path)), responses((status = 200, content_type = "application/octet-stream", body = String)))]
pub async fn download_backup(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
) -> Result<Response<Body>, ApiError> {
    require_session(&state, &headers).await?;
    let id = BackupSetId::from_uuid(backup_set_id);
    let runtime = require_system_backup(&state)?;
    runtime.get(id).await?;
    let (reader, writer) = tokio::io::duplex(256 * 1024);
    tokio::spawn(async move {
        if let Err(error) = runtime.download(id, writer).await {
            tracing::warn!(backup_set_id = %id.as_uuid(), error = %error, "backup download stream failed");
        }
    });
    let stream = stream::unfold(reader, |mut reader| async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        match reader.read(&mut buffer).await {
            Ok(0) => None,
            Ok(read) => {
                buffer.truncate(read);
                Some((Ok::<Bytes, Infallible>(Bytes::from(buffer)), reader))
            }
            Err(_) => None,
        }
    });
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}.1fb-backup\"",
            id.as_uuid()
        ))
        .map_err(|_| ControlPlaneError::InvalidInput("backup_set_id"))?,
    );
    Ok(response)
}

#[utoipa::path(post, path = "/api/console/settings/system-backups/{backup_set_id}/recovery/preflight", params(("backup_set_id" = Uuid, Path)), responses((status = 200, body = RecoveryPreflightResponse)))]
pub async fn preflight_recovery(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<RecoveryPreflightResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    require_root_cookie(&context)?;
    let plan = require_system_backup(&state)?
        .preflight(BackupSetId::from_uuid(backup_set_id))
        .await;
    Ok(Json(ApiSuccess::new(preflight_response(&plan)?)))
}

#[utoipa::path(post, path = "/api/console/settings/system-backups/recovery/reauth", request_body = RecoveryReauthRequest, responses((status = 200, body = RecoveryReauthResponse)))]
pub async fn reauthenticate_recovery(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<RecoveryReauthRequest>,
) -> Result<Json<ApiSuccess<RecoveryReauthResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = require_root_cookie(&context)?;
    validate_exact_name(body.backup_set_id, &body.exact_backup_name)?;
    let plan = require_system_backup(&state)?
        .preflight(body.backup_set_id)
        .await;
    require_compatible_digest(&plan, &body.plan_digest)?;
    let parsed = PasswordHash::new(&context.user.password_hash)
        .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed)
        .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
    let digest = ContentDigest::try_from(body.plan_digest)
        .map_err(|_| ControlPlaneError::InvalidInput("plan_digest"))?;
    let challenge = issue_reauth_challenge(
        context.actor.user_id,
        &session.session_id,
        body.backup_set_id,
        digest,
        &body.exact_backup_name,
    );
    Ok(Json(ApiSuccess::new(RecoveryReauthResponse {
        challenge_token: challenge.token,
        expires_at: challenge.expires_at.to_string(),
    })))
}

#[utoipa::path(post, path = "/api/console/settings/system-backups/{backup_set_id}/recovery/intents", params(("backup_set_id" = Uuid, Path)), request_body = CreateRecoveryIntentRequest, responses((status = 202, body = RecoveryIntentResponse)))]
pub async fn create_recovery_intent(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(backup_set_id): Path<Uuid>,
    Json(body): Json<CreateRecoveryIntentRequest>,
) -> Result<(StatusCode, Json<ApiSuccess<RecoveryIntentResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = require_root_cookie(&context)?;
    let backup_set_id = BackupSetId::from_uuid(backup_set_id);
    validate_exact_name(backup_set_id, &body.exact_backup_name)?;
    let runtime = require_system_backup(&state)?;
    let plan = runtime.preflight(backup_set_id).await;
    let plan_digest = require_compatible_digest(&plan, &body.plan_digest)?;
    consume_reauth_challenge(
        body.challenge_token,
        context.actor.user_id,
        &session.session_id,
        backup_set_id,
        &plan_digest,
        &body.exact_backup_name,
    )
    .map_err(ControlPlaneError::PermissionDenied)?;

    let now = OffsetDateTime::now_utc();
    let expires_at = now + recovery_intent_ttl();
    let intent_id = Uuid::now_v7();
    let recovery_job_id = RecoveryJobId::new();
    let confirmed = ConfirmedRecoveryIntent::try_new(
        intent_id,
        recovery_job_id,
        context.actor.user_id,
        backup_set_id,
        plan_digest,
        now,
        expires_at,
    )
    .map_err(|_| ControlPlaneError::InvalidInput("recovery_intent"))?;
    tokio::spawn(async move {
        if let Err(error) = runtime.prepare_recovery(confirmed).await {
            tracing::error!(intent_id = %intent_id, recovery_job_id = %recovery_job_id.as_uuid(), error = %error, "recovery handoff preparation failed");
        }
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(ApiSuccess::new(RecoveryIntentResponse {
            intent_id,
            recovery_job_id,
            backup_set_id,
            status: "preparing".to_owned(),
            expires_at: expires_at.to_string(),
        })),
    ))
}

#[utoipa::path(get, path = "/api/console/settings/system-backups/recovery/status", params(RecoveryStatusQuery), responses((status = 200, body = RecoveryStatusResponse)))]
pub async fn get_recovery_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RecoveryStatusQuery>,
) -> Result<Json<ApiSuccess<RecoveryStatusResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_root_cookie(&context)?;
    let runtime = require_system_backup(&state)?;
    let maintenance = runtime.maintenance_status();
    let active = runtime.active_recovery();
    let requested_job_id = query
        .recovery_job_id
        .map(RecoveryJobId::from_uuid)
        .or(maintenance.recovery_job_id);
    let journal = match requested_job_id {
        Some(recovery_job_id) => runtime.recovery_journal(recovery_job_id).await?,
        None => Vec::new(),
    };
    let journal_state = journal.iter().rev().find_map(|event| match &event.event {
        domain::BackupJournalEventKind::RecoveryStateChanged { state } => Some(*state),
        _ => None,
    });
    let journal_events = journal
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let phase = match maintenance.phase {
        control_plane::system_recovery::SystemMaintenancePhase::Online => "online",
        control_plane::system_recovery::SystemMaintenancePhase::Draining => "draining",
        control_plane::system_recovery::SystemMaintenancePhase::Active => "active",
    };
    Ok(Json(ApiSuccess::new(RecoveryStatusResponse {
        phase: phase.to_owned(),
        recovery_job_id: requested_job_id,
        active_write_count: maintenance.active_write_count() as u64,
        started_at: maintenance.started_at.map(|value| value.to_string()),
        target_backup_set_id: active.as_ref().map(|value| value.target_backup_set_id),
        safety_backup_set_id: active.as_ref().map(|value| value.safety_backup_set_id),
        plan_digest: active.map(|value| value.plan_digest.as_str().to_owned()),
        journal_state,
        journal_events,
    })))
}

fn canonical_backup_name(id: BackupSetId) -> String {
    id.as_uuid().to_string()
}

fn detail_response(
    backup_set_id: BackupSetId,
    detail: crate::system_backup::SystemBackupDetail,
) -> BackupSetDetailResponse {
    let manifest = detail.sealed.manifest();
    let mut content = BackupContentSummaryResponse {
        component_count: manifest.components().len() as u64,
        postgresql_count: 0,
        business_object_count: 0,
        extension_artifact_count: 0,
        mcp_artifact_count: 0,
        embedded_component_count: 0,
        identity_only_component_count: 0,
        total_size_bytes: manifest.total_size_bytes(),
        excluded_domains: manifest.excluded_domains().iter().copied().collect(),
    };
    let components = manifest
        .components()
        .iter()
        .map(|component| {
            match component.kind {
                domain::BackupComponentKind::PostgreSql => content.postgresql_count += 1,
                domain::BackupComponentKind::BusinessObject => content.business_object_count += 1,
                domain::BackupComponentKind::ExtensionArtifact => {
                    content.extension_artifact_count += 1
                }
                domain::BackupComponentKind::McpArtifact => content.mcp_artifact_count += 1,
            }
            match component.disposition {
                domain::BackupComponentDisposition::Embedded => {
                    content.embedded_component_count += 1
                }
                domain::BackupComponentDisposition::IdentityOnly => {
                    content.identity_only_component_count += 1
                }
            }
            BackupComponentDetailResponse {
                component_id: component.component_id.as_str().to_owned(),
                kind: component.kind,
                source_identity: component.source_identity.as_str().to_owned(),
                content_type: component.content_type.clone(),
                size_bytes: component.size_bytes,
                content_digest: component.content_digest.as_str().to_owned(),
                disposition: component.disposition,
                rebuildability: component.rebuildability,
                restore_target: component.restore_target.clone(),
            }
        })
        .collect();
    let creation_journal = detail
        .creation_journal
        .into_iter()
        .map(|event| {
            let (state, component_id, failure_code) = match event.event {
                domain::BackupJournalEventKind::BackupStateChanged { state } => {
                    (Some(state), None, None)
                }
                domain::BackupJournalEventKind::ComponentSealed { component_id } => {
                    (None, Some(component_id.as_str().to_owned()), None)
                }
                domain::BackupJournalEventKind::TerminalFailure { code } => {
                    (None, None, Some(code))
                }
                _ => (None, None, None),
            };
            BackupCreationJournalEntryResponse {
                sequence: event.sequence,
                occurred_at: event.occurred_at.to_string(),
                state,
                component_id,
                failure_code,
            }
        })
        .collect();
    let mut recovery_history = BTreeMap::<RecoveryJobId, BackupRecoveryHistoryEntryResponse>::new();
    for event in detail.recovery_history {
        let domain::BackupJournalSubject::Recovery(recovery_job_id) = event.subject else {
            continue;
        };
        let entry = recovery_history.entry(recovery_job_id).or_insert_with(|| {
            BackupRecoveryHistoryEntryResponse {
                recovery_job_id,
                status: None,
                started_at: event.occurred_at.to_string(),
                updated_at: event.occurred_at.to_string(),
                failure_code: None,
            }
        });
        entry.updated_at = event.occurred_at.to_string();
        match event.event {
            domain::BackupJournalEventKind::RecoveryStateChanged { state } => {
                entry.status = Some(state)
            }
            domain::BackupJournalEventKind::TerminalFailure { code } => {
                entry.failure_code = Some(code)
            }
            _ => {}
        }
    }
    let verification = detail.verification;
    BackupSetDetailResponse {
        backup_set_id,
        exact_backup_name: canonical_backup_name(backup_set_id),
        created_at: manifest.created_at().to_string(),
        content,
        components,
        compatibility: BackupCompatibilityResponse {
            compatible: detail.compatibility_failures.is_empty(),
            failures: detail.compatibility_failures,
            format_version: manifest.format_version(),
            application_build: manifest.application_build().as_str().to_owned(),
            migration_head: manifest.migration_head().as_str().to_owned(),
            master_key_fingerprint: manifest.master_key_fingerprint().as_str().to_owned(),
        },
        verification: BackupVerificationDetailResponse {
            verified: verification.as_ref().map(|receipt| receipt.verified),
            checked_at: verification.map(|receipt| receipt.checked_at.to_string()),
        },
        creation_journal,
        recovery_history: recovery_history.into_values().collect(),
    }
}

fn validate_exact_name(id: BackupSetId, actual: &str) -> Result<(), ApiError> {
    if actual != canonical_backup_name(id) {
        return Err(ControlPlaneError::InvalidInput("exact_backup_name").into());
    }
    Ok(())
}

fn require_root_cookie(
    context: &crate::middleware::require_session::RequestContext,
) -> Result<&domain::SessionRecord, ApiError> {
    if !context.actor.is_root {
        return Err(ControlPlaneError::PermissionDenied("root_recovery_required").into());
    }
    context.cookie_session().map_err(Into::into)
}

fn require_compatible_digest(
    plan: &RecoveryPlan,
    supplied: &str,
) -> Result<ContentDigest, ApiError> {
    if !plan.is_compatible() {
        return Err(ControlPlaneError::Conflict("recovery_preflight").into());
    }
    let digest = recovery_plan_digest(plan)
        .map_err(|_| ControlPlaneError::Conflict("recovery_plan_digest"))?;
    if digest.as_str() != supplied {
        return Err(ControlPlaneError::Conflict("recovery_plan_changed").into());
    }
    Ok(digest)
}

fn preflight_response(plan: &RecoveryPlan) -> Result<RecoveryPreflightResponse, ApiError> {
    let digest = recovery_plan_digest(plan)
        .map_err(|_| ControlPlaneError::Conflict("recovery_plan_digest"))?;
    Ok(RecoveryPreflightResponse {
        backup_set_id: plan.backup_set_id,
        plan_digest: digest.as_str().to_owned(),
        compatible: plan.is_compatible(),
        required_space_bytes: plan.required_space_bytes,
        available_space_bytes: plan.available_space_bytes,
        impact: serde_json::to_value(&plan.impact)?,
        failures: plan
            .failures
            .iter()
            .map(|failure| format!("{failure:?}").to_ascii_lowercase())
            .collect(),
    })
}

fn mutation_response(id: BackupSetId) -> BackupMutationResponse {
    BackupMutationResponse {
        backup_set_id: id,
        exact_backup_name: canonical_backup_name(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_confirmation_uses_the_canonical_backup_identifier() {
        let id = BackupSetId::new();
        assert!(validate_exact_name(id, &id.as_uuid().to_string()).is_ok());
        assert!(validate_exact_name(id, "daily backup").is_err());
    }
}
