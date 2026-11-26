use super::{validate_non_empty, validate_publisher_namespace, PluginManifestV1};
use crate::{FrameworkResult, PluginFrameworkError};
use std::collections::BTreeSet;

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
fn collection_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 63
        && code
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

pub(super) fn validate_resource_declarations(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let has_resources =
        !manifest.resource_exports.is_empty() || !manifest.resource_imports.is_empty();
    if !has_resources && manifest.plugin_dependencies.is_empty() {
        return Ok(());
    }
    let service = manifest
        .managed_service
        .as_ref()
        .ok_or_else(|| invalid("plugin dependencies and resources require a managed service"))?;
    if manifest.manifest_version != 2 || service.scope != crate::ManagedServiceScope::System {
        return Err(invalid(
            "plugin dependencies and resources require a v2 system managed service",
        ));
    }
    let mut dependencies = BTreeSet::new();
    for dependency in &manifest.plugin_dependencies {
        validate_publisher_namespace(&dependency.publisher_namespace)?;
        validate_non_empty(&dependency.plugin_code, "plugin_dependencies.plugin_code")?;
        if !dependencies.insert(dependency.clone())
            || (dependency.publisher_namespace == manifest.publisher_namespace
                && dependency.plugin_code == manifest.plugin_code()?)
        {
            return Err(invalid(
                "plugin dependencies must be unique and cannot reference self",
            ));
        }
    }
    let owned = manifest
        .data_models
        .iter()
        .flat_map(|m| &m.owned_collections)
        .map(|c| c.collection_code.as_str())
        .collect::<BTreeSet<_>>();
    let mut exports = BTreeSet::new();
    for export in &manifest.resource_exports {
        validate_non_empty(&export.resource_code, "resource_exports.resource_code")?;
        if !exports.insert(&export.resource_code) {
            return Err(invalid("duplicate resource export"));
        }
        if export.collections.is_empty() && export.credentials.is_none() {
            return Err(invalid(
                "resource export must expose a collection or credentials",
            ));
        }
        let mut collections = BTreeSet::new();
        for collection in &export.collections {
            if !collection_code(&collection.collection_code)
                || !collections.insert(collection.collection_code.as_str())
                || !owned.contains(collection.collection_code.as_str())
            {
                return Err(invalid(
                    "resource export collections must be unique provider-owned collections",
                ));
            }
        }
        let mut resources = BTreeSet::new();
        for binding in &export.record_resources {
            if !resources.insert(&binding.resource_code)
                || !collections.contains(binding.collection_code.as_str())
                || !service.record_resources.iter().any(|r| {
                    r.resource_code == binding.resource_code
                        && r.collection_code == binding.collection_code
                        && r.id_field == binding.id_field
                })
            {
                return Err(invalid(
                    "exported record binding must match the provider record resource",
                ));
            }
        }
        for resource in &service.record_resources {
            if collections.contains(resource.collection_code.as_str())
                && !resources.contains(&resource.resource_code)
            {
                return Err(invalid(
                    "resource export cannot omit a protected collection record binding",
                ));
            }
        }
        if export.credentials.is_some() && manifest.permissions.secrets != "host_managed" {
            return Err(invalid(
                "credential exports require permissions.secrets=host_managed",
            ));
        }
    }
    let mut imports = BTreeSet::new();
    let mut imported_collections = BTreeSet::new();
    let mut imported_resources = BTreeSet::new();
    let mut credential_owners = 0;
    for import in &manifest.resource_imports {
        validate_non_empty(&import.resource_code, "resource_imports.resource_code")?;
        if !dependencies.contains(&import.provider)
            || !imports.insert((import.provider.clone(), &import.resource_code))
        {
            return Err(invalid(
                "resource imports require a unique declared plugin dependency",
            ));
        }
        if import.collections.is_empty() && import.credentials.is_none() {
            return Err(invalid(
                "resource import must request a collection or credentials",
            ));
        }
        for collection in &import.collections {
            if !collection_code(&collection.collection_code)
                || owned.contains(collection.collection_code.as_str())
                || !imported_collections.insert(&collection.collection_code)
            {
                return Err(invalid(
                    "imported collection cannot duplicate owned schema or another import",
                ));
            }
        }
        for resource in &import.record_resources {
            if !imported_resources.insert(resource)
                || !service.record_resources.iter().any(|r| {
                    &r.resource_code == resource
                        && import
                            .collections
                            .iter()
                            .any(|c| c.collection_code == r.collection_code)
                })
            {
                return Err(invalid(
                    "imported record resources require an explicit local operation mapping for an imported collection",
                ));
            }
        }
        if import.credentials.is_some() {
            credential_owners += 1;
            if credential_owners > 1 || manifest.permissions.secrets != "host_managed" {
                return Err(invalid(
                    "credential imports require one unambiguous owner and permissions.secrets=host_managed",
                ));
            }
        }
    }
    if has_resources
        && manifest.permissions.storage != "host_managed"
        && (!owned.is_empty() || !imported_collections.is_empty())
    {
        return Err(invalid(
            "resource collections require permissions.storage=host_managed",
        ));
    }
    Ok(())
}
