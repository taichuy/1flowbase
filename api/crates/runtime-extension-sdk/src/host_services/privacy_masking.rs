use crate::{MultiplexEmitter, MultiplexError, MultiplexHostService};
use extension_contracts::*;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum PrivacyMaskingClientError {
    #[error(transparent)]
    Transport(#[from] MultiplexError),
    #[error(transparent)]
    Host(#[from] HostServiceError),
    #[error("invalid privacy masking response")]
    InvalidResponse,
}

#[derive(Clone)]
pub struct PrivacyMaskingClient(MultiplexEmitter);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    result: Option<PrivacyMaskingResponse>,
    error: Option<HostServiceError>,
}

type Result<T> = std::result::Result<T, PrivacyMaskingClientError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyMaskingSubjectsPage {
    pub model_provider_instances: Vec<PrivacyMaskingModelInstanceSubjectDto>,
    pub agent_flows: Vec<PrivacyMaskingAgentFlowSubjectDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyMaskingSyncReceipt {
    pub policy_version: i64,
    pub enabled_rule_count: u32,
    pub active_subject_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyMaskingPreviewResult {
    pub masked_text: String,
    pub restored_text: Option<String>,
    pub hits: Vec<PrivacyMaskingPreviewHitDto>,
    pub total_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyMaskingLogsPage {
    pub items: Vec<PrivacyMaskingLogEntryDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivacyMaskingLogsQuery {
    pub subject_kind: Option<PrivacyMaskingSubjectKind>,
    pub subject_id: Option<String>,
    pub placeholder_kind: Option<String>,
    pub rule_id: Option<String>,
    pub flow_run_id: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

impl PrivacyMaskingClient {
    pub fn new(emitter: MultiplexEmitter) -> Self {
        Self(emitter)
    }

    pub async fn execute(&self, request: PrivacyMaskingRequest) -> Result<PrivacyMaskingResponse> {
        let request = serde_json::to_value(request)
            .map_err(|_| PrivacyMaskingClientError::InvalidResponse)?;
        let envelope = self
            .0
            .callback(MultiplexHostService::PrivacyMaskingV1, request)
            .await?;
        let envelope: Envelope = serde_json::from_value(envelope)
            .map_err(|_| PrivacyMaskingClientError::InvalidResponse)?;
        match (envelope.result, envelope.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn inspect_caller_context(&self) -> Result<Option<PrivacyMaskingCallerDto>> {
        match self
            .execute(PrivacyMaskingRequest::InspectCallerContext)
            .await?
        {
            PrivacyMaskingResponse::CallerContext { caller } => Ok(caller),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn list_builtin_rules(&self) -> Result<Vec<PrivacyMaskingBuiltinRuleDescriptorDto>> {
        match self
            .execute(PrivacyMaskingRequest::ListBuiltinRules)
            .await?
        {
            PrivacyMaskingResponse::BuiltinRules { items } => Ok(items),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn list_subjects(&self) -> Result<PrivacyMaskingSubjectsPage> {
        match self.execute(PrivacyMaskingRequest::ListSubjects).await? {
            PrivacyMaskingResponse::Subjects {
                model_provider_instances,
                agent_flows,
            } => Ok(PrivacyMaskingSubjectsPage {
                model_provider_instances,
                agent_flows,
            }),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn sync_policy(
        &self,
        policy: PrivacyMaskingWorkspacePolicyDto,
    ) -> Result<PrivacyMaskingSyncReceipt> {
        match self
            .execute(PrivacyMaskingRequest::SyncPolicy { policy })
            .await?
        {
            PrivacyMaskingResponse::PolicySynced {
                policy_version,
                enabled_rule_count,
                active_subject_count,
            } => Ok(PrivacyMaskingSyncReceipt {
                policy_version,
                enabled_rule_count,
                active_subject_count,
            }),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn preview_masking(
        &self,
        text: String,
        policy: Option<PrivacyMaskingWorkspacePolicyDto>,
        restore_output: bool,
    ) -> Result<PrivacyMaskingPreviewResult> {
        match self
            .execute(PrivacyMaskingRequest::PreviewMasking {
                text,
                policy,
                restore_output,
            })
            .await?
        {
            PrivacyMaskingResponse::PreviewMasked {
                masked_text,
                restored_text,
                hits,
                total_hits,
            } => Ok(PrivacyMaskingPreviewResult {
                masked_text,
                restored_text,
                hits,
                total_hits,
            }),
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }

    pub async fn list_logs(
        &self,
        query: PrivacyMaskingLogsQuery,
    ) -> Result<PrivacyMaskingLogsPage> {
        match self
            .execute(PrivacyMaskingRequest::ListLogs {
                subject_kind: query.subject_kind,
                subject_id: query.subject_id,
                placeholder_kind: query.placeholder_kind,
                rule_id: query.rule_id,
                flow_run_id: query.flow_run_id,
                limit: query.limit,
                cursor: query.cursor,
            })
            .await?
        {
            PrivacyMaskingResponse::Logs { items, next_cursor } => {
                Ok(PrivacyMaskingLogsPage { items, next_cursor })
            }
            _ => Err(PrivacyMaskingClientError::InvalidResponse),
        }
    }
}
