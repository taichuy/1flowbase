use std::sync::Arc;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordVerifier},
};
use control_plane::{errors::ControlPlaneError, system_recovery::ConfirmedRecoveryIntent};
use domain::{BackupSetId, ContentDigest, RecoveryJobId};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use time::OffsetDateTime;
use tokio::io::{AsyncWriteExt, DuplexStream};
use uuid::Uuid;

use super::{
    BackupJobStatusResponse, BackupMutationResponse, BackupSetDetailResponse,
    BackupSetListResponse, BackupSetSummaryResponse, BackupVerificationResponse,
    CreateRecoveryIntentRequest, QueuedBackupResponse, RecoveryIntentResponse,
    RecoveryPreflightResponse, RecoveryReauthRequest, RecoveryReauthResponse,
    RecoveryStatusResponse, canonical_backup_name, detail_response, mutation_response,
    preflight_response, require_compatible_digest, validate_exact_name,
};
use crate::{
    error_response::{ApiError, ApiServiceUnavailable},
    middleware::require_settings_feature_permission::authorize_compiled_console_access,
    recovery_authorization::{
        consume_reauth_challenge, issue_reauth_challenge, recovery_intent_ttl,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    system_backup::SystemBackupRuntime,
};

pub(crate) enum SystemBackupsInput {
    List,
    Create {
        backup_password: Option<String>,
    },
    Import {
        bytes: Vec<u8>,
        backup_password: Option<String>,
    },
    GetJobStatus {
        backup_job_id: Uuid,
    },
    GetDetail {
        backup_set_id: Uuid,
    },
    Delete {
        backup_set_id: Uuid,
    },
    Verify {
        backup_set_id: Uuid,
        backup_password: Option<String>,
    },
    Download {
        backup_set_id: Uuid,
    },
    GetRecoveryStatus {
        recovery_job_id: Option<Uuid>,
    },
    RecoveryPreflight {
        backup_set_id: Uuid,
        backup_password: Option<String>,
    },
    RecoveryReauth {
        request: RecoveryReauthRequest,
    },
    RecoveryIntent {
        backup_set_id: Uuid,
        request: CreateRecoveryIntentRequest,
    },
}

impl InterfaceContract for SystemBackupsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[("variant", mp::tag_schema("Create"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Import")),
                (
                    "bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetJobStatus")),
                ("backup_job_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetDetail")),
                ("backup_set_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("backup_set_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Verify")),
                ("backup_set_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Download")),
                ("backup_set_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRecoveryStatus")),
                (
                    "recovery_job_id",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryPreflight")),
                ("backup_set_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryReauth")),
                (
                    "request",
                    mp::object_schema(&[
                        (
                            "exact_backup_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "plan_digest",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryIntent")),
                ("backup_set_id", mp::text_schema()),
                (
                    "request",
                    mp::object_schema(&[
                        (
                            "exact_backup_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "plan_digest",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List => {
                mp::object_value(&[("variant", serde_json::Value::String("List".to_owned()))])
            }
            Self::Create { .. } => {
                mp::object_value(&[("variant", serde_json::Value::String("Create".to_owned()))])
            }
            Self::Import {
                bytes: _field_bytes,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Import".to_owned())),
                (
                    "bytes",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_bytes).len()))]),
                ),
            ]),
            Self::GetJobStatus {
                backup_job_id: _field_backup_job_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetJobStatus".to_owned()),
                ),
                (
                    "backup_job_id",
                    serde_json::Value::String((_field_backup_job_id).to_string()),
                ),
            ]),
            Self::GetDetail {
                backup_set_id: _field_backup_set_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("GetDetail".to_owned())),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
            ]),
            Self::Delete {
                backup_set_id: _field_backup_set_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
            ]),
            Self::Verify {
                backup_set_id: _field_backup_set_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Verify".to_owned())),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
            ]),
            Self::Download {
                backup_set_id: _field_backup_set_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Download".to_owned())),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
            ]),
            Self::GetRecoveryStatus {
                recovery_job_id: _field_recovery_job_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRecoveryStatus".to_owned()),
                ),
                (
                    "recovery_job_id",
                    match (_field_recovery_job_id).as_ref() {
                        Some(item) => serde_json::Value::String((item).to_string()),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::RecoveryPreflight {
                backup_set_id: _field_backup_set_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RecoveryPreflight".to_owned()),
                ),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
            ]),
            Self::RecoveryReauth {
                request: _field_request,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RecoveryReauth".to_owned()),
                ),
                (
                    "request",
                    mp::object_value(&[
                        (
                            "exact_backup_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_request).exact_backup_name).len()),
                            )]),
                        ),
                        (
                            "plan_digest",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_request).plan_digest).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::RecoveryIntent {
                backup_set_id: _field_backup_set_id,
                request: _field_request,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RecoveryIntent".to_owned()),
                ),
                (
                    "backup_set_id",
                    serde_json::Value::String((_field_backup_set_id).to_string()),
                ),
                (
                    "request",
                    mp::object_value(&[
                        (
                            "exact_backup_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_request).exact_backup_name).len()),
                            )]),
                        ),
                        (
                            "plan_digest",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_request).plan_digest).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-system-backups-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct BackupDownload {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    pub(crate) content_disposition: String,
    pub(crate) reader: DuplexStream,
}

pub(crate) enum SystemBackupsOutput {
    Listed(BackupSetListResponse),
    Created(QueuedBackupResponse),
    Imported(BackupMutationResponse),
    JobStatus(BackupJobStatusResponse),
    Detail(BackupSetDetailResponse),
    Deleted,
    Verified(BackupVerificationResponse),
    Download(BackupDownload),
    RecoveryStatus(RecoveryStatusResponse),
    RecoveryPreflight(RecoveryPreflightResponse),
    RecoveryReauth(RecoveryReauthResponse),
    RecoveryIntent(RecoveryIntentResponse),
}

impl InterfaceContract for SystemBackupsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Listed")),
                (
                    "0",
                    mp::object_schema(&[(
                        "items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("exact_backup_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("availability",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Ready"))]), mp::object_schema(&[("variant",mp::tag_schema("Corrupt"))]), mp::object_schema(&[("variant",mp::tag_schema("Incompatible"))])])), ("total_size_bytes",serde_json::json!({"type":"integer"})), ("envelope_digest",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Created"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Imported")),
                (
                    "0",
                    mp::object_schema(&[(
                        "exact_backup_name",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("JobStatus")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "status",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Fencing"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Capturing"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Sealing"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Verifying"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                            ]),
                        ),
                        (
                            "failure_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("sealed_components", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Detail")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "exact_backup_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("created_at", mp::text_schema()),
                        (
                            "content",
                            mp::object_schema(&[
                                ("component_count", serde_json::json!({"type":"integer"})),
                                ("postgresql_count", serde_json::json!({"type":"integer"})),
                                (
                                    "business_object_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "extension_artifact_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                ("mcp_artifact_count", serde_json::json!({"type":"integer"})),
                                (
                                    "embedded_component_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "identity_only_component_count",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                ("total_size_bytes", serde_json::json!({"type":"integer"})),
                                (
                                    "excluded_domains",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "components",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("component_id",mp::text_schema()), ("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("PostgreSql"))]), mp::object_schema(&[("variant",mp::tag_schema("BusinessObject"))]), mp::object_schema(&[("variant",mp::tag_schema("ExtensionArtifact"))]), mp::object_schema(&[("variant",mp::tag_schema("McpArtifact"))])])), ("source_identity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("content_type",mp::text_schema()), ("size_bytes",serde_json::json!({"type":"integer"})), ("content_digest",mp::object_schema(&[("byte_count",mp::count_schema())])), ("disposition",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Embedded"))]), mp::object_schema(&[("variant",mp::tag_schema("IdentityOnly"))])])), ("rebuildability",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Rebuildable"))]), mp::object_schema(&[("variant",mp::tag_schema("NonRebuildable"))]), mp::object_schema(&[("variant",mp::tag_schema("NotApplicable"))])])), ("restore_target",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("PostgreSql"))]), mp::object_schema(&[("variant",mp::tag_schema("BusinessObject")), ("storage_id",mp::text_schema()), ("object_path",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("Artifact")), ("category",mp::object_schema(&[("byte_count",mp::count_schema())])), ("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_id",mp::text_schema()), ("version",mp::text_schema())])]))])}),
                        ),
                        (
                            "compatibility",
                            mp::object_schema(&[
                                ("compatible", serde_json::json!({"type":"boolean"})),
                                (
                                    "failures",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                ("format_version", serde_json::json!({"type":"integer"})),
                                (
                                    "application_build",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "migration_head",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "verification",
                            mp::object_schema(&[
                                (
                                    "verified",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                                ),
                                (
                                    "checked_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "creation_journal",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("sequence",serde_json::json!({"type":"integer"})), ("occurred_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("state",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Queued"))]), mp::object_schema(&[("variant",mp::tag_schema("Fencing"))]), mp::object_schema(&[("variant",mp::tag_schema("Capturing"))]), mp::object_schema(&[("variant",mp::tag_schema("Sealing"))]), mp::object_schema(&[("variant",mp::tag_schema("Verifying"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])]), {"type":"null"}]})), ("component_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("failure_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                        ),
                        (
                            "recovery_history",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("status",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Preflight"))]), mp::object_schema(&[("variant",mp::tag_schema("AwaitingConfirmation"))]), mp::object_schema(&[("variant",mp::tag_schema("SafetyBackup"))]), mp::object_schema(&[("variant",mp::tag_schema("Fencing"))]), mp::object_schema(&[("variant",mp::tag_schema("Draining"))]), mp::object_schema(&[("variant",mp::tag_schema("Restoring"))]), mp::object_schema(&[("variant",mp::tag_schema("Reconciling"))]), mp::object_schema(&[("variant",mp::tag_schema("Verifying"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("RolledBack"))]), mp::object_schema(&[("variant",mp::tag_schema("ManualRecoveryRequired"))])]), {"type":"null"}]})), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_at",mp::text_schema()), ("failure_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Verified")),
                (
                    "0",
                    mp::object_schema(&[("verified", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Download")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", serde_json::json!({"type":"integer"})),
                        ("content_type", mp::text_schema()),
                        (
                            "content_disposition",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryStatus")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "phase",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("active_write_count", serde_json::json!({"type":"integer"})),
                        (
                            "started_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "plan_digest",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "journal_state",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Preflight"))]), mp::object_schema(&[("variant",mp::tag_schema("AwaitingConfirmation"))]), mp::object_schema(&[("variant",mp::tag_schema("SafetyBackup"))]), mp::object_schema(&[("variant",mp::tag_schema("Fencing"))]), mp::object_schema(&[("variant",mp::tag_schema("Draining"))]), mp::object_schema(&[("variant",mp::tag_schema("Restoring"))]), mp::object_schema(&[("variant",mp::tag_schema("Reconciling"))]), mp::object_schema(&[("variant",mp::tag_schema("Verifying"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("RolledBack"))]), mp::object_schema(&[("variant",mp::tag_schema("ManualRecoveryRequired"))])]), {"type":"null"}]}),
                        ),
                        (
                            "journal_events",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryPreflight")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "plan_digest",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("compatible", serde_json::json!({"type":"boolean"})),
                        (
                            "required_space_bytes",
                            serde_json::json!({"type":"integer"}),
                        ),
                        (
                            "available_space_bytes",
                            serde_json::json!({"type":"integer"}),
                        ),
                        ("impact", mp::json_summary_schema()),
                        (
                            "failures",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryReauth")),
                ("0", mp::object_schema(&[("expires_at", mp::text_schema())])),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RecoveryIntent")),
                (
                    "0",
                    mp::object_schema(&[
                        ("intent_id", mp::text_schema()),
                        ("status", mp::text_schema()),
                        ("expires_at", mp::text_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Listed(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Listed".to_owned())), ("0",mp::object_value(&[("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("exact_backup_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).exact_backup_name).len()))])), ("created_at",mp::text(&(item).created_at)?), ("availability",match &(item).availability {domain::system_backup::BackupSetAvailability::Ready => mp::object_value(&[("variant",serde_json::Value::String("Ready".to_owned()))]), domain::system_backup::BackupSetAvailability::Corrupt => mp::object_value(&[("variant",serde_json::Value::String("Corrupt".to_owned()))]), domain::system_backup::BackupSetAvailability::Incompatible => mp::object_value(&[("variant",serde_json::Value::String("Incompatible".to_owned()))])}), ("total_size_bytes",serde_json::json!(*(&(item).total_size_bytes))), ("envelope_digest",match (&(item).envelope_digest).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Created(_) => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned()))]), Self::Imported(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Imported".to_owned())), ("0",mp::object_value(&[("exact_backup_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).exact_backup_name).len()))]))]))]), Self::JobStatus(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("JobStatus".to_owned())), ("0",mp::object_value(&[("status",match &(_field_0).status {domain::system_backup::BackupJobState::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), domain::system_backup::BackupJobState::Fencing => mp::object_value(&[("variant",serde_json::Value::String("Fencing".to_owned()))]), domain::system_backup::BackupJobState::Capturing => mp::object_value(&[("variant",serde_json::Value::String("Capturing".to_owned()))]), domain::system_backup::BackupJobState::Sealing => mp::object_value(&[("variant",serde_json::Value::String("Sealing".to_owned()))]), domain::system_backup::BackupJobState::Verifying => mp::object_value(&[("variant",serde_json::Value::String("Verifying".to_owned()))]), domain::system_backup::BackupJobState::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::system_backup::BackupJobState::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("failure_code",match (&(_field_0).failure_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("sealed_components",serde_json::json!(*(&(_field_0).sealed_components)))]))]), Self::Detail(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Detail".to_owned())), ("0",mp::object_value(&[("exact_backup_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).exact_backup_name).len()))])), ("created_at",mp::text(&(_field_0).created_at)?), ("content",mp::object_value(&[("component_count",serde_json::json!(*(&(&(_field_0).content).component_count))), ("postgresql_count",serde_json::json!(*(&(&(_field_0).content).postgresql_count))), ("business_object_count",serde_json::json!(*(&(&(_field_0).content).business_object_count))), ("extension_artifact_count",serde_json::json!(*(&(&(_field_0).content).extension_artifact_count))), ("mcp_artifact_count",serde_json::json!(*(&(&(_field_0).content).mcp_artifact_count))), ("embedded_component_count",serde_json::json!(*(&(&(_field_0).content).embedded_component_count))), ("identity_only_component_count",serde_json::json!(*(&(&(_field_0).content).identity_only_component_count))), ("total_size_bytes",serde_json::json!(*(&(&(_field_0).content).total_size_bytes))), ("excluded_domains",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).content).excluded_domains).len()))]))])), ("components",{ if (&(_field_0).components).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).components).iter().map(|item| Some(mp::object_value(&[("component_id",mp::text(&(item).component_id)?), ("kind",match &(item).kind {domain::system_backup::BackupComponentKind::PostgreSql => mp::object_value(&[("variant",serde_json::Value::String("PostgreSql".to_owned()))]), domain::system_backup::BackupComponentKind::BusinessObject => mp::object_value(&[("variant",serde_json::Value::String("BusinessObject".to_owned()))]), domain::system_backup::BackupComponentKind::ExtensionArtifact => mp::object_value(&[("variant",serde_json::Value::String("ExtensionArtifact".to_owned()))]), domain::system_backup::BackupComponentKind::McpArtifact => mp::object_value(&[("variant",serde_json::Value::String("McpArtifact".to_owned()))])}), ("source_identity",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_identity).len()))])), ("content_type",mp::text(&(item).content_type)?), ("size_bytes",serde_json::json!(*(&(item).size_bytes))), ("content_digest",mp::object_value(&[("byte_count",serde_json::json!((&(item).content_digest).len()))])), ("disposition",match &(item).disposition {domain::system_backup::BackupComponentDisposition::Embedded => mp::object_value(&[("variant",serde_json::Value::String("Embedded".to_owned()))]), domain::system_backup::BackupComponentDisposition::IdentityOnly => mp::object_value(&[("variant",serde_json::Value::String("IdentityOnly".to_owned()))])}), ("rebuildability",match &(item).rebuildability {domain::system_backup::ArtifactRebuildability::Rebuildable => mp::object_value(&[("variant",serde_json::Value::String("Rebuildable".to_owned()))]), domain::system_backup::ArtifactRebuildability::NonRebuildable => mp::object_value(&[("variant",serde_json::Value::String("NonRebuildable".to_owned()))]), domain::system_backup::ArtifactRebuildability::NotApplicable => mp::object_value(&[("variant",serde_json::Value::String("NotApplicable".to_owned()))])}), ("restore_target",match &(item).restore_target {domain::system_backup::BackupComponentRestoreTarget::PostgreSql => mp::object_value(&[("variant",serde_json::Value::String("PostgreSql".to_owned()))]), domain::system_backup::BackupComponentRestoreTarget::BusinessObject {storage_id: _field_storage_id, object_path: _field_object_path, .. } => mp::object_value(&[("variant",serde_json::Value::String("BusinessObject".to_owned())), ("storage_id",serde_json::Value::String((_field_storage_id).to_string())), ("object_path",mp::object_value(&[("byte_count",serde_json::json!((_field_object_path).len()))]))]), domain::system_backup::BackupComponentRestoreTarget::Artifact {category: _field_category, organization: _field_organization, artifact_id: _field_artifact_id, version: _field_version, .. } => mp::object_value(&[("variant",serde_json::Value::String("Artifact".to_owned())), ("category",mp::object_value(&[("byte_count",serde_json::json!((_field_category).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("artifact_id",mp::text(_field_artifact_id)?), ("version",mp::text(_field_version)?)])})]))).collect::<Option<Vec<_>>>()?) }), ("compatibility",mp::object_value(&[("compatible",serde_json::Value::Bool(*(&(&(_field_0).compatibility).compatible))), ("failures",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).compatibility).failures).len()))])), ("format_version",serde_json::json!(*(&(&(_field_0).compatibility).format_version))), ("application_build",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).compatibility).application_build).len()))])), ("migration_head",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).compatibility).migration_head).len()))]))])), ("verification",mp::object_value(&[("verified",match (&(&(_field_0).verification).verified).as_ref() { Some(item) => serde_json::Value::Bool(*(item)), None => serde_json::Value::Null }), ("checked_at",match (&(&(_field_0).verification).checked_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("creation_journal",{ if (&(_field_0).creation_journal).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).creation_journal).iter().map(|item| Some(mp::object_value(&[("sequence",serde_json::json!(*(&(item).sequence))), ("occurred_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).occurred_at).len()))])), ("state",match (&(item).state).as_ref() { Some(item) => match item {domain::system_backup::BackupJobState::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), domain::system_backup::BackupJobState::Fencing => mp::object_value(&[("variant",serde_json::Value::String("Fencing".to_owned()))]), domain::system_backup::BackupJobState::Capturing => mp::object_value(&[("variant",serde_json::Value::String("Capturing".to_owned()))]), domain::system_backup::BackupJobState::Sealing => mp::object_value(&[("variant",serde_json::Value::String("Sealing".to_owned()))]), domain::system_backup::BackupJobState::Verifying => mp::object_value(&[("variant",serde_json::Value::String("Verifying".to_owned()))]), domain::system_backup::BackupJobState::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::system_backup::BackupJobState::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}, None => serde_json::Value::Null }), ("component_id",match (&(item).component_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("failure_code",match (&(item).failure_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("recovery_history",{ if (&(_field_0).recovery_history).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).recovery_history).iter().map(|item| Some(mp::object_value(&[("status",match (&(item).status).as_ref() { Some(item) => match item {domain::system_backup::RecoveryJobState::Preflight => mp::object_value(&[("variant",serde_json::Value::String("Preflight".to_owned()))]), domain::system_backup::RecoveryJobState::AwaitingConfirmation => mp::object_value(&[("variant",serde_json::Value::String("AwaitingConfirmation".to_owned()))]), domain::system_backup::RecoveryJobState::SafetyBackup => mp::object_value(&[("variant",serde_json::Value::String("SafetyBackup".to_owned()))]), domain::system_backup::RecoveryJobState::Fencing => mp::object_value(&[("variant",serde_json::Value::String("Fencing".to_owned()))]), domain::system_backup::RecoveryJobState::Draining => mp::object_value(&[("variant",serde_json::Value::String("Draining".to_owned()))]), domain::system_backup::RecoveryJobState::Restoring => mp::object_value(&[("variant",serde_json::Value::String("Restoring".to_owned()))]), domain::system_backup::RecoveryJobState::Reconciling => mp::object_value(&[("variant",serde_json::Value::String("Reconciling".to_owned()))]), domain::system_backup::RecoveryJobState::Verifying => mp::object_value(&[("variant",serde_json::Value::String("Verifying".to_owned()))]), domain::system_backup::RecoveryJobState::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::system_backup::RecoveryJobState::RolledBack => mp::object_value(&[("variant",serde_json::Value::String("RolledBack".to_owned()))]), domain::system_backup::RecoveryJobState::ManualRecoveryRequired => mp::object_value(&[("variant",serde_json::Value::String("ManualRecoveryRequired".to_owned()))])}, None => serde_json::Value::Null }), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("updated_at",mp::text(&(item).updated_at)?), ("failure_code",match (&(item).failure_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Deleted => mp::object_value(&[("variant",serde_json::Value::String("Deleted".to_owned()))]), Self::Verified(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Verified".to_owned())), ("0",mp::object_value(&[("verified",serde_json::Value::Bool(*(&(_field_0).verified)))]))]), Self::Download(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Download".to_owned())), ("0",mp::object_value(&[("status",serde_json::json!(*(&(_field_0).status))), ("content_type",mp::text(&(_field_0).content_type)?), ("content_disposition",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).content_disposition).len()))]))]))]), Self::RecoveryStatus(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RecoveryStatus".to_owned())), ("0",mp::object_value(&[("phase",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).phase).len()))])), ("active_write_count",serde_json::json!(*(&(_field_0).active_write_count))), ("started_at",match (&(_field_0).started_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("plan_digest",match (&(_field_0).plan_digest).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("journal_state",match (&(_field_0).journal_state).as_ref() { Some(item) => match item {domain::system_backup::RecoveryJobState::Preflight => mp::object_value(&[("variant",serde_json::Value::String("Preflight".to_owned()))]), domain::system_backup::RecoveryJobState::AwaitingConfirmation => mp::object_value(&[("variant",serde_json::Value::String("AwaitingConfirmation".to_owned()))]), domain::system_backup::RecoveryJobState::SafetyBackup => mp::object_value(&[("variant",serde_json::Value::String("SafetyBackup".to_owned()))]), domain::system_backup::RecoveryJobState::Fencing => mp::object_value(&[("variant",serde_json::Value::String("Fencing".to_owned()))]), domain::system_backup::RecoveryJobState::Draining => mp::object_value(&[("variant",serde_json::Value::String("Draining".to_owned()))]), domain::system_backup::RecoveryJobState::Restoring => mp::object_value(&[("variant",serde_json::Value::String("Restoring".to_owned()))]), domain::system_backup::RecoveryJobState::Reconciling => mp::object_value(&[("variant",serde_json::Value::String("Reconciling".to_owned()))]), domain::system_backup::RecoveryJobState::Verifying => mp::object_value(&[("variant",serde_json::Value::String("Verifying".to_owned()))]), domain::system_backup::RecoveryJobState::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), domain::system_backup::RecoveryJobState::RolledBack => mp::object_value(&[("variant",serde_json::Value::String("RolledBack".to_owned()))]), domain::system_backup::RecoveryJobState::ManualRecoveryRequired => mp::object_value(&[("variant",serde_json::Value::String("ManualRecoveryRequired".to_owned()))])}, None => serde_json::Value::Null }), ("journal_events",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).journal_events).len()))]))]))]), Self::RecoveryPreflight(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RecoveryPreflight".to_owned())), ("0",mp::object_value(&[("plan_digest",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).plan_digest).len()))])), ("compatible",serde_json::Value::Bool(*(&(_field_0).compatible))), ("required_space_bytes",serde_json::json!(*(&(_field_0).required_space_bytes))), ("available_space_bytes",serde_json::json!(*(&(_field_0).available_space_bytes))), ("impact",mp::json_summary(&(_field_0).impact)), ("failures",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).failures).len()))]))]))]), Self::RecoveryReauth(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RecoveryReauth".to_owned())), ("0",mp::object_value(&[("expires_at",mp::text(&(_field_0).expires_at)?)]))]), Self::RecoveryIntent(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RecoveryIntent".to_owned())), ("0",mp::object_value(&[("intent_id",serde_json::Value::String((&(_field_0).intent_id).to_string())), ("status",mp::text(&(_field_0).status)?), ("expires_at",mp::text(&(_field_0).expires_at)?)]))])})
    }

    const CONTRACT_ID: &'static str = "console-system-backups-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct SystemBackupsDependencies {
    pub(crate) runtime: Option<Arc<SystemBackupRuntime>>,
    pub(crate) store: MainDurableStore,
    pub(crate) backup_status_access: BackupStatusAccess,
}

#[derive(Clone)]
pub(crate) struct BackupStatusAccess {
    pub(crate) operation_id: String,
    pub(crate) policy_group: access_control::ConsolePolicyGroup,
    pub(crate) authorization: access_control::ConsoleAuthorization,
    pub(crate) resource_access: Option<access_control::ResourceAccessRegistration>,
}

struct SystemBackupsAdapter(SystemBackupsDependencies);

pub(crate) fn port(
    dependencies: SystemBackupsDependencies,
) -> Arc<dyn ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>> {
    Arc::new(SystemBackupsAdapter(dependencies))
}

impl SystemBackupsAdapter {
    fn runtime(&self) -> Result<Arc<SystemBackupRuntime>, ApiError> {
        self.0
            .runtime
            .clone()
            .ok_or_else(|| ApiServiceUnavailable("system_backup_unavailable").into())
    }

    fn require_root_cookie(principal: &UserPrincipal) -> Result<(), ApiError> {
        if !principal.actor().is_root {
            return Err(ControlPlaneError::PermissionDenied("root_recovery_required").into());
        }
        if principal.authenticated_session().is_none() {
            return Err(ControlPlaneError::PermissionDenied("cookie_session_required").into());
        }
        Ok(())
    }

    async fn authorize_backup_status(&self, principal: &UserPrincipal) -> Result<(), ApiError> {
        let actor = principal.actor();
        if actor.is_root
            || matches!(
                self.0.backup_status_access.authorization,
                access_control::ConsoleAuthorization::Authenticated
            )
        {
            return Ok(());
        }
        let policies = self
            .0
            .store
            .load_console_policy_for_bound_role(
                actor.user_id,
                actor.current_workspace_id,
                &actor.effective_display_role,
            )
            .await?;
        let access = access_control::ConsoleRouteAccess {
            operation_id: &self.0.backup_status_access.operation_id,
            policy_group: &self.0.backup_status_access.policy_group,
            authorization: &self.0.backup_status_access.authorization,
            resource_access: self.0.backup_status_access.resource_access.as_ref(),
        };
        if !authorize_compiled_console_access(&access, actor, &policies) {
            return Err(
                ControlPlaneError::PermissionDenied("console_operation_permission_denied").into(),
            );
        }
        Ok(())
    }

    async fn recovery_reauth(
        &self,
        principal: &UserPrincipal,
        body: RecoveryReauthRequest,
    ) -> Result<RecoveryReauthResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        validate_exact_name(body.backup_set_id, &body.exact_backup_name)?;
        let plan = self
            .runtime()?
            .preflight_with_password(body.backup_set_id, body.backup_password.as_deref())
            .await;
        require_compatible_digest(&plan, &body.plan_digest)?;
        let user = self
            .0
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(ControlPlaneError::PermissionDenied(
                "recovery_reauth_failed",
            ))?;
        let parsed = PasswordHash::new(&user.password_hash)
            .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
        Argon2::default()
            .verify_password(body.password.as_bytes(), &parsed)
            .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
        let digest = ContentDigest::try_from(body.plan_digest)
            .map_err(|_| ControlPlaneError::InvalidInput("plan_digest"))?;
        let session = principal
            .authenticated_session()
            .expect("root cookie checked");
        let challenge = issue_reauth_challenge(
            principal.actor().user_id,
            session.expose_to_trusted_handler(),
            body.backup_set_id,
            digest,
            &body.exact_backup_name,
        );
        Ok(RecoveryReauthResponse {
            challenge_token: challenge.token,
            expires_at: challenge.expires_at.to_string(),
        })
    }

    async fn recovery_intent(
        &self,
        principal: &UserPrincipal,
        backup_set_id: Uuid,
        body: CreateRecoveryIntentRequest,
    ) -> Result<RecoveryIntentResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        let backup_set_id = BackupSetId::from_uuid(backup_set_id);
        validate_exact_name(backup_set_id, &body.exact_backup_name)?;
        let runtime = self.runtime()?;
        let plan = runtime
            .preflight_with_password(backup_set_id, body.backup_password.as_deref())
            .await;
        let plan_digest = require_compatible_digest(&plan, &body.plan_digest)?;
        let session = principal
            .authenticated_session()
            .expect("root cookie checked");
        consume_reauth_challenge(
            body.challenge_token,
            principal.actor().user_id,
            session.expose_to_trusted_handler(),
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
            principal.actor().user_id,
            backup_set_id,
            plan_digest,
            now,
            expires_at,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("recovery_intent"))?;
        let lease = runtime.reserve_recovery_maintenance(recovery_job_id)?;
        let target_backup_password = body.backup_password;
        tokio::spawn(async move {
            if let Err(error) = runtime
                .prepare_recovery_with_maintenance_lease(confirmed, target_backup_password, lease)
                .await
            {
                tracing::error!(intent_id = %intent_id, recovery_job_id = %recovery_job_id.as_uuid(), error = %error, "recovery handoff preparation failed");
            }
        });
        Ok(RecoveryIntentResponse {
            intent_id,
            recovery_job_id,
            backup_set_id,
            status: "preparing".to_owned(),
            expires_at: expires_at.to_string(),
        })
    }

    async fn import(
        &self,
        bytes: Vec<u8>,
        backup_password: Option<String>,
    ) -> Result<BackupMutationResponse, ApiError> {
        let runtime = self.runtime()?;
        let capacity = bytes.len().clamp(1, 256 * 1024);
        let (mut reader, mut writer) = tokio::io::duplex(capacity);
        tokio::spawn(async move {
            let _ = writer.write_all(&bytes).await;
            let _ = writer.shutdown().await;
        });
        let sealed = runtime
            .import_with_password(&mut reader, backup_password.as_deref())
            .await?;
        Ok(mutation_response(sealed.manifest().backup_set_id()))
    }

    async fn download(&self, backup_set_id: Uuid) -> Result<BackupDownload, ApiError> {
        let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
        let runtime = self.runtime()?;
        runtime.get(backup_set_id).await?;
        let (reader, writer) = tokio::io::duplex(256 * 1024);
        tokio::spawn(async move {
            if let Err(error) = runtime.download(backup_set_id, writer).await {
                tracing::warn!(backup_set_id = %backup_set_id.as_uuid(), error = %error, "backup download stream failed");
            }
        });
        Ok(BackupDownload {
            status: 200,
            content_type: "application/octet-stream",
            content_disposition: format!(
                "attachment; filename=\"{}.1fb-backup\"",
                backup_set_id.as_uuid()
            ),
            reader,
        })
    }

    async fn recovery_status(
        &self,
        principal: &UserPrincipal,
        recovery_job_id: Option<Uuid>,
    ) -> Result<RecoveryStatusResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        let runtime = self.runtime()?;
        let maintenance = runtime.maintenance_status();
        let active = runtime.active_recovery();
        let requested_job_id = recovery_job_id
            .map(domain::RecoveryJobId::from_uuid)
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
        Ok(RecoveryStatusResponse {
            phase: phase.to_owned(),
            recovery_job_id: requested_job_id,
            active_write_count: maintenance.active_write_count() as u64,
            started_at: maintenance.started_at.map(|value| value.to_string()),
            target_backup_set_id: active.as_ref().map(|value| value.target_backup_set_id),
            safety_backup_set_id: active.as_ref().map(|value| value.safety_backup_set_id),
            plan_digest: active.map(|value| value.plan_digest.as_str().to_owned()),
            journal_state,
            journal_events,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: SystemBackupsInput,
    ) -> Result<SystemBackupsOutput, ApiError> {
        match input {
            SystemBackupsInput::List => {
                let items = self
                    .runtime()?
                    .list()
                    .await?
                    .into_iter()
                    .map(|entry| BackupSetSummaryResponse {
                        exact_backup_name: canonical_backup_name(entry.backup_set_id),
                        backup_set_id: entry.backup_set_id,
                        created_at: entry.created_at.to_string(),
                        availability: entry.availability,
                        total_size_bytes: entry.total_size_bytes,
                        envelope_digest: entry
                            .envelope_digest
                            .map(|value| value.as_str().to_owned()),
                    })
                    .collect();
                Ok(SystemBackupsOutput::Listed(BackupSetListResponse { items }))
            }
            SystemBackupsInput::Create { backup_password } => {
                self.authorize_backup_status(principal).await?;
                let queued = self
                    .runtime()?
                    .queue_manual_backup(principal.actor().user_id, backup_password)
                    .await?;
                Ok(SystemBackupsOutput::Created(QueuedBackupResponse {
                    backup_job_id: queued.backup_job_id,
                    backup_set_id: queued.backup_set_id,
                }))
            }
            SystemBackupsInput::Import {
                bytes,
                backup_password,
            } => Ok(SystemBackupsOutput::Imported(
                self.import(bytes, backup_password).await?,
            )),
            SystemBackupsInput::GetJobStatus { backup_job_id } => {
                let status = self
                    .runtime()?
                    .backup_job_status(domain::BackupJobId::from_uuid(backup_job_id))
                    .await?
                    .ok_or(ControlPlaneError::NotFound("backup_job"))?;
                Ok(SystemBackupsOutput::JobStatus(BackupJobStatusResponse {
                    backup_job_id: status.backup_job_id,
                    backup_set_id: status.backup_set_id,
                    status: status.state,
                    failure_code: status.failure_code,
                    sealed_components: status.sealed_components,
                }))
            }
            SystemBackupsInput::GetDetail { backup_set_id } => {
                let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
                let detail = self.runtime()?.detail(backup_set_id).await?;
                Ok(SystemBackupsOutput::Detail(detail_response(
                    backup_set_id,
                    detail,
                )))
            }
            SystemBackupsInput::Delete { backup_set_id } => {
                self.runtime()?
                    .delete(domain::BackupSetId::from_uuid(backup_set_id))
                    .await?;
                Ok(SystemBackupsOutput::Deleted)
            }
            SystemBackupsInput::Verify {
                backup_set_id,
                backup_password,
            } => {
                let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
                self.runtime()?
                    .verify_with_password(backup_set_id, backup_password.as_deref())
                    .await?;
                Ok(SystemBackupsOutput::Verified(BackupVerificationResponse {
                    backup_set_id,
                    verified: true,
                }))
            }
            SystemBackupsInput::Download { backup_set_id } => Ok(SystemBackupsOutput::Download(
                self.download(backup_set_id).await?,
            )),
            SystemBackupsInput::GetRecoveryStatus { recovery_job_id } => {
                Ok(SystemBackupsOutput::RecoveryStatus(
                    self.recovery_status(principal, recovery_job_id).await?,
                ))
            }
            SystemBackupsInput::RecoveryPreflight {
                backup_set_id,
                backup_password,
            } => {
                Self::require_root_cookie(principal)?;
                let plan = self
                    .runtime()?
                    .preflight_with_password(
                        BackupSetId::from_uuid(backup_set_id),
                        backup_password.as_deref(),
                    )
                    .await;
                Ok(SystemBackupsOutput::RecoveryPreflight(preflight_response(
                    &plan,
                )?))
            }
            SystemBackupsInput::RecoveryReauth { request } => {
                Ok(SystemBackupsOutput::RecoveryReauth(
                    self.recovery_reauth(principal, request).await?,
                ))
            }
            SystemBackupsInput::RecoveryIntent {
                backup_set_id,
                request,
            } => Ok(SystemBackupsOutput::RecoveryIntent(
                self.recovery_intent(principal, backup_set_id, request)
                    .await?,
            )),
        }
    }
}

impl ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput> for SystemBackupsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: SystemBackupsInput,
    ) -> ConsoleInterfaceFuture<'a, SystemBackupsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.list",
        binding_id: "http.console.settings.system-backups.list.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.create",
        binding_id: "http.console.settings.system-backups.create.v1",
        method: "POST",
        path: "/api/console/settings/system-backups",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.import",
        binding_id: "http.console.settings.system-backups.import.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/import",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.status",
        binding_id: "http.console.settings.system-backups.recovery-status.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/recovery/status",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.status",
        binding_id: "http.console.settings.system-backups.job-status.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/jobs/status/:backup_job_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.detail",
        binding_id: "http.console.settings.system-backups.detail.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/:backup_set_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.delete",
        binding_id: "http.console.settings.system-backups.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/system-backups/:backup_set_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.verify",
        binding_id: "http.console.settings.system-backups.verify.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/verify",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.download",
        binding_id: "http.console.settings.system-backups.download.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/:backup_set_id/download",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.preflight",
        binding_id: "http.console.settings.system-backups.recovery-preflight.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/recovery/preflight",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.reauth",
        binding_id: "http.console.settings.system-backups.recovery-reauth.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/recovery/reauth",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.intent",
        binding_id: "http.console.settings.system-backups.recovery-intent.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/recovery/intents",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-system-backups",
        "graph:console-system-backups-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableSystemBackupsPort;

#[cfg(test)]
impl ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>
    for UnavailableSystemBackupsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: SystemBackupsInput,
    ) -> ConsoleInterfaceFuture<'a, SystemBackupsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("system backup fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f13c_registry_freezes_system_backup_bindings() {
        let registry = compile_registry(Arc::new(UnavailableSystemBackupsPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared system backup binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
        for operation in [
            "system_backups.list",
            "system_backups.create",
            "system_backups.recovery.preflight",
            "system_backups.recovery.reauth",
            "system_backups.recovery.intent",
        ] {
            assert!(
                DECLARATIONS
                    .iter()
                    .any(|declaration| declaration.interface_id == operation)
            );
        }
    }
}
