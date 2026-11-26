//! Live, installation-owned resource exports. No deployment registry or environment fallback.
use anyhow::{ensure, Result};
use domain::{PluginDesiredState, PluginInstallationRecord};
use extension_contracts::{
    PluginDataBinding, PluginDataOperation, PluginDataRequest, PluginDataTarget,
};
use plugin_framework::{
    PluginDependency, PluginResourceAccess, PluginResourceCollection, PluginResourceExport,
    PluginResourceImport, PluginResourceRecordBinding,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// A frozen API projection, or a descriptor resolved under the caller's authority lease.
/// Identity is always the actual provider installation, never a consumer alias.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPluginResource {
    pub installation_id: Uuid,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub resource_code: String,
    pub collections: Vec<PluginResourceCollection>,
    pub record_resources: Vec<PluginResourceRecordBinding>,
    pub credentials: Option<PluginResourceAccess>,
    pub data_models: Vec<extension_contracts::PluginDataModelContribution>,
}
fn metadata<T: serde::de::DeserializeOwned>(
    i: &PluginInstallationRecord,
    key: &str,
) -> Result<Vec<T>> {
    match i.metadata_json.get(key) {
        None => Ok(Vec::new()),
        Some(value) => Ok(serde_json::from_value(value.clone())?),
    }
}
fn matches_provider(i: &PluginInstallationRecord, p: &PluginDependency) -> bool {
    i.organization == p.publisher_namespace && i.provider_code == p.plugin_code
}
fn enabled(i: &PluginInstallationRecord) -> bool {
    i.desired_state == PluginDesiredState::ActiveRequested
        && i.contract_version == "1flowbase.extension-bus/v1"
        && domain::managed_installation_scope(i, domain::DEFAULT_SCOPE_ID)
            == domain::SYSTEM_SCOPE_ID
}
fn provider<'a>(
    p: &PluginDependency,
    installations: &'a BTreeMap<Uuid, PluginInstallationRecord>,
) -> Result<&'a PluginInstallationRecord> {
    let mut candidates = installations
        .values()
        .filter(|i| matches_provider(i, p) && enabled(i));
    let first = candidates
        .next()
        .ok_or_else(|| dependency_error("plugin_dependency_unavailable", p, None))?;
    ensure!(
        candidates.next().is_none(),
        dependency_error("plugin_dependency_ambiguous", p, None)
    );
    Ok(first)
}
fn dependency_error(code: &str, p: &PluginDependency, resource: Option<&str>) -> anyhow::Error {
    control_plane_contracts::ControlPlaneContractError::PluginDependency {
        code: code.into(),
        provider_publisher_namespace: p.publisher_namespace.clone(),
        provider_plugin_code: p.plugin_code.clone(),
        resource_code: resource.map(str::to_owned),
        dependents: Vec::new(),
    }
    .into()
}
pub fn validate_plugin_dependencies(
    consumer: &PluginInstallationRecord,
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
) -> Result<()> {
    for dependency in metadata::<PluginDependency>(consumer, "plugin_dependencies")? {
        ensure!(
            !matches_provider(consumer, &dependency),
            dependency_error("plugin_dependency_self", &dependency, None)
        );
        provider(&dependency, installations)?;
    }
    resolve_resource_imports(consumer, installations)?;
    Ok(())
}
/// Used before lifecycle mutation, with the same system-workspace fence as runtime reads.
/// Malformed dependency metadata fails closed by retaining the enabled installation as a blocker.
pub fn plugin_resource_dependents(
    target: &PluginInstallationRecord,
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
) -> Vec<String> {
    let mut dependents: Vec<_> = installations
        .values()
        .filter(|i| i.id != target.id && enabled(i))
        .filter(|i| {
            let deps = metadata::<PluginDependency>(i, "plugin_dependencies");
            let imports = metadata::<PluginResourceImport>(i, "resource_imports");
            match (deps, imports) {
                (Ok(deps), Ok(imports)) => {
                    deps.iter().any(|d| matches_provider(target, d))
                        || imports
                            .iter()
                            .any(|r| matches_provider(target, &r.provider))
                }
                _ => true,
            }
        })
        .map(|i| {
            format!(
                "{}/{}@{}",
                i.organization, i.provider_code, i.plugin_version
            )
        })
        .collect();
    dependents.sort();
    dependents.dedup();
    dependents
}
/// Pure current-facts resolver; the caller owns serialization and never borrows another connection.
pub fn resolve_resource_imports(
    consumer: &PluginInstallationRecord,
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
) -> Result<Vec<ResolvedPluginResource>> {
    let dependencies = metadata::<PluginDependency>(consumer, "plugin_dependencies")?;
    let imports = metadata::<PluginResourceImport>(consumer, "resource_imports")?;
    ensure!(
        imports.is_empty()
            || (consumer.contract_version == "1flowbase.extension-bus/v1"
                && domain::managed_installation_scope(consumer, domain::DEFAULT_SCOPE_ID)
                    == domain::SYSTEM_SCOPE_ID),
        "plugin_resource_consumer_scope_denied"
    );
    let mut result = Vec::new();
    let mut collections = BTreeSet::new();
    let mut records = BTreeSet::new();
    let mut resources = BTreeSet::new();
    for import in imports {
        let error = || {
            dependency_error(
                "plugin_resource_import_denied",
                &import.provider,
                Some(&import.resource_code),
            )
        };
        ensure!(
            !matches_provider(consumer, &import.provider)
                && dependencies.iter().any(|d| d == &import.provider),
            error()
        );
        let owner = provider(&import.provider, installations)?;
        ensure!(
            resources.insert((owner.id, import.resource_code.clone())),
            error()
        );
        let exports = metadata::<PluginResourceExport>(owner, "resource_exports")?;
        let mut exports = exports
            .iter()
            .filter(|e| e.resource_code == import.resource_code);
        let export = exports.next().ok_or_else(error)?;
        ensure!(exports.next().is_none(), error());
        let models =
            metadata::<extension_contracts::PluginDataModelContribution>(owner, "data_models")?;
        let mut resolved_collections = Vec::new();
        for requested in &import.collections {
            ensure!(
                collections.insert(requested.collection_code.clone()),
                error()
            );
            let mut offered = export
                .collections
                .iter()
                .filter(|c| c.collection_code == requested.collection_code);
            let offered_collection = offered.next().ok_or_else(error)?;
            ensure!(
                offered.next().is_none()
                    && models
                        .iter()
                        .flat_map(|m| &m.owned_collections)
                        .any(|c| c.collection_code == requested.collection_code),
                error()
            );
            ensure!(offered_collection.access.allows(requested.access), error());
            resolved_collections.push(requested.clone());
        }
        let mut record_resources = Vec::new();
        for code in &import.record_resources {
            ensure!(records.insert(code.clone()), error());
            let mut bindings = export
                .record_resources
                .iter()
                .filter(|r| &r.resource_code == code);
            let binding = bindings.next().ok_or_else(error)?;
            ensure!(
                bindings.next().is_none()
                    && resolved_collections
                        .iter()
                        .any(|c| c.collection_code == binding.collection_code),
                error()
            );
            ensure!(!binding.id_field.is_empty(), error());
            record_resources.push(binding.clone());
        }
        // An ACL-protected exported collection cannot be imported without its original ACL binding.
        for binding in &export.record_resources {
            if resolved_collections
                .iter()
                .any(|c| c.collection_code == binding.collection_code)
            {
                ensure!(record_resources.contains(binding), error());
            }
        }
        let credentials = match &import.credentials {
            None => None,
            Some(requested) => {
                let offered = export.credentials.ok_or_else(error)?;
                ensure!(offered.allows(*requested), error());
                Some(*requested)
            }
        };
        result.push(ResolvedPluginResource {
            installation_id: owner.id,
            publisher_namespace: owner.organization.clone(),
            plugin_code: owner.provider_code.clone(),
            plugin_version: owner.plugin_version.clone(),
            resource_code: import.resource_code,
            collections: resolved_collections,
            record_resources,
            credentials,
            data_models: models,
        });
    }
    Ok(result)
}
impl ResolvedPluginResource {
    pub fn owner(&self) -> (&str, &str, &str) {
        (
            &self.publisher_namespace,
            &self.plugin_code,
            &self.plugin_version,
        )
    }
    fn permits(&self, collection: &str, write: bool) -> bool {
        self.collections.iter().any(|c| {
            c.collection_code == collection
                && (!write || c.access == PluginResourceAccess::ReadWrite)
        })
    }
    fn protect_records(&self, binding: &PluginDataBinding, collection: &str) -> Result<()> {
        for protected in self
            .record_resources
            .iter()
            .filter(|r| r.collection_code == collection)
        {
            ensure!(
                binding
                    .record_permissions
                    .as_ref()
                    .is_some_and(|context| context
                        .resources
                        .iter()
                        .any(|r| r.resource_code == protected.resource_code
                            && r.collection_code == protected.collection_code
                            && r.id_field == protected.id_field)),
                "plugin_resource_record_context_required"
            );
        }
        if let Some(context) = &binding.record_permissions {
            ensure!(
                context
                    .resources
                    .iter()
                    .filter(|r| r.collection_code == collection)
                    .all(|r| self
                        .record_resources
                        .iter()
                        .any(|p| p.resource_code == r.resource_code
                            && p.collection_code == r.collection_code
                            && p.id_field == r.id_field)),
                "plugin_resource_record_context_denied"
            );
        }
        Ok(())
    }
}
pub fn resolve_plugin_data_binding(
    consumer: &PluginInstallationRecord,
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
    binding: &PluginDataBinding,
    request: &PluginDataRequest,
) -> Result<PluginDataBinding> {
    let imports = resolve_resource_imports(consumer, installations)?;
    let own_models =
        metadata::<extension_contracts::PluginDataModelContribution>(consumer, "data_models")?;
    let mut selected: Option<(&str, &str, &str)> = None;
    for operation in &request.operations {
        let (target, write) = match operation {
            PluginDataOperation::Find { target, .. }
            | PluginDataOperation::FindOne { target, .. }
            | PluginDataOperation::Count { target, .. } => (target, false),
            PluginDataOperation::Insert { target, .. }
            | PluginDataOperation::Update { target, .. }
            | PluginDataOperation::Delete { target, .. }
            | PluginDataOperation::Upsert { target, .. } => (target, true),
        };
        let PluginDataTarget::OwnedCollection { collection_code } = target else {
            anyhow::bail!("plugin_resource_target_denied")
        };
        let imported = imports.iter().find(|r| {
            r.collections
                .iter()
                .any(|c| &c.collection_code == collection_code)
        });
        let owner = if let Some(resource) = imported {
            ensure!(
                !own_models
                    .iter()
                    .flat_map(|m| &m.owned_collections)
                    .any(|c| &c.collection_code == collection_code),
                "plugin_resource_collection_ambiguous"
            );
            ensure!(
                resource.permits(collection_code, write),
                "plugin_resource_access_denied"
            );
            resource.protect_records(binding, collection_code)?;
            resource.owner()
        } else {
            (
                &*consumer.organization,
                &*consumer.provider_code,
                &*consumer.plugin_version,
            )
        };
        ensure!(
            selected.is_none_or(|prior| prior == owner),
            "plugin_resource_mixed_owners"
        );
        selected = Some(owner);
    }
    let mut resolved = binding.clone();
    if let Some((publisher, code, version)) = selected {
        resolved.publisher_namespace = publisher.into();
        resolved.plugin_code = code.into();
        resolved.plugin_version = version.into();
    }
    Ok(resolved)
}
pub fn resolve_plugin_credential_binding(
    consumer: &PluginInstallationRecord,
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
    binding: &extension_contracts::PluginCredentialBinding,
    request: &extension_contracts::PluginCredentialRequest,
) -> Result<extension_contracts::PluginCredentialBinding> {
    let imports = resolve_resource_imports(consumer, installations)?;
    let mut candidates = imports.iter().filter(|r| r.credentials.is_some());
    let mut resolved = binding.clone();
    if let Some(resource) = candidates.next() {
        ensure!(
            candidates.next().is_none(),
            "plugin_resource_credentials_ambiguous"
        );
        ensure!(
            matches!(
                request,
                extension_contracts::PluginCredentialRequest::Get { .. }
            ) || resource.credentials == Some(PluginResourceAccess::ReadWrite),
            "plugin_resource_credentials_denied"
        );
        resolved.publisher_namespace = resource.publisher_namespace.clone();
        resolved.plugin_code = resource.plugin_code.clone();
        resolved.plugin_version = resource.plugin_version.clone();
    }
    Ok(resolved)
}

#[cfg(test)]
#[path = "_tests/resolution.rs"]
mod tests;
