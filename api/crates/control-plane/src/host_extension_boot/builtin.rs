use plugin_framework::{
    HostExtensionBootstrapPhase, HostExtensionContributionManifest, HostExtensionManifestV1,
    HostExtensionRegistry, PluginManifestV1, RegisteredHostExtension,
};

pub fn register_builtin_host_extension_contributions(
    manifests: &[(PluginManifestV1, HostExtensionContributionManifest)],
) -> anyhow::Result<HostExtensionRegistry> {
    let mut registry = HostExtensionRegistry::default();
    for (manifest, contribution) in manifests {
        let plugin_code = manifest.plugin_code()?;
        if plugin_code != contribution.extension_id {
            anyhow::bail!(
                "builtin host extension identity mismatch: package {} contribution {}",
                plugin_code,
                contribution.extension_id
            );
        }
        if manifest.version != contribution.version {
            anyhow::bail!(
                "builtin host extension version mismatch: package {} contribution {}",
                manifest.version,
                contribution.version
            );
        }
        register_host_extension_contribution(&mut registry, contribution)?;
    }
    Ok(registry)
}

pub fn register_host_extension_contribution(
    registry: &mut HostExtensionRegistry,
    contribution: &HostExtensionContributionManifest,
) -> anyhow::Result<()> {
    registry.register(RegisteredHostExtension {
        extension_id: contribution.extension_id.clone(),
        bootstrap_phase: contribution.bootstrap_phase,
        provides_contracts: vec![],
        overrides_contracts: vec![],
        registers_slots: vec![],
        registers_storage: vec![],
        infrastructure_providers: contribution.infrastructure_providers.clone(),
        auth_providers: contribution.auth_providers.clone(),
        owned_resources: contribution.owned_resources.clone(),
        extends_resources: contribution.extends_resources.clone(),
        routes: contribution
            .routes
            .iter()
            .map(|route| route.route_id.clone())
            .collect(),
        workers: contribution
            .workers
            .iter()
            .map(|worker| worker.worker_id.clone())
            .collect(),
        migrations: contribution
            .migrations
            .iter()
            .map(|migration| migration.id.clone())
            .collect(),
    })?;
    Ok(())
}

pub fn register_builtin_host_extensions(
    manifests: &[HostExtensionManifestV1],
) -> anyhow::Result<HostExtensionRegistry> {
    let mut registry = HostExtensionRegistry::default();
    for manifest in manifests {
        registry.register(RegisteredHostExtension {
            extension_id: manifest.extension_id.clone(),
            bootstrap_phase: HostExtensionBootstrapPhase::Boot,
            provides_contracts: manifest.provides_contracts.clone(),
            overrides_contracts: manifest.overrides_contracts.clone(),
            registers_slots: manifest.registers_slots.clone(),
            registers_storage: manifest
                .registers_storage
                .iter()
                .map(|entry| (entry.kind.clone(), entry.implementation.clone()))
                .collect(),
            infrastructure_providers: vec![],
            auth_providers: vec![],
            owned_resources: vec![],
            extends_resources: vec![],
            routes: vec![],
            workers: vec![],
            migrations: vec![],
        })?;
    }
    Ok(registry)
}
