use super::*;

pub(super) fn build_node_contribution_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationNodeContributionsInput {
    let plugin_unique_identifier = stable_plugin_unique_identifier(installation.plugin_id());
    let package_id = installation.plugin_id().to_string();

    ReplaceInstallationNodeContributionsInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .node_contributions
            .iter()
            .map(|entry| NodeContributionRegistryInput {
                plugin_unique_identifier: plugin_unique_identifier.clone(),
                package_id: package_id.clone(),
                contribution_code: entry.contribution_code.clone(),
                node_shell: entry.node_shell.clone(),
                category: entry.category.clone(),
                title: entry.title.clone(),
                description: entry.description.clone(),
                icon: entry.icon.clone(),
                schema_ui: entry.schema_ui.clone(),
                schema_version: entry.schema_version.clone(),
                output_schema: entry.output_schema.clone(),
                contribution_checksum: stable_sha256_json(
                    &serde_json::to_value(entry).unwrap_or_else(|_| json!({})),
                ),
                compiled_contribution_hash: stable_sha256_json(&json!({
                    "schema_version": entry.schema_version,
                    "node_shell": entry.node_shell,
                    "schema_ui": entry.schema_ui,
                    "output_schema": entry.output_schema,
                    "side_effect_policy": entry.side_effect_policy,
                    "infra_contracts": entry.infra_contracts,
                })),
                output_schema_snapshot: entry.output_schema.clone(),
                side_effect_policy: entry.side_effect_policy.clone(),
                infra_contracts: entry.infra_contracts.clone(),
                required_auth: entry.required_auth.clone(),
                visibility: entry.visibility.clone(),
                experimental: entry.experimental,
                dependency_installation_kind: entry.dependency.installation_kind.clone(),
                dependency_plugin_version_range: entry.dependency.plugin_version_range.clone(),
            })
            .collect(),
    }
}

pub(super) fn build_js_dependency_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationJsDependenciesInput {
    ReplaceInstallationJsDependenciesInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .js_dependencies
            .iter()
            .flat_map(|dependency| {
                dependency.targets.iter().filter_map(|target| {
                    dependency.artifacts.get(target).map(|artifact_path| {
                        JsDependencyRegistryInput {
                            alias: dependency.alias.clone(),
                            package: dependency.package.clone(),
                            version: dependency.version.clone(),
                            target: target.clone(),
                            artifact_path: artifact_path.clone(),
                            integrity: dependency.integrity.clone(),
                            permissions: domain::JsDependencyPermissions {
                                network: dependency.permissions.network.clone(),
                                filesystem: dependency.permissions.filesystem.clone(),
                                env: dependency.permissions.env.clone(),
                            },
                        }
                    })
                })
            })
            .collect(),
    }
}

pub(super) fn build_frontend_block_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationFrontendBlocksInput {
    ReplaceInstallationFrontendBlocksInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .block_contributions
            .iter()
            .map(|block| FrontendBlockCatalogRegistryInput {
                contribution_code: block.contribution_code.clone(),
                title: block.title.clone(),
                runtime: block.runtime.clone(),
                entry: block.entry.clone(),
                code_template: block.code_template.clone(),
                code_template_version: block.code_template_version.clone(),
                code_template_language: block.code_template_language.clone(),
                code_modules: block
                    .code_modules
                    .iter()
                    .map(|code_module| domain::FrontendBlockCodeModule {
                        source: code_module.source.clone(),
                        version: code_module.version.clone(),
                        exports: code_module.exports.clone(),
                        binding: match code_module.binding {
                            plugin_framework::FrontendModuleBindingManifest::Host => {
                                domain::FrontendModuleBinding::Host
                            }
                            plugin_framework::FrontendModuleBindingManifest::Fetched => {
                                domain::FrontendModuleBinding::Fetched
                            }
                        },
                        assets: code_module
                            .assets
                            .iter()
                            .map(|asset| domain::FrontendModuleAsset {
                                path: asset.path.clone(),
                                role: match asset.role {
                                    plugin_framework::FrontendModuleAssetRoleManifest::BrowserModule => {
                                        domain::FrontendModuleAssetRole::BrowserModule
                                    }
                                    plugin_framework::FrontendModuleAssetRoleManifest::ShadowStyle => {
                                        domain::FrontendModuleAssetRole::ShadowStyle
                                    }
                                    plugin_framework::FrontendModuleAssetRoleManifest::Support => {
                                        domain::FrontendModuleAssetRole::Support
                                    }
                                },
                                media_type: asset.media_type.clone(),
                                sha256: asset.sha256.clone(),
                            })
                            .collect(),
                        type_declarations: code_module.type_declarations.clone(),
                        components: code_module
                            .components
                            .iter()
                            .map(|component| domain::FrontendComponentContract {
                                component_code: component.component_code.clone(),
                                export_name: component.export_name.clone(),
                                upstream: component.upstream.as_ref().map(|upstream| {
                                    domain::FrontendComponentUpstream {
                                        package: upstream.package.clone(),
                                        component: upstream.component.clone(),
                                        version: upstream.version.clone(),
                                    }
                                }),
                                description: component.description.clone(),
                                props: component
                                    .props
                                    .iter()
                                    .map(|prop| domain::FrontendComponentProp {
                                        name: prop.name.clone(),
                                        type_name: prop.type_name.clone(),
                                        required: prop.required,
                                        description: prop.description.clone(),
                                    })
                                    .collect(),
                                limitations: component.limitations.clone(),
                                examples: component
                                    .examples
                                    .iter()
                                    .map(|example| domain::FrontendComponentExample {
                                        title: example.title.clone(),
                                        code: example.code.clone(),
                                    })
                                    .collect(),
                                insert_snippet: component.insert_snippet.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
                context_contract: domain::FrontendBlockContextContract {
                    primitives: block.context_contract.primitives.clone(),
                    input_schema: block.context_contract.input_schema.clone(),
                },
                permissions: domain::FrontendBlockPermissions {
                    network: block.permissions.network.clone(),
                    storage: block.permissions.storage.clone(),
                    secrets: block.permissions.secrets.clone(),
                },
                ui_capabilities: block.ui_capabilities.clone(),
            })
            .collect(),
    }
}
