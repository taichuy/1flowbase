use anyhow::{anyhow, Result};
use domain::{ApplicationSections, ApplicationType, FlowChangeKind, FlowVersionTrigger};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredFlowRow {
    pub id: Uuid,
    pub application_id: Uuid,
    pub created_by: Uuid,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredFlowDraftRow {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub schema_version: String,
    pub document: serde_json::Value,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredFlowVersionRow {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub sequence: i64,
    pub trigger: String,
    pub change_kind: String,
    pub summary: String,
    pub summary_is_custom: bool,
    pub is_user_protected: bool,
    pub is_current_publication: bool,
    pub document: serde_json::Value,
    pub created_at: OffsetDateTime,
}

pub struct PgFlowMapper;

impl PgFlowMapper {
    pub fn to_flow_record(row: StoredFlowRow) -> domain::FlowRecord {
        domain::FlowRecord {
            id: row.id,
            application_id: row.application_id,
            created_by: row.created_by,
            updated_at: row.updated_at,
        }
    }

    pub fn to_flow_draft_record(row: StoredFlowDraftRow) -> domain::FlowDraftRecord {
        domain::FlowDraftRecord {
            id: row.id,
            flow_id: row.flow_id,
            schema_version: row.schema_version,
            document: row.document,
            updated_at: row.updated_at,
        }
    }

    pub fn to_flow_version_record(row: StoredFlowVersionRow) -> Result<domain::FlowVersionRecord> {
        Ok(domain::FlowVersionRecord {
            id: row.id,
            flow_id: row.flow_id,
            sequence: row.sequence,
            trigger: parse_flow_version_trigger(&row.trigger)?,
            change_kind: parse_flow_change_kind(&row.change_kind)?,
            summary: row.summary,
            summary_is_custom: row.summary_is_custom,
            is_user_protected: row.is_user_protected,
            is_current_publication: row.is_current_publication,
            document: row.document,
            created_at: row.created_at,
        })
    }
}

pub fn parse_flow_version_trigger(value: &str) -> Result<FlowVersionTrigger> {
    match value {
        "autosave" => Ok(FlowVersionTrigger::Autosave),
        "restore" => Ok(FlowVersionTrigger::Restore),
        _ => Err(anyhow!("unknown flow version trigger: {value}")),
    }
}

pub fn parse_flow_change_kind(value: &str) -> Result<FlowChangeKind> {
    match value {
        "layout" => Ok(FlowChangeKind::Layout),
        "logical" => Ok(FlowChangeKind::Logical),
        _ => Err(anyhow!("unknown flow change kind: {value}")),
    }
}

pub fn flow_sections(
    application_type: ApplicationType,
    current_flow_id: Option<Uuid>,
    current_draft_id: Option<Uuid>,
) -> ApplicationSections {
    let ready = current_flow_id.is_some() && current_draft_id.is_some();

    ApplicationSections {
        orchestration: domain::ApplicationOrchestrationSection {
            status: if ready {
                "ready".into()
            } else {
                "planned".into()
            },
            subject_kind: application_type.as_str().into(),
            subject_status: if ready {
                "editable".into()
            } else {
                "unconfigured".into()
            },
            current_subject_id: current_flow_id,
            current_draft_id,
        },
        api: domain::ApplicationApiSection {
            status: "planned".into(),
            credential_kind: "application_api_key".into(),
            invoke_routing_mode: "api_key_bound_application".into(),
            invoke_path_template: None,
            api_capability_status: "planned".into(),
            credentials_status: "planned".into(),
        },
        logs: domain::ApplicationLogsSection {
            status: if ready {
                "ready".into()
            } else {
                "planned".into()
            },
            runs_capability_status: if ready {
                "queryable".into()
            } else {
                "planned".into()
            },
            run_object_kind: "application_run".into(),
            log_retention_status: "planned".into(),
        },
        monitoring: domain::ApplicationMonitoringSection {
            status: "planned".into(),
            metrics_capability_status: "planned".into(),
            metrics_object_kind: "application_metrics".into(),
            tracing_config_status: "planned".into(),
        },
    }
}
