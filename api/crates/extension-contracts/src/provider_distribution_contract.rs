use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1: &str = "1flowbase.provider-distribution-rule/v1";
pub const PROVIDER_DISTRIBUTION_RULE_SLOT: &str = "provider_distribution_rule";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProviderDistributionConfigValue {
    String(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDistributionRuleContribution {
    pub rule_id: String,
    pub rule_version: String,
    pub contract_version: String,
    pub display_name: String,
    pub handler: String,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub config_fields: BTreeMap<String, ProviderDistributionConfigField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDistributionConfigField {
    pub value_type: ProviderDistributionConfigValueType,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDistributionConfigValueType {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDistributionCandidate {
    pub target_id: String,
    pub order: u32,
    pub ready: bool,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDistributionInvocation {
    pub invocation_id: String,
    pub conversation_id: Option<String>,
    pub routing_policy_id: String,
    pub attempt: u32,
    pub rule_id: String,
    pub rule_version: String,
    pub contract_version: String,
    pub registry_fingerprint: String,
    #[serde(default)]
    pub config: BTreeMap<String, ProviderDistributionConfigValue>,
    pub candidates: Vec<ProviderDistributionCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderDistributionDecision {
    Select { target_id: String },
    NoEligibleTarget { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDistributionSelectionReceipt {
    pub invocation_id: String,
    pub rule_id: String,
    pub rule_version: String,
    pub contract_version: String,
    pub registry_fingerprint: String,
    pub attempt: u32,
    pub decision: ProviderDistributionDecision,
}

impl ProviderDistributionRuleContribution {
    pub fn validate(&self) -> Result<(), ProviderDistributionContractError> {
        validate_identity(&self.rule_id)?;
        validate_identity(&self.handler)?;
        if self.rule_version.trim().is_empty()
            || self.display_name.trim().is_empty()
            || self.contract_version != PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1
        {
            return Err(ProviderDistributionContractError::InvalidContribution);
        }
        for permission in &self.required_permissions {
            if permission != "plugin_data.read" && permission != "plugin_data.write" {
                return Err(ProviderDistributionContractError::UnknownPermission(
                    permission.clone(),
                ));
            }
        }
        for field in self.config_fields.keys() {
            validate_identity(field)?;
        }
        Ok(())
    }
}

impl ProviderDistributionInvocation {
    pub fn validate(&self) -> Result<(), ProviderDistributionContractError> {
        if self.invocation_id.trim().is_empty()
            || self.routing_policy_id.trim().is_empty()
            || self.rule_version.trim().is_empty()
            || self.contract_version != PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1
            || self.registry_fingerprint.trim().is_empty()
            || self.candidates.is_empty()
            || self.candidates.len() > 64
        {
            return Err(ProviderDistributionContractError::InvalidInvocation);
        }
        validate_identity(&self.rule_id)?;
        let mut targets = BTreeSet::new();
        for candidate in &self.candidates {
            if candidate.target_id.trim().is_empty() || !targets.insert(&candidate.target_id) {
                return Err(ProviderDistributionContractError::InvalidInvocation);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderDistributionContractError {
    #[error("provider distribution contribution is invalid")]
    InvalidContribution,
    #[error("provider distribution invocation is invalid")]
    InvalidInvocation,
    #[error("provider distribution identity is invalid")]
    InvalidIdentity,
    #[error("provider distribution permission is unknown: {0}")]
    UnknownPermission(String),
}

fn validate_identity(value: &str) -> Result<(), ProviderDistributionContractError> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.' | '@' | '/')
        })
    {
        return Err(ProviderDistributionContractError::InvalidIdentity);
    }
    Ok(())
}
