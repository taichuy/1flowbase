//! Deployment-owned resource delegation. Packages can reference, never define, contracts.
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedResourceIdentity {
    pub publisher_namespace: String,
    pub plugin_code: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedRecordResourceBinding {
    pub collection_code: String,
    pub id_field: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedResourceContract {
    pub contract_id: String,
    pub owner: SharedResourceIdentity,
    pub schema_version: String,
    pub participants: BTreeSet<SharedResourceIdentity>,
    pub collections: BTreeSet<String>,
    pub record_resources: BTreeSet<String>,
    pub record_resource_bindings: BTreeMap<String, SharedRecordResourceBinding>,
    pub credentials: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SharedResourceRegistry(pub Vec<SharedResourceContract>);
impl SharedResourceRegistry {
    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for c in &self.0 {
            ensure!(
                !c.contract_id.trim().is_empty() && !c.schema_version.trim().is_empty(),
                "shared_resource_contract_invalid"
            );
            ensure!(
                ids.insert(&c.contract_id) && owners.insert(&c.owner),
                "shared_resource_contract_duplicate"
            );
            ensure!(
                !c.collections.is_empty() && c.participants.contains(&c.owner),
                "shared_resource_contract_invalid"
            );
            ensure!(
                c.record_resource_bindings
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    == c.record_resources,
                "shared_resource_record_bindings_mismatch"
            );
            ensure!(
                c.record_resource_bindings
                    .values()
                    .all(|binding| c.collections.contains(&binding.collection_code)
                        && !binding.id_field.trim().is_empty()),
                "shared_resource_record_binding_invalid"
            );
            for identity in c.participants.iter().chain(std::iter::once(&c.owner)) {
                ensure!(
                    !identity.publisher_namespace.trim().is_empty()
                        && !identity.plugin_code.trim().is_empty(),
                    "shared_resource_identity_invalid"
                );
            }
            ensure!(
                c.collections
                    .iter()
                    .chain(&c.record_resources)
                    .all(|v| !v.trim().is_empty()),
                "shared_resource_scope_invalid"
            );
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        reference: Option<&str>,
        publisher: &str,
        code: &str,
    ) -> Result<Option<&SharedResourceContract>> {
        let Some(reference) = reference else {
            return Ok(None);
        };
        let contract = self
            .0
            .iter()
            .find(|c| c.contract_id == reference)
            .ok_or_else(|| anyhow::anyhow!("shared_resource_contract_unknown"))?;
        ensure!(
            contract.participants.contains(&SharedResourceIdentity {
                publisher_namespace: publisher.into(),
                plugin_code: code.into()
            }),
            "shared_resource_participant_denied"
        );
        Ok(Some(contract))
    }
}
impl SharedResourceContract {
    pub fn validate_schema(
        &self,
        models: &[extension_contracts::PluginDataModelContribution],
    ) -> Result<()> {
        let mut collections = BTreeSet::new();
        for model in models {
            model.validate_additive_v1()?;
            ensure!(
                model.extension_fields.is_empty(),
                "shared_resource_extension_fields_denied"
            );
            for collection in &model.owned_collections {
                ensure!(
                    collections.insert(collection.collection_code.clone()),
                    "shared_resource_duplicate_collection"
                );
            }
        }
        ensure!(
            collections == self.collections,
            "shared_resource_schema_collections_mismatch"
        );
        Ok(())
    }
    pub fn validate_record_resources(
        &self,
        resources: &[plugin_framework::ManagedServiceRecordResource],
    ) -> Result<()> {
        ensure!(
            resources
                .iter()
                .map(|r| r.resource_code.clone())
                .collect::<BTreeSet<_>>()
                == self.record_resources,
            "shared_resource_record_resources_mismatch"
        );
        for resource in resources {
            let binding = self
                .record_resource_bindings
                .get(&resource.resource_code)
                .ok_or_else(|| anyhow::anyhow!("shared_resource_record_resource_denied"))?;
            ensure!(
                binding.collection_code == resource.collection_code
                    && binding.id_field == resource.id_field,
                "shared_resource_record_binding_mismatch"
            );
        }
        Ok(())
    }
    /// Before the first shared-version projection exists, an omitted nullable legacy
    /// field must not silently become an inactive/partial shared schema.
    pub fn validate_existing_schema(
        &self,
        models: &[extension_contracts::PluginDataModelContribution],
        existing: &[crate::ports::ManagedSchemaOwnershipRecord],
    ) -> Result<()> {
        use crate::ports::{ManagedSchemaFieldType as T, ManagedSchemaObjectKind};
        let desired = models
            .iter()
            .flat_map(|m| &m.owned_collections)
            .flat_map(|c| {
                c.fields
                    .iter()
                    .map(move |f| (format!("{}.{}", c.collection_code, f.field_code), f))
            })
            .collect::<BTreeMap<_, _>>();
        let owner = self.schema_owner().stable_id();
        for field in existing
            .iter()
            .filter(|r| r.owner_id == owner && r.object_kind == ManagedSchemaObjectKind::OwnedField)
        {
            let Some((collection, _)) = field.logical_name.split_once('.') else {
                bail!("shared_resource_existing_field_invalid")
            };
            if !self.collections.contains(collection) {
                continue;
            }
            let declared = desired
                .get(&field.logical_name)
                .ok_or_else(|| anyhow::anyhow!("shared_resource_existing_field_missing"))?;
            let expected = match declared.field_type {
                extension_contracts::PluginDataFieldType::String => T::String,
                extension_contracts::PluginDataFieldType::Text => T::Text,
                extension_contracts::PluginDataFieldType::Number => T::Number,
                extension_contracts::PluginDataFieldType::Boolean => T::Boolean,
                extension_contracts::PluginDataFieldType::Datetime => T::Datetime,
                extension_contracts::PluginDataFieldType::Json => T::Json,
                extension_contracts::PluginDataFieldType::Uuid => T::Uuid,
            };
            ensure!(
                field.field_type == Some(expected) && field.nullable == Some(declared.nullable),
                "shared_resource_existing_field_mismatch"
            );
        }
        Ok(())
    }
    pub fn schema_owner(&self) -> plugin_framework::PluginSchemaOwner {
        plugin_framework::PluginSchemaOwner {
            publisher_namespace: self.owner.publisher_namespace.clone(),
            plugin_code: self.owner.plugin_code.clone(),
            plugin_version: self.schema_version.clone(),
        }
    }
    pub fn data_binding(
        &self,
        binding: &extension_contracts::PluginDataBinding,
        request: &extension_contracts::PluginDataRequest,
    ) -> Result<extension_contracts::PluginDataBinding> {
        use extension_contracts::{PluginDataOperation::*, PluginDataTarget};
        for operation in &request.operations {
            let target = match operation {
                Find { target, .. }
                | FindOne { target, .. }
                | Count { target, .. }
                | Insert { target, .. }
                | Update { target, .. }
                | Delete { target, .. }
                | Upsert { target, .. } => target,
            };
            let PluginDataTarget::OwnedCollection { collection_code } = target else {
                bail!("shared_resource_target_denied")
            };
            ensure!(
                self.collections.contains(collection_code),
                "shared_resource_collection_denied"
            );
            for (code, protected) in &self.record_resource_bindings {
                if protected.collection_code != *collection_code {
                    continue;
                }
                ensure!(
                    binding
                        .record_permissions
                        .as_ref()
                        .is_some_and(|context| context
                            .resources
                            .iter()
                            .any(|r| r.resource_code == *code
                                && r.collection_code == protected.collection_code
                                && r.id_field == protected.id_field)),
                    "shared_resource_record_context_required"
                );
            }
        }
        if let Some(context) = &binding.record_permissions {
            ensure!(
                context.resources.iter().all(|r| self
                    .record_resource_bindings
                    .get(&r.resource_code)
                    .is_some_and(|protected| protected.collection_code == r.collection_code
                        && protected.id_field == r.id_field)),
                "shared_resource_record_resource_denied"
            );
        }
        let mut resolved = binding.clone();
        resolved.publisher_namespace = self.owner.publisher_namespace.clone();
        resolved.plugin_code = self.owner.plugin_code.clone();
        resolved.plugin_version = self.schema_version.clone();
        Ok(resolved)
    }
    pub fn credential_binding(
        &self,
        binding: &extension_contracts::PluginCredentialBinding,
    ) -> Result<extension_contracts::PluginCredentialBinding> {
        ensure!(self.credentials, "shared_resource_credentials_denied");
        let mut resolved = binding.clone();
        resolved.publisher_namespace = self.owner.publisher_namespace.clone();
        resolved.plugin_code = self.owner.plugin_code.clone();
        resolved.plugin_version = self.schema_version.clone();
        Ok(resolved)
    }
}
static REGISTRY: OnceLock<SharedResourceRegistry> = OnceLock::new();
/// Composition-root configuration, immutable for the process lifetime. No package write path.
pub fn configure_shared_resources(registry: SharedResourceRegistry) -> Result<()> {
    registry.validate()?;
    if let Some(existing) = REGISTRY.get() {
        ensure!(
            existing == &registry,
            "shared_resource_registry_already_configured"
        );
        return Ok(());
    }
    REGISTRY
        .set(registry)
        .map_err(|_| anyhow::anyhow!("shared_resource_registry_already_configured"))
}
pub fn shared_resource_registry() -> &'static SharedResourceRegistry {
    static EMPTY: OnceLock<SharedResourceRegistry> = OnceLock::new();
    REGISTRY
        .get()
        .unwrap_or_else(|| EMPTY.get_or_init(SharedResourceRegistry::default))
}
pub fn installation_shared_resource(
    installation: &domain::PluginInstallationRecord,
) -> Result<Option<&'static SharedResourceContract>> {
    shared_resource_registry().resolve_installation(installation)
}
impl SharedResourceRegistry {
    pub fn resolve_installation(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<Option<&SharedResourceContract>> {
        let reference = installation.metadata_json.get("shared_resource_contract");
        ensure!(
            reference.is_none() || reference.is_some_and(|v| v.is_string()),
            "shared_resource_reference_invalid"
        );
        self.resolve(
            reference.and_then(serde_json::Value::as_str),
            &installation.organization,
            &installation.provider_code,
        )
    }
}

#[cfg(test)]
#[path = "../_tests/shared_resources.rs"]
mod tests;
