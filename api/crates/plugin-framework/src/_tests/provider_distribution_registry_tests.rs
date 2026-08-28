use std::collections::{BTreeMap, BTreeSet};

use extension_contracts::{
    ProviderDistributionConfigField, ProviderDistributionConfigValue,
    ProviderDistributionConfigValueType, ProviderDistributionRuleContribution,
    PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1,
};

use crate::provider_distribution_registry::{
    ProviderDistributionHandlerRef, ProviderDistributionRegistryError,
    ProviderDistributionRuleRegistry,
};

fn contribution(rule_id: &str) -> ProviderDistributionRuleContribution {
    ProviderDistributionRuleContribution {
        rule_id: rule_id.to_string(),
        rule_version: "1.0.0".to_string(),
        contract_version: PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1.to_string(),
        display_name: "Session retry".to_string(),
        handler: "select".to_string(),
        required_permissions: BTreeSet::from([
            "plugin_data.read".to_string(),
            "plugin_data.write".to_string(),
        ]),
        config_fields: BTreeMap::from([(
            "affinity_namespace".to_string(),
            ProviderDistributionConfigField {
                value_type: ProviderDistributionConfigValueType::String,
                required: true,
            },
        )]),
    }
}

#[test]
fn drs_001_builtins_and_runtime_contribution_share_one_registry_fingerprint() {
    let registry = ProviderDistributionRuleRegistry::compile([(
        "session-retry-distribution@1.0.0".to_string(),
        contribution("@taichuy/session_retry"),
    )])
    .unwrap();
    assert_eq!(registry.definitions().count(), 4);
    assert!(matches!(
        registry.get("@taichuy/session_retry").unwrap().handler,
        ProviderDistributionHandlerRef::RuntimeExtension { .. }
    ));
    assert_eq!(registry.fingerprint().len(), 64);
}

#[test]
fn drs_002_duplicate_unknown_permission_and_config_fail_closed() {
    assert!(matches!(
        ProviderDistributionRuleRegistry::compile([(
            "attacker@1".to_string(),
            contribution("builtin.none")
        )]),
        Err(ProviderDistributionRegistryError::DuplicateRule(_))
    ));
    let mut invalid = contribution("@taichuy/invalid");
    invalid
        .required_permissions
        .insert("host.registry".to_string());
    assert!(matches!(
        ProviderDistributionRuleRegistry::compile([("invalid@1".to_string(), invalid)]),
        Err(ProviderDistributionRegistryError::InvalidContribution)
    ));
    let registry = ProviderDistributionRuleRegistry::compile([(
        "session@1".to_string(),
        contribution("@taichuy/session_retry"),
    )])
    .unwrap();
    assert!(registry
        .validate_config("missing", &BTreeMap::new())
        .is_err());
    assert!(registry
        .validate_config(
            "@taichuy/session_retry",
            &BTreeMap::from([(
                "affinity_namespace".to_string(),
                ProviderDistributionConfigValue::Boolean(true),
            )]),
        )
        .is_err());
}
