use std::collections::BTreeMap;

use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory,
    ConsolePolicyGroup as RegisteredConsolePolicyGroup, ResourceAccessScopeKind,
    SettingsFeatureLifecycle,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
};

pub use control_plane_contracts::console_policy_migration::*;

pub fn compile_console_policy_migration_plan(
    inventory: &ConsoleOperationCompiledInventory,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<CompiledConsolePolicyMigrationPlan, ConsolePolicyMigrationError> {
    let catalog = compiled_catalog_from_inventory(inventory)?;
    compile_console_policy_migration_plan_with_schema(inventory.schema_version, catalog, mappings)
}

fn compiled_catalog_from_inventory(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyCatalog, ConsolePolicyMigrationError> {
    let mut resources = BTreeMap::new();
    for resource in &inventory.resources {
        if resource.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ConsolePolicyMigrationError::new(format!(
                "compiled resource {} is inactive",
                resource.resource_code
            )));
        }
        if resources
            .insert(resource.resource_code.as_str(), resource)
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled resource {}",
                resource.resource_code
            )));
        }
    }

    let mut groups =
        BTreeMap::<ConsolePolicyGroup, BTreeMap<ConsoleOperationId, ConsoleOperationPolicy>>::new();
    for operation in &inventory.operations {
        if operation.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ConsolePolicyMigrationError::new(format!(
                "compiled operation {} is inactive",
                operation.operation_id
            )));
        }
        let group = match &operation.policy_group {
            RegisteredConsolePolicyGroup::SettingsFeature(feature_id) => {
                ConsolePolicyGroup::settings_feature(feature_id).map_err(|_| {
                    ConsolePolicyMigrationError::new("compiled settings feature group is invalid")
                })?
            }
            RegisteredConsolePolicyGroup::Other(group_id) => ConsolePolicyGroup::other(group_id)
                .map_err(|_| ConsolePolicyMigrationError::new("compiled Other group is invalid"))?,
        };
        let operation_id =
            ConsoleOperationId::try_from(operation.operation_id.as_str()).map_err(|_| {
                ConsolePolicyMigrationError::new(format!(
                    "compiled operation {} is invalid",
                    operation.operation_id
                ))
            })?;
        let full_operation = match &operation.authorization {
            ConsoleAuthorization::Authenticated => continue,
            ConsoleAuthorization::Simple => {
                ConsoleOperationPolicy::simple(operation_id.clone(), true)
            }
            ConsoleAuthorization::ResourceAction {
                resource_code,
                action_code,
            } => {
                let resource = resources.get(resource_code.as_str()).ok_or_else(|| {
                    ConsolePolicyMigrationError::new(format!(
                        "compiled operation {} references an unknown resource {}",
                        operation.operation_id, resource_code
                    ))
                })?;
                if resource.scope_kind != ResourceAccessScopeKind::Workspace
                    || resource.scope_field.as_deref() != Some("scope_id")
                    || resource.owner_field.as_deref() != Some("created_by")
                {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "compiled resource {} does not support console own/scope_all authorization",
                        resource_code
                    )));
                }
                if !resource
                    .actions
                    .iter()
                    .any(|action| action.action_code == *action_code)
                {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "compiled operation {} references an unknown action {}.{}",
                        operation.operation_id, resource_code, action_code
                    )));
                }
                ConsoleOperationPolicy::row(
                    operation_id.clone(),
                    ConsoleOperationRowScope::ScopeAll,
                )
            }
        };
        if groups
            .entry(group)
            .or_default()
            .insert(operation_id.clone(), full_operation)
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled operation {}",
                operation_id.as_str()
            )));
        }
    }
    if groups.is_empty() {
        return Err(ConsolePolicyMigrationError::new(
            "compiled inventory has no configurable console operations",
        ));
    }
    Ok(CompiledConsolePolicyCatalog {
        complete: true,
        groups: groups
            .into_iter()
            .map(|(group, operations)| CompiledConsolePolicyGroup {
                group,
                full_operations: operations.into_values().collect(),
            })
            .collect(),
    })
}
