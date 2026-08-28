use std::collections::BTreeMap;

use extension_contracts::{
    ProviderDistributionConfigValue, ProviderDistributionConfigValueType,
    ProviderDistributionRuleContribution, PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ProviderDistributionHandlerRef {
    Builtin { code: String },
    RuntimeExtension { plugin_id: String, handler: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderDistributionRuleDefinition {
    pub rule_id: String,
    pub rule_version: String,
    pub contract_version: String,
    pub display_name: String,
    pub handler: ProviderDistributionHandlerRef,
    pub config_fields: BTreeMap<String, extension_contracts::ProviderDistributionConfigField>,
}

#[derive(Debug, Clone)]
pub struct ProviderDistributionRuleRegistry {
    fingerprint: String,
    definitions: BTreeMap<String, ProviderDistributionRuleDefinition>,
}

impl ProviderDistributionRuleRegistry {
    pub fn compile(
        contributions: impl IntoIterator<Item = (String, ProviderDistributionRuleContribution)>,
    ) -> Result<Self, ProviderDistributionRegistryError> {
        let mut definitions = builtins()
            .into_iter()
            .map(|definition| (definition.rule_id.clone(), definition))
            .collect::<BTreeMap<_, _>>();
        for (plugin_id, contribution) in contributions {
            contribution
                .validate()
                .map_err(|_| ProviderDistributionRegistryError::InvalidContribution)?;
            let rule_id = contribution.rule_id.clone();
            let definition = ProviderDistributionRuleDefinition {
                rule_id: rule_id.clone(),
                rule_version: contribution.rule_version,
                contract_version: contribution.contract_version,
                display_name: contribution.display_name,
                handler: ProviderDistributionHandlerRef::RuntimeExtension {
                    plugin_id,
                    handler: contribution.handler,
                },
                config_fields: contribution.config_fields,
            };
            if definitions.insert(rule_id.clone(), definition).is_some() {
                return Err(ProviderDistributionRegistryError::DuplicateRule(rule_id));
            }
        }
        let bytes = serde_json::to_vec(&definitions)
            .map_err(|_| ProviderDistributionRegistryError::Fingerprint)?;
        let fingerprint = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(Self {
            fingerprint,
            definitions,
        })
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn get(&self, rule_id: &str) -> Option<&ProviderDistributionRuleDefinition> {
        self.definitions.get(rule_id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ProviderDistributionRuleDefinition> {
        self.definitions.values()
    }

    pub fn validate_config(
        &self,
        rule_id: &str,
        config: &BTreeMap<String, ProviderDistributionConfigValue>,
    ) -> Result<(), ProviderDistributionRegistryError> {
        let definition = self
            .get(rule_id)
            .ok_or_else(|| ProviderDistributionRegistryError::UnknownRule(rule_id.to_string()))?;
        for (field, descriptor) in &definition.config_fields {
            let value = config.get(field);
            if descriptor.required && value.is_none() {
                return Err(ProviderDistributionRegistryError::InvalidConfig);
            }
            if let Some(value) = value {
                let value_type = match value {
                    ProviderDistributionConfigValue::String(_) => {
                        ProviderDistributionConfigValueType::String
                    }
                    ProviderDistributionConfigValue::Integer(_) => {
                        ProviderDistributionConfigValueType::Integer
                    }
                    ProviderDistributionConfigValue::Boolean(_) => {
                        ProviderDistributionConfigValueType::Boolean
                    }
                };
                if value_type != descriptor.value_type {
                    return Err(ProviderDistributionRegistryError::InvalidConfig);
                }
            }
        }
        if config
            .keys()
            .any(|field| !definition.config_fields.contains_key(field))
        {
            return Err(ProviderDistributionRegistryError::InvalidConfig);
        }
        Ok(())
    }
}

fn builtins() -> Vec<ProviderDistributionRuleDefinition> {
    ["none", "round_robin", "retry_round_robin"]
        .into_iter()
        .map(|code| ProviderDistributionRuleDefinition {
            rule_id: format!("builtin.{code}"),
            rule_version: "1".to_string(),
            contract_version: PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1.to_string(),
            display_name: code.replace('_', " "),
            handler: ProviderDistributionHandlerRef::Builtin {
                code: code.to_string(),
            },
            config_fields: BTreeMap::new(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderDistributionRegistryError {
    #[error("provider distribution contribution is invalid")]
    InvalidContribution,
    #[error("provider distribution rule is duplicated: {0}")]
    DuplicateRule(String),
    #[error("provider distribution rule is unknown: {0}")]
    UnknownRule(String),
    #[error("provider distribution config is invalid")]
    InvalidConfig,
    #[error("provider distribution registry fingerprint failed")]
    Fingerprint,
}
