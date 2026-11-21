//! Lifecycle preflight only. Resource/permission resolution stays with the resource owner.
use super::*;
use plugin_framework::{PluginDependency, PluginResourceImport};

fn metadata<T: serde::de::DeserializeOwned>(
    installation: &domain::PluginInstallationRecord,
    key: &str,
) -> Result<Vec<T>> {
    installation
        .metadata_json
        .get(key)
        .map(|value| serde_json::from_value(value.clone()).map_err(Into::into))
        .unwrap_or_else(|| Ok(Vec::new()))
}
fn identity(installation: &domain::PluginInstallationRecord) -> String {
    format!(
        "{}/{}@{}",
        installation.organization, installation.provider_code, installation.plugin_version
    )
}
fn matches_provider(
    installation: &domain::PluginInstallationRecord,
    dependency: &PluginDependency,
) -> bool {
    installation.organization == dependency.publisher_namespace
        && installation.provider_code == dependency.plugin_code
}
fn dependency_error(
    code: &str,
    provider: &domain::PluginInstallationRecord,
    resource_code: Option<String>,
    dependents: Vec<String>,
) -> anyhow::Error {
    control_plane_contracts::ControlPlaneContractError::PluginDependency {
        code: code.into(),
        provider_publisher_namespace: provider.organization.clone(),
        provider_plugin_code: provider.provider_code.clone(),
        resource_code,
        dependents,
    }
    .into()
}
fn references(
    consumer: &domain::PluginInstallationRecord,
    provider: &domain::PluginInstallationRecord,
) -> Result<bool> {
    Ok(
        metadata::<PluginDependency>(consumer, "plugin_dependencies")?
            .iter()
            .any(|dependency| matches_provider(provider, dependency))
            || metadata::<PluginResourceImport>(consumer, "resource_imports")?
                .iter()
                .any(|import| matches_provider(provider, &import.provider)),
    )
}
fn validate(
    consumer: &domain::PluginInstallationRecord,
    installations: &BTreeMap<Uuid, domain::PluginInstallationRecord>,
) -> Result<()> {
    for dependency in metadata::<PluginDependency>(consumer, "plugin_dependencies")? {
        let candidates: Vec<_> = installations
            .values()
            .filter(|candidate| matches_provider(candidate, &dependency))
            .collect();
        let active = candidates
            .iter()
            .filter(|candidate| {
                candidate.desired_state == domain::PluginDesiredState::ActiveRequested
            })
            .count();
        let code = if matches_provider(consumer, &dependency) {
            Some("plugin_dependency_self")
        } else if candidates.is_empty() {
            Some("plugin_dependency_missing")
        } else if active == 0 {
            Some("plugin_dependency_disabled")
        } else if active > 1 {
            Some("plugin_dependency_ambiguous")
        } else {
            None
        };
        if let Some(code) = code {
            return Err(
                control_plane_contracts::ControlPlaneContractError::PluginDependency {
                    code: code.into(),
                    provider_publisher_namespace: dependency.publisher_namespace,
                    provider_plugin_code: dependency.plugin_code,
                    resource_code: None,
                    dependents: vec![identity(consumer)],
                }
                .into(),
            );
        }
    }
    for import in metadata::<PluginResourceImport>(consumer, "resource_imports")? {
        if let Some(provider) = installations.values().find(|candidate| {
            matches_provider(candidate, &import.provider)
                && candidate.desired_state == domain::PluginDesiredState::ActiveRequested
        }) {
            if !metadata::<plugin_framework::PluginResourceExport>(provider, "resource_exports")?
                .iter()
                .any(|export| export.resource_code == import.resource_code)
            {
                return Err(dependency_error(
                    "plugin_resource_export_missing",
                    provider,
                    Some(import.resource_code),
                    vec![identity(consumer)],
                ));
            }
        }
    }
    let resolved = super::resolve_resource_imports(consumer, installations)?;
    // Use the owner's permission resolution; lifecycle enable must reject, not silently clamp,
    // a request above its export. No grants/schema/MCP effects have happened at this point.
    for import in metadata::<PluginResourceImport>(consumer, "resource_imports")? {
        let resource = resolved.iter().find(|resource| {
            resource.publisher_namespace == import.provider.publisher_namespace
                && resource.plugin_code == import.provider.plugin_code
                && resource.resource_code == import.resource_code
        });
        let allowed = resource.is_some_and(|resource| {
            import.collections.iter().all(|request| {
                resource.collections.iter().any(|grant| {
                    grant.collection_code == request.collection_code
                        && grant.access.allows(request.access)
                })
            }) && import.credentials.is_none_or(|requested| {
                resource
                    .credentials
                    .is_some_and(|granted| granted.allows(requested))
            })
        });
        if !allowed {
            return Err(
                control_plane_contracts::ControlPlaneContractError::PluginDependency {
                    code: "plugin_resource_import_denied".into(),
                    provider_publisher_namespace: import.provider.publisher_namespace,
                    provider_plugin_code: import.provider.plugin_code,
                    resource_code: Some(import.resource_code),
                    dependents: vec![identity(consumer)],
                }
                .into(),
            );
        }
    }
    Ok(())
}

pub(super) async fn activation_preflight<R: PluginRepository>(
    repository: &R,
    node_id: &str,
    workspace_id: Uuid,
    installation_id: Uuid,
    manifest: &PluginManifestV1,
) -> Result<()> {
    if manifest.plugin_dependencies.is_empty() && manifest.resource_imports.is_empty() {
        return Ok(());
    }
    let mut consumer = repository
        .get_installation(installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    // The verified artifact, not an API payload or an older cached projection, is authoritative.
    consumer.metadata_json["plugin_dependencies"] =
        serde_json::to_value(&manifest.plugin_dependencies)?;
    consumer.metadata_json["resource_imports"] = serde_json::to_value(&manifest.resource_imports)?;
    let selected: HashSet<_> = repository
        .list_assignments(workspace_id)
        .await?
        .into_iter()
        .map(|assignment| assignment.installation_id)
        .collect();
    let ready: HashMap<_, _> = repository
        .list_artifact_instances(node_id)
        .await?
        .into_iter()
        .filter(|artifact| artifact.artifact_status.is_ready())
        .map(|artifact| (artifact.installation_id, artifact.local_version))
        .collect();
    let all = repository.list_installations().await?;
    let ready_here = |candidate: &domain::PluginInstallationRecord| {
        ready
            .get(&candidate.id)
            .and_then(|version| version.as_deref())
            == Some(candidate.plugin_version.as_str())
    };
    let installations: BTreeMap<_, _> = all
        .iter()
        .filter(|candidate| {
            candidate.contract_version == "1flowbase.extension-bus/v1"
                && ready_here(candidate)
                && (domain::managed_installation_scope(candidate, workspace_id)
                    == domain::SYSTEM_SCOPE_ID
                    || selected.contains(&candidate.id))
        })
        .cloned()
        .map(|candidate| (candidate.id, candidate))
        .collect();
    for dependency in &manifest.plugin_dependencies {
        if installations.values().any(|candidate| {
            matches_provider(candidate, dependency)
                && candidate.desired_state == domain::PluginDesiredState::ActiveRequested
        }) {
            continue;
        }
        let candidates: Vec<_> = all
            .iter()
            .filter(|candidate| {
                candidate.contract_version == "1flowbase.extension-bus/v1"
                    && matches_provider(candidate, dependency)
            })
            .collect();
        let active: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                candidate.desired_state == domain::PluginDesiredState::ActiveRequested
            })
            .collect();
        let code = if matches_provider(&consumer, dependency) {
            "plugin_dependency_self"
        } else if candidates.is_empty() {
            "plugin_dependency_missing"
        } else if active.is_empty() {
            "plugin_dependency_disabled"
        } else if !active.iter().any(|candidate| ready_here(candidate)) {
            "plugin_dependency_not_ready"
        } else {
            "plugin_dependency_not_selected"
        };
        return Err(
            control_plane_contracts::ControlPlaneContractError::PluginDependency {
                code: code.into(),
                provider_publisher_namespace: dependency.publisher_namespace.clone(),
                provider_plugin_code: dependency.plugin_code.clone(),
                resource_code: None,
                dependents: vec![identity(&consumer)],
            }
            .into(),
        );
    }
    validate(&consumer, &installations)
}

/// Early rejection for readable task errors; the storage transaction repeats the guard.
pub(super) fn provider_change_preflight(
    provider: &domain::PluginInstallationRecord,
    replacement: Option<&domain::PluginInstallationRecord>,
    installations: &[domain::PluginInstallationRecord],
) -> Result<()> {
    if provider.contract_version != "1flowbase.extension-bus/v1" {
        return Ok(());
    }
    let mut projected: BTreeMap<_, _> = installations
        .iter()
        .cloned()
        .map(|candidate| (candidate.id, candidate))
        .collect();
    if let Some(current) = projected.get_mut(&provider.id) {
        current.desired_state = domain::PluginDesiredState::Disabled;
    }
    if let Some(replacement) = replacement {
        let mut candidate = replacement.clone();
        candidate.desired_state = domain::PluginDesiredState::ActiveRequested;
        projected.insert(candidate.id, candidate);
    }
    let mut blocked = Vec::new();
    for consumer in installations.iter().filter(|candidate| {
        candidate.id != provider.id
            && replacement.is_none_or(|replacement| candidate.id != replacement.id)
            && candidate.contract_version == "1flowbase.extension-bus/v1"
            && candidate.desired_state == domain::PluginDesiredState::ActiveRequested
    }) {
        if !references(consumer, provider)? {
            continue;
        }
        let mut affected = consumer.clone();
        // Other dependencies may belong to a different workspace. Only this provider changes.
        let imports: Vec<_> = metadata::<PluginResourceImport>(consumer, "resource_imports")?
            .into_iter()
            .filter(|import| matches_provider(provider, &import.provider))
            .collect();
        affected.metadata_json["resource_imports"] = serde_json::to_value(&imports)?;
        affected.metadata_json["plugin_dependencies"] =
            serde_json::to_value(vec![PluginDependency {
                publisher_namespace: provider.organization.clone(),
                plugin_code: provider.provider_code.clone(),
            }])?;
        let incompatible = if replacement.is_some() && !imports.is_empty() {
            if validate(&affected, &projected).is_err() {
                blocked.push(identity(consumer));
                continue;
            }
            let original: BTreeMap<_, _> =
                installations.iter().cloned().map(|i| (i.id, i)).collect();
            match (
                super::resolve_resource_imports(&affected, &original),
                super::resolve_resource_imports(&affected, &projected),
            ) {
                (Ok(before), Ok(after)) => before.iter().any(|old| {
                    after
                        .iter()
                        .find(|new| new.resource_code == old.resource_code)
                        .is_none_or(|new| old.record_resources != new.record_resources)
                }),
                _ => true,
            }
        } else {
            false
        };
        if replacement.is_none() || incompatible {
            blocked.push(identity(consumer));
        }
    }
    if !blocked.is_empty() {
        blocked.sort();
        blocked.dedup();
        return Err(dependency_error(
            "plugin_dependency_in_use",
            provider,
            None,
            blocked,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/dependencies.rs"]
mod tests;
