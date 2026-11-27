//! Privacy masking host service contract; execution identity is bound by the host, not the wire.
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

pub const PRIVACY_MASKING_CONTRACT_ID: &str = "privacy-masking";
pub const PRIVACY_MASKING_CONTRACT_V1: &str = "1";
pub const PRIVACY_SUBJECTS_READ_PERMISSION: &str = "privacy.subjects.read";
pub const PRIVACY_POLICY_SYNC_PERMISSION: &str = "privacy.policy.sync";
pub const PRIVACY_LOGS_READ_PERMISSION: &str = "privacy.logs.read";

fn default_true() -> bool {
    true
}

/// Read-only projection of the host-authenticated caller, never constructed from worker input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingCallerDto {
    pub user_id: String,
    pub tenant_id: String,
    pub current_workspace_id: String,
    pub is_root: bool,
    pub effective_display_role: String,
}

#[derive(Debug, Clone)]
pub struct PrivacyMaskingBinding {
    pub subject: crate::extension_bus::ManagedContributionSubject,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub can_read_subjects: bool,
    pub can_sync_policy: bool,
    pub can_read_logs: bool,
    pub deadline_unix_ms: i64,
    pub caller: Option<PrivacyMaskingCallerDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMaskingSubjectKind {
    AgentFlow,
    ModelProviderInstance,
}

impl PrivacyMaskingSubjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentFlow => "agent_flow",
            Self::ModelProviderInstance => "model_provider_instance",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agent_flow" => Some(Self::AgentFlow),
            "model_provider_instance" => Some(Self::ModelProviderInstance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMaskingRuleKind {
    Builtin,
    CustomRegex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMaskingRuleMode {
    #[default]
    AllEnabled,
    SelectedRules,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingRuleDto {
    pub rule_id: String,
    pub name: String,
    pub kind: PrivacyMaskingRuleKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin_code: Option<String>,
    pub placeholder_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingSubjectBindingDto {
    pub binding_id: String,
    pub subject_kind: PrivacyMaskingSubjectKind,
    pub subject_id: String,
    #[serde(default)]
    pub subject_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub restore_output: bool,
    #[serde(default)]
    pub rule_mode: PrivacyMaskingRuleMode,
    #[serde(default)]
    pub rule_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingWorkspacePolicyDto {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub rules: Vec<PrivacyMaskingRuleDto>,
    #[serde(default)]
    pub subject_bindings: Vec<PrivacyMaskingSubjectBindingDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingBuiltinRuleDescriptorDto {
    pub builtin_code: String,
    pub category: String,
    pub placeholder_kind: String,
    pub default_enabled: bool,
    pub display_name_zh: String,
    pub display_name_en: String,
    pub description_zh: String,
    pub description_en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingModelInstanceSubjectDto {
    pub instance_id: String,
    pub display_name: String,
    pub provider_code: String,
    pub protocol: String,
    pub status: String,
    #[serde(default)]
    pub enabled_model_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingAgentFlowSubjectDto {
    pub application_id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingLogMatchDto {
    pub rule_id: String,
    pub rule_name: String,
    pub placeholder_kind: String,
    pub placeholder: String,
    pub original_len: u32,
    pub prefix_hint: String,
    pub hit_count: u32,
    pub restored_count: u32,
    pub masked_snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingLogEntryDto {
    pub log_id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_instance_id: Option<String>,
    pub subject_kind: PrivacyMaskingSubjectKind,
    pub subject_id: String,
    pub subject_name: String,
    pub model_id: String,
    pub protocol: String,
    pub restore_enabled: bool,
    pub hit_count: u32,
    pub restored_count: u32,
    #[serde(default)]
    pub matched_rules: Vec<PrivacyMaskingLogMatchDto>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyMaskingPreviewHitDto {
    pub rule_id: String,
    pub rule_name: String,
    pub placeholder_kind: String,
    pub placeholder: String,
    pub original_len: u32,
    pub prefix_hint: String,
    pub hit_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrivacyMaskingRequest {
    #[serde(deserialize_with = "crate::workspace_policy_contract::deserialize_empty_variant")]
    InspectCallerContext,
    #[serde(deserialize_with = "crate::workspace_policy_contract::deserialize_empty_variant")]
    ListBuiltinRules,
    #[serde(deserialize_with = "crate::workspace_policy_contract::deserialize_empty_variant")]
    ListSubjects,
    SyncPolicy {
        policy: PrivacyMaskingWorkspacePolicyDto,
    },
    PreviewMasking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<PrivacyMaskingWorkspacePolicyDto>,
        #[serde(default = "default_true")]
        restore_output: bool,
    },
    ListLogs {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject_kind: Option<PrivacyMaskingSubjectKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placeholder_kind: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        flow_run_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
    },
}

impl PrivacyMaskingRequest {
    pub const fn required_permission(&self) -> &'static str {
        match self {
            Self::InspectCallerContext | Self::ListBuiltinRules | Self::ListSubjects => {
                PRIVACY_SUBJECTS_READ_PERMISSION
            }
            Self::SyncPolicy { .. } | Self::PreviewMasking { .. } => PRIVACY_POLICY_SYNC_PERMISSION,
            Self::ListLogs { .. } => PRIVACY_LOGS_READ_PERMISSION,
        }
    }

    pub fn validate(&self) -> Result<(), crate::HostServiceError> {
        match self {
            Self::ListLogs { limit: Some(0), .. } => {
                Err(crate::HostServiceError::new("privacy_logs_limit_invalid"))
            }
            Self::ListLogs {
                limit: Some(limit), ..
            } if *limit > 200 => Err(crate::HostServiceError::new("privacy_logs_limit_invalid")),
            Self::PreviewMasking { text, .. } if text.len() > 256 * 1024 => Err(
                crate::HostServiceError::new("privacy_preview_text_too_large"),
            ),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrivacyMaskingResponse {
    CallerContext {
        caller: Option<PrivacyMaskingCallerDto>,
    },
    BuiltinRules {
        items: Vec<PrivacyMaskingBuiltinRuleDescriptorDto>,
    },
    Subjects {
        model_provider_instances: Vec<PrivacyMaskingModelInstanceSubjectDto>,
        agent_flows: Vec<PrivacyMaskingAgentFlowSubjectDto>,
    },
    PolicySynced {
        policy_version: i64,
        enabled_rule_count: u32,
        active_subject_count: u32,
    },
    PreviewMasked {
        masked_text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restored_text: Option<String>,
        hits: Vec<PrivacyMaskingPreviewHitDto>,
        total_hits: u32,
    },
    Logs {
        items: Vec<PrivacyMaskingLogEntryDto>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
}

pub type PrivacyMaskingFuture<'a> = Pin<
    Box<dyn Future<Output = Result<PrivacyMaskingResponse, crate::HostServiceError>> + Send + 'a>,
>;

pub trait PrivacyMaskingPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        binding: &'a PrivacyMaskingBinding,
        request: &'a PrivacyMaskingRequest,
    ) -> PrivacyMaskingFuture<'a>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_masking_request_round_trips_and_validates() {
        let req = PrivacyMaskingRequest::SyncPolicy {
            policy: PrivacyMaskingWorkspacePolicyDto {
                enabled: true,
                allowlist: vec!["example-token".into()],
                rules: vec![PrivacyMaskingRuleDto {
                    rule_id: "r1".into(),
                    name: "Custom Token".into(),
                    kind: PrivacyMaskingRuleKind::CustomRegex,
                    builtin_code: None,
                    placeholder_kind: "CUSTOM_TOKEN".into(),
                    pattern: Some("my_[a-z0-9]{8}".into()),
                    enabled: true,
                    priority: 10,
                }],
                subject_bindings: vec![PrivacyMaskingSubjectBindingDto {
                    binding_id: "b1".into(),
                    subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
                    subject_id: "inst-1".into(),
                    subject_name: "OpenAI Prod".into(),
                    enabled: true,
                    restore_output: true,
                    rule_mode: PrivacyMaskingRuleMode::AllEnabled,
                    rule_ids: vec![],
                }],
            },
        };
        assert_eq!(req.required_permission(), PRIVACY_POLICY_SYNC_PERMISSION);
        assert!(req.validate().is_ok());
        let encoded = serde_json::to_value(&req).unwrap();
        let decoded: PrivacyMaskingRequest = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, req);

        let empty_req: PrivacyMaskingRequest =
            serde_json::from_value(serde_json::json!({"operation": "list_subjects"})).unwrap();
        assert_eq!(empty_req, PrivacyMaskingRequest::ListSubjects);
        assert_eq!(
            empty_req.required_permission(),
            PRIVACY_SUBJECTS_READ_PERMISSION
        );

        let bad_limit = PrivacyMaskingRequest::ListLogs {
            subject_kind: None,
            subject_id: None,
            placeholder_kind: None,
            rule_id: None,
            flow_run_id: None,
            limit: Some(0),
            cursor: None,
        };
        assert!(bad_limit.validate().is_err());
    }
}
