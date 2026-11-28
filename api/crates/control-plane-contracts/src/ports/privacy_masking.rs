use anyhow::Result;
use async_trait::async_trait;
use extension_contracts::{
    PrivacyMaskingLogEntryDto, PrivacyMaskingLogMatchDto, PrivacyMaskingSubjectKind,
    PrivacyMaskingWorkspacePolicyDto,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyMaskingRuntimePolicyRecord {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub policy_version: i64,
    pub policy: PrivacyMaskingWorkspacePolicyDto,
    pub updated_by: Option<Uuid>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct UpsertPrivacyMaskingRuntimePolicyInput {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub policy: PrivacyMaskingWorkspacePolicyDto,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct AppendPrivacyMaskingLogInput {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub flow_run_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub provider_instance_id: Option<Uuid>,
    pub subject_kind: PrivacyMaskingSubjectKind,
    pub subject_id: String,
    pub subject_name: String,
    pub model_id: String,
    pub protocol: String,
    pub restore_enabled: bool,
    pub hit_count: u32,
    pub restored_count: u32,
    pub matched_rules: Vec<PrivacyMaskingLogMatchDto>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Default)]
pub struct ListPrivacyMaskingLogsQuery {
    pub workspace_id: Uuid,
    pub subject_kind: Option<PrivacyMaskingSubjectKind>,
    pub subject_id: Option<String>,
    pub placeholder_kind: Option<String>,
    pub rule_id: Option<String>,
    pub flow_run_id: Option<Uuid>,
    pub limit: u32,
    pub cursor: Option<(OffsetDateTime, Uuid)>,
}

#[derive(Debug, Clone)]
pub struct PrivacyMaskingLogsQueryPage {
    pub items: Vec<PrivacyMaskingLogEntryDto>,
    pub next_cursor: Option<(OffsetDateTime, Uuid)>,
}

/// Process-local notification fired after Postgres commits a policy fence or mutation.
pub trait PrivacyMaskingInvalidationObserver: Send + Sync {
    fn invalidate_installation(&self, installation_id: Uuid);
    fn invalidate_workspace(&self, workspace_id: Uuid);
}

/// A deterministic, side-effect-free transform supplied by the Kernel. The adapter
/// applies it to the current lossless values while holding the run write fence.
pub type PrivacyRunPayloadMask<'a> = dyn Fn(&mut serde_json::Value) + Send + Sync + 'a;

#[async_trait]
pub trait PrivacyMaskingRepository: Send + Sync {
    async fn upsert_privacy_masking_policy(
        &self,
        input: &UpsertPrivacyMaskingRuntimePolicyInput,
    ) -> Result<PrivacyMaskingRuntimePolicyRecord>;

    async fn list_active_privacy_masking_policies(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PrivacyMaskingRuntimePolicyRecord>>;

    async fn delete_privacy_masking_policies_for_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<u64>;

    async fn mask_privacy_run_inputs(
        &self,
        workspace_id: Uuid,
        flow_run_id: Uuid,
        mask: &PrivacyRunPayloadMask<'_>,
    ) -> Result<()>;

    async fn append_privacy_masking_log(&self, input: &AppendPrivacyMaskingLogInput) -> Result<()>;

    async fn list_privacy_masking_logs(
        &self,
        query: &ListPrivacyMaskingLogsQuery,
    ) -> Result<PrivacyMaskingLogsQueryPage>;
}
