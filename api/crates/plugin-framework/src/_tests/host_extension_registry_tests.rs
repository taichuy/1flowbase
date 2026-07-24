use plugin_framework::{
    HostExtensionBootstrapPhase, HostExtensionRegistry, HostInfrastructureProviderManifest,
    RegisteredHostExtension,
};

fn registered_extension(extension_id: &str) -> RegisteredHostExtension {
    RegisteredHostExtension {
        extension_id: extension_id.into(),
        bootstrap_phase: HostExtensionBootstrapPhase::Boot,
        provides_contracts: vec![],
        overrides_contracts: vec![],
        registers_slots: vec![],
        registers_storage: vec![],
        infrastructure_providers: vec![],
        auth_providers: vec![],
        owned_resources: vec![],
        extends_resources: vec![],
        routes: vec![],
        workers: vec![],
        migrations: vec![],
    }
}

fn infrastructure_provider(
    contract: &str,
    provider_code: &str,
) -> HostInfrastructureProviderManifest {
    HostInfrastructureProviderManifest {
        contract: contract.into(),
        provider_code: provider_code.into(),
        display_name: provider_code.to_string(),
        description: None,
        config_ref: format!("secret://system/{provider_code}/config"),
        config_schema: vec![],
    }
}

#[test]
fn registry_rejects_duplicate_contract_without_override() {
    let mut registry = HostExtensionRegistry::default();
    let mut official = registered_extension("official.storage-host");
    official.provides_contracts = vec!["storage-durable".into()];
    official.registers_storage = vec![("storage-durable".into(), "postgres".into())];
    registry
        .register(official)
        .expect("first registration should pass");

    let mut custom = registered_extension("custom.storage-host");
    custom.provides_contracts = vec!["storage-durable".into()];
    custom.registers_storage = vec![("storage-durable".into(), "cockroach".into())];
    let error = registry
        .register(custom)
        .expect_err("duplicate contract requires override");

    assert!(error.to_string().contains("storage-durable"));
}

#[test]
fn registry_allows_explicit_contract_override() {
    let mut registry = HostExtensionRegistry::default();
    let mut official = registered_extension("official.storage-host");
    official.provides_contracts = vec!["storage-ephemeral".into()];
    official.registers_storage = vec![("storage-ephemeral".into(), "memory".into())];
    registry
        .register(official)
        .expect("first registration should pass");

    let mut custom = registered_extension("custom.redis-host");
    custom.overrides_contracts = vec!["storage-ephemeral".into()];
    custom.registers_storage = vec![("storage-ephemeral".into(), "redis".into())];
    registry
        .register(custom)
        .expect("explicit override should pass");

    assert_eq!(
        registry.contract_provider("storage-ephemeral"),
        Some("custom.redis-host")
    );
}

#[test]
fn registry_rejects_duplicate_infrastructure_provider() {
    let mut registry = HostExtensionRegistry::default();
    let mut local = registered_extension("local-infra-host");
    local.infrastructure_providers = vec![infrastructure_provider("storage-ephemeral", "redis")];
    registry
        .register(local)
        .expect("first provider should register");

    let mut redis = registered_extension("redis-infra-host");
    redis.infrastructure_providers = vec![infrastructure_provider("storage-ephemeral", "redis")];
    let error = registry
        .register(redis)
        .expect_err("duplicate provider should fail");

    assert!(error
        .to_string()
        .contains("duplicate infrastructure provider"));
}

#[test]
fn registry_rejects_multiple_default_providers_for_contract() {
    let mut registry = HostExtensionRegistry::default();
    let mut local = registered_extension("local-infra-host");
    local.infrastructure_providers = vec![infrastructure_provider("storage-ephemeral", "local")];
    registry
        .register(local)
        .expect("first default provider should register");

    let mut redis = registered_extension("redis-infra-host");
    redis.infrastructure_providers = vec![infrastructure_provider("storage-ephemeral", "redis")];
    let error = registry
        .register(redis)
        .expect_err("second default provider should fail");

    assert!(error.to_string().contains("default provider"));
}
