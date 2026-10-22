use plugin_framework::{
    HostExtensionBootstrapPhase, HostExtensionRegistry, HostInfrastructureProviderManifest,
    RegisteredHostExtension,
};

fn registered_extension(extension_id: &str) -> RegisteredHostExtension {
    RegisteredHostExtension {
        extension_id: extension_id.into(),
        bootstrap_phase: HostExtensionBootstrapPhase::Boot,
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
