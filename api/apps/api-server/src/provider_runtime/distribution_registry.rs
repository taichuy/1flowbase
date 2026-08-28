use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{bail, Context, Result};
use extension_contracts::{ProviderDistributionConfigValue, ProviderDistributionRuleContribution};
use plugin_framework::{
    parse_plugin_manifest,
    provider_distribution_registry::{
        ProviderDistributionHandlerRef, ProviderDistributionRuleDefinition,
        ProviderDistributionRuleRegistry,
    },
};

#[derive(Debug, Clone)]
pub(super) struct EffectiveProviderDistributionSnapshot {
    contributions: BTreeMap<String, (String, ProviderDistributionRuleContribution)>,
    registry: ProviderDistributionRuleRegistry,
}

impl EffectiveProviderDistributionSnapshot {
    pub(super) fn builtins() -> Result<Self> {
        Ok(Self {
            contributions: BTreeMap::new(),
            registry: ProviderDistributionRuleRegistry::compile([])?,
        })
    }

    pub(super) fn with_runtime_package(
        &self,
        plugin_id: &str,
        package_root: &Path,
    ) -> Result<Self> {
        let raw = fs::read_to_string(package_root.join("manifest.yaml"))
            .with_context(|| format!("failed to read distribution manifest for {plugin_id}"))?;
        let manifest = parse_plugin_manifest(&raw)?;
        if manifest.versioned_plugin_id()? != plugin_id {
            bail!("distribution runtime manifest identity does not match installation");
        }
        let contribution = manifest
            .provider_distribution_rules
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("distribution runtime has no rule contribution"))?;
        self.with_contribution(plugin_id, contribution)
    }

    fn with_contribution(
        &self,
        plugin_id: &str,
        contribution: ProviderDistributionRuleContribution,
    ) -> Result<Self> {
        let mut contributions = self.contributions.clone();
        if contributions
            .get(&contribution.rule_id)
            .is_some_and(|(owner, _)| owner != plugin_id)
        {
            bail!("provider distribution rule is already owned by another runtime");
        }
        contributions.insert(
            contribution.rule_id.clone(),
            (plugin_id.to_string(), contribution),
        );
        Self::compile(contributions)
    }

    pub(super) fn without_plugin(&self, plugin_id: &str) -> Result<Self> {
        let contributions = self
            .contributions
            .iter()
            .filter(|(_, (candidate, _))| candidate != plugin_id)
            .map(|(rule_id, contribution)| (rule_id.clone(), contribution.clone()))
            .collect();
        Self::compile(contributions)
    }

    fn compile(
        contributions: BTreeMap<String, (String, ProviderDistributionRuleContribution)>,
    ) -> Result<Self> {
        let registry = ProviderDistributionRuleRegistry::compile(
            contributions
                .values()
                .map(|(plugin_id, contribution)| (plugin_id.clone(), contribution.clone())),
        )?;
        Ok(Self {
            contributions,
            registry,
        })
    }

    pub(super) fn fingerprint(&self) -> &str {
        self.registry.fingerprint()
    }

    pub(super) fn definitions(&self) -> Vec<ProviderDistributionRuleDefinition> {
        self.registry.definitions().cloned().collect()
    }

    pub(super) fn resolve_runtime(
        &self,
        rule_id: &str,
        contract_version: &str,
        config: &BTreeMap<String, ProviderDistributionConfigValue>,
    ) -> Result<(&str, &str)> {
        let definition = self
            .registry
            .get(rule_id)
            .ok_or_else(|| anyhow::anyhow!("provider distribution rule is not active"))?;
        if definition.contract_version != contract_version {
            bail!("provider distribution contract version mismatch");
        }
        self.registry.validate_config(rule_id, config)?;
        match &definition.handler {
            ProviderDistributionHandlerRef::RuntimeExtension { plugin_id, .. } => {
                Ok((plugin_id.as_str(), self.registry.fingerprint()))
            }
            ProviderDistributionHandlerRef::Builtin { .. } => {
                bail!("builtin distribution rule cannot be dispatched as a runtime extension")
            }
        }
    }

    pub(super) fn validate(
        &self,
        rule_id: &str,
        contract_version: &str,
        config: &BTreeMap<String, ProviderDistributionConfigValue>,
    ) -> Result<()> {
        let definition = self
            .registry
            .get(rule_id)
            .ok_or_else(|| anyhow::anyhow!("provider distribution rule is not active"))?;
        if definition.contract_version != contract_version {
            bail!("provider distribution contract version mismatch");
        }
        self.registry.validate_config(rule_id, config)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1;

    fn contribution(rule_id: &str) -> ProviderDistributionRuleContribution {
        ProviderDistributionRuleContribution {
            rule_id: rule_id.to_string(),
            rule_version: "1.0.0".to_string(),
            contract_version: PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1.to_string(),
            display_name: "Session retry".to_string(),
            handler: "select".to_string(),
            required_permissions: std::collections::BTreeSet::from([
                "plugin_data.read".to_string(),
                "plugin_data.write".to_string(),
            ]),
            config_fields: BTreeMap::new(),
        }
    }

    #[test]
    fn active_rule_resolves_host_target_and_shared_fingerprint() {
        let snapshot = EffectiveProviderDistributionSnapshot::compile(BTreeMap::from([(
            "@taichuy/session_retry".to_string(),
            (
                "@taichuy/session-retry-distribution@0.0.0".to_string(),
                contribution("@taichuy/session_retry"),
            ),
        )]))
        .unwrap();
        let (target, fingerprint) = snapshot
            .resolve_runtime(
                "@taichuy/session_retry",
                PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1,
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(target, "@taichuy/session-retry-distribution@0.0.0");
        assert_eq!(fingerprint, snapshot.fingerprint());
    }

    #[test]
    fn duplicate_rule_owner_is_rejected() {
        let first = EffectiveProviderDistributionSnapshot::compile(BTreeMap::from([(
            "@taichuy/session_retry".to_string(),
            (
                "plugin-a@1".to_string(),
                contribution("@taichuy/session_retry"),
            ),
        )]))
        .unwrap();
        assert!(first
            .with_contribution("plugin-b@1", contribution("@taichuy/session_retry"),)
            .is_err());
    }
}
