use super::*;
use extension_contracts::*;
use serde_json::json;

pub(super) fn registry() -> SharedResourceRegistry {
    let registry: SharedResourceRegistry = serde_json::from_value(json!([{
        "contract_id":"example/hosts/v1", "owner":{"publisher_namespace":"original","plugin_code":"ssh"},
        "schema_version":"shared-v1", "participants":[
            {"publisher_namespace":"original","plugin_code":"ssh"},
            {"publisher_namespace":"consumer","plugin_code":"monitor"}],
        "collections":["hosts"], "record_resources":["hosts"], "record_resource_bindings":{"hosts":{"collection_code":"hosts","id_field":"id"}}, "credentials":true
    }])).unwrap();
    registry.validate().unwrap();
    registry
}
#[test]
fn sharing_defaults_to_isolation_and_only_host_allowlisted_participants_resolve() {
    let registry = registry();
    assert!(registry
        .resolve(None, "consumer", "monitor")
        .unwrap()
        .is_none());
    assert!(SharedResourceRegistry::default()
        .resolve(Some("example/hosts/v1"), "consumer", "monitor")
        .is_err());
    assert!(registry
        .resolve(Some("example/hosts/v1"), "intruder", "monitor")
        .is_err());
    assert!(registry
        .resolve(Some("example/hosts/v1"), "consumer", "other")
        .is_err());
    assert!(registry
        .resolve(Some("unknown"), "consumer", "monitor")
        .is_err());
    for (publisher, plugin) in [("original", "ssh"), ("consumer", "monitor")] {
        let contract = registry
            .resolve(Some("example/hosts/v1"), publisher, plugin)
            .unwrap()
            .unwrap();
        assert_eq!(contract.schema_owner().stable_id(), "original/ssh");
        assert_eq!(contract.schema_owner().plugin_version, "shared-v1");
    }
}
#[test]
fn shared_schema_requires_the_complete_exact_collection_set() {
    let contract = &registry().0[0];
    let schema: PluginDataModelContribution = serde_json::from_value(json!({
        "contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
        "owned_collections":[{"collection_code":"hosts","fields":[{"field_code":"name","field_type":"text","nullable":false}]}]
    })).unwrap();
    contract
        .validate_schema(std::slice::from_ref(&schema))
        .unwrap();
    assert!(contract.validate_schema(&[]).is_err());
    let mut other = schema.clone();
    other.owned_collections[0].collection_code = "other".into();
    assert!(contract.validate_schema(&[other]).is_err());
    assert!(contract.validate_schema(&[schema.clone(), schema]).is_err());
}
#[test]
fn credential_delegation_retains_original_aad_identity_and_original_caller_authority() {
    let contract = &registry().0[0];
    let binding = PluginCredentialBinding {
        installation_id: "consumer-install".into(),
        contribution_id: "monitor.read".into(),
        publisher_namespace: "consumer".into(),
        plugin_code: "monitor".into(),
        plugin_version: "0.1.0".into(),
        scope_id: domain::SYSTEM_SCOPE_ID.to_string(),
        deadline_unix_ms: 9000,
    };
    let resolved = contract.credential_binding(&binding).unwrap();
    assert_eq!(
        (&resolved.publisher_namespace, &resolved.plugin_code),
        (&"original".to_string(), &"ssh".to_string())
    );
    assert_eq!(resolved.installation_id, binding.installation_id);
    assert_eq!(resolved.contribution_id, binding.contribution_id);
    assert_eq!(resolved.scope_id, binding.scope_id);
    assert_eq!(resolved.deadline_unix_ms, binding.deadline_unix_ms);
    let mut denied = contract.clone();
    denied.credentials = false;
    assert!(denied.credential_binding(&binding).is_err());
}
#[test]
fn invalid_or_ambiguous_host_configuration_fails_closed() {
    let mut duplicate = registry();
    duplicate.0.push(duplicate.0[0].clone());
    assert!(duplicate.validate().is_err());
    let mut missing_owner = registry();
    missing_owner.0[0]
        .participants
        .retain(|p| p.publisher_namespace != "original");
    assert!(missing_owner.validate().is_err());
    let mut empty = registry();
    empty.0[0].collections.clear();
    assert!(empty.validate().is_err());
    let forged = json!({"contract_id":"example/hosts/v1","owner":"caller-chosen"});
    assert!(serde_json::from_value::<SharedResourceContract>(forged).is_err());
}

#[test]
fn shared_schema_cannot_omit_a_nullable_legacy_field_before_first_projection() {
    use crate::ports::{
        ManagedSchemaFieldType, ManagedSchemaObjectKind, ManagedSchemaOwnershipRecord,
    };
    let contract = &registry().0[0];
    let existing = vec![ManagedSchemaOwnershipRecord {
        ownership_key: "column:example.legacy".into(),
        owner_id: "original/ssh".into(),
        owner_version: "0.1.21".into(),
        object_kind: ManagedSchemaObjectKind::OwnedField,
        logical_name: "hosts.legacy".into(),
        physical_table: "example_hosts".into(),
        physical_column: Some("legacy".into()),
        field_type: Some(ManagedSchemaFieldType::Text),
        nullable: Some(true),
        active: true,
        plan_fingerprint: "legacy".into(),
    }];
    let mut schema: PluginDataModelContribution = serde_json::from_value(json!({
        "contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
        "owned_collections":[{"collection_code":"hosts","fields":[{"field_code":"name","field_type":"text","nullable":false}]}]
    })).unwrap();
    contract
        .validate_existing_schema(&[schema.clone()], &[])
        .unwrap();
    assert!(contract
        .validate_existing_schema(&[schema.clone()], &existing)
        .is_err());
    schema.owned_collections[0].fields.push(PluginOwnedField {
        field_code: "legacy".into(),
        field_type: PluginDataFieldType::Text,
        nullable: true,
    });
    contract
        .validate_existing_schema(&[schema.clone()], &existing)
        .unwrap();
    schema.owned_collections[0].fields[1].nullable = false;
    assert!(contract
        .validate_existing_schema(&[schema.clone()], &existing)
        .is_err());
    schema.owned_collections[0].fields[1].nullable = true;
    schema.owned_collections[0].fields[1].field_type = PluginDataFieldType::Boolean;
    assert!(contract
        .validate_existing_schema(&[schema], &existing)
        .is_err());
}
#[test]
fn shared_row_policy_protection_cannot_be_omitted_or_retargeted() {
    let contract = &registry().0[0];
    let mut resource = plugin_framework::ManagedServiceRecordResource {
        resource_code: "hosts".into(),
        collection_code: "hosts".into(),
        id_field: "id".into(),
        label: "Hosts".into(),
        description: "Hosts".into(),
        actions: vec![],
    };
    contract
        .validate_record_resources(&[resource.clone()])
        .unwrap();
    assert!(contract.validate_record_resources(&[]).is_err());
    resource.id_field = "other_id".into();
    assert!(contract
        .validate_record_resources(&[resource.clone()])
        .is_err());
    resource.id_field = "id".into();
    resource.collection_code = "credential_staging".into();
    assert!(contract.validate_record_resources(&[resource]).is_err());
    let mut binding = PluginDataBinding {
        managed_subject: None,
        record_permissions: None,
        publisher_namespace: "consumer".into(),
        plugin_code: "monitor".into(),
        plugin_version: "0.1.0".into(),
        storage_binding: "main".into(),
        workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
        actor_id: None,
        provider_instance_id: "consumer-install".into(),
        permissions: [PluginDataPermission::Read].into(),
        deadline_unix_ms: 9000,
    };
    let request = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Count {
            target: PluginDataTarget::OwnedCollection {
                collection_code: "hosts".into(),
            },
            filters: vec![],
        }],
    };
    assert!(contract.data_binding(&binding, &request).is_err());
    binding.record_permissions = Some(PluginDataRecordPermissionContext {
        caller_workspace_id: "caller-workspace".into(),
        actor_user_id: "caller-user".into(),
        active_role_code: "operator".into(),
        is_root: false,
        resources: vec![PluginDataRecordResource {
            resource_code: "hosts".into(),
            collection_code: "hosts".into(),
            id_field: "id".into(),
            action_code: "open".into(),
            record_id: Some("host-1".into()),
        }],
    });
    let resolved = contract.data_binding(&binding, &request).unwrap();
    assert_eq!(resolved.record_permissions, binding.record_permissions);
    assert_eq!(resolved.publisher_namespace, "original");
    assert_eq!(resolved.plugin_code, "ssh");
    binding.record_permissions.as_mut().unwrap().resources[0].id_field = "other_id".into();
    assert!(contract.data_binding(&binding, &request).is_err());
}
