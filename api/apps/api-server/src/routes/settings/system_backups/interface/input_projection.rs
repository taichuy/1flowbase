use super::*;

impl InterfaceContract for SystemBackupsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Catalog"))]),
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[("variant", mp::tag_schema("Create"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Import")),
                (
                    "bytes",
                    mp::object_schema(&[(
                        "byte_count",
                        serde_json::json!({"anyOf": [mp::count_schema(), {"type":"null"}]}),
                    )]),
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
            Self::Catalog => mp::object_value(&[("variant", serde_json::json!("Catalog"))]),
            Self::List => {
                mp::object_value(&[("variant", serde_json::Value::String("List".to_owned()))])
            }
            Self::Create { .. } => {
                mp::object_value(&[("variant", serde_json::Value::String("Create".to_owned()))])
            }
            Self::Import { byte_count, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Import".to_owned())),
                (
                    "bytes",
                    mp::object_value(&[("byte_count", serde_json::json!(byte_count))]),
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
                                serde_json::json!((_field_request).exact_backup_name.len()),
                            )]),
                        ),
                        (
                            "plan_digest",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_request).plan_digest.len()),
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
                                serde_json::json!((_field_request).exact_backup_name.len()),
                            )]),
                        ),
                        (
                            "plan_digest",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_request).plan_digest.len()),
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
