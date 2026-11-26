use super::*;
use extension_contracts::*;
use serde_json::json;

fn installation(publisher: &str, code: &str) -> PluginInstallationRecord {
    let now = time::OffsetDateTime::now_utc();
    PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: publisher.into(),
        provider_code: code.into(),
        plugin_id: format!("{code}@2.4.0"),
        plugin_version: "2.4.0".into(),
        contract_version: "1flowbase.extension-bus/v1".into(),
        protocol: "stdio_json".into(),
        display_name: code.into(),
        source_kind: "uploaded".into(),
        trust_level: "unverified".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({"managed_service":{"scope":"system"}}),
        is_system_reserved: false,
        created_by: Uuid::nil(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    }
}
fn fixture() -> (
    PluginInstallationRecord,
    BTreeMap<Uuid, PluginInstallationRecord>,
    PluginDataBinding,
) {
    let mut consumer = installation("consumer", "monitor");
    let mut provider = installation("taichuy", "ssh");
    provider.plugin_id = "ssh@0.1.21".into();
    provider.plugin_version = "0.1.21".into();
    provider.metadata_json["resource_exports"] = json!([{
        "resource_code":"ssh-hosts-v1", "collections":[{"collection_code":"ssh_hosts","access":"read_write"}],
        "record_resources":[{"resource_code":"hosts","collection_code":"ssh_hosts","id_field":"host_key"}],
        "credentials":"read_write"
    }]);
    provider.metadata_json["data_models"] = json!([{
        "contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
        "owned_collections":[{"collection_code":"ssh_hosts","fields":[{"field_code":"host_key","field_type":"string"}]}]
    }]);
    consumer.metadata_json["plugin_dependencies"] =
        json!([{"publisher_namespace":"taichuy","plugin_code":"ssh"}]);
    consumer.metadata_json["resource_imports"] = json!([{
        "provider":{"publisher_namespace":"taichuy","plugin_code":"ssh"},"resource_code":"ssh-hosts-v1",
        "collections":[{"collection_code":"ssh_hosts","access":"read_write"}],"record_resources":["hosts"],"credentials":"read_write"
    }]);
    let binding = PluginDataBinding {
        managed_subject: None,
        publisher_namespace: consumer.organization.clone(),
        plugin_code: consumer.provider_code.clone(),
        plugin_version: consumer.plugin_version.clone(),
        storage_binding: "main".into(),
        workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
        actor_id: Some(Uuid::now_v7().to_string()),
        provider_instance_id: consumer.id.to_string(),
        permissions: [PluginDataPermission::Read, PluginDataPermission::Write].into(),
        deadline_unix_ms: i64::MAX,
        record_permissions: Some(PluginDataRecordPermissionContext {
            actor_user_id: Uuid::nil().to_string(),
            caller_workspace_id: Uuid::nil().to_string(),
            active_role_code: "member".into(),
            is_root: false,
            resources: vec![PluginDataRecordResource {
                resource_code: "hosts".into(),
                collection_code: "ssh_hosts".into(),
                id_field: "host_key".into(),
                action_code: "read".into(),
                record_id: Some("original-key".into()),
            }],
        }),
    };
    (consumer, [(provider.id, provider)].into(), binding)
}
fn request(write: bool) -> PluginDataRequest {
    let target = PluginDataTarget::OwnedCollection {
        collection_code: "ssh_hosts".into(),
    };
    PluginDataRequest {
        idempotency_key: None,
        operations: vec![if write {
            PluginDataOperation::Delete {
                target,
                filters: vec![],
            }
        } else {
            PluginDataOperation::Count {
                target,
                filters: vec![],
            }
        }],
    }
}
#[test]
fn live_provider_identity_models_and_original_acl_are_preserved() {
    let (consumer, providers, binding) = fixture();
    let resolved = resolve_resource_imports(&consumer, &providers).unwrap();
    assert_eq!(resolved[0].owner(), ("taichuy", "ssh", "0.1.21"));
    assert_eq!(
        resolved[0].data_models[0].owned_collections[0].fields[0].field_type,
        PluginDataFieldType::String
    );
    for write in [false, true] {
        let bound =
            resolve_plugin_data_binding(&consumer, &providers, &binding, &request(write)).unwrap();
        let mut expected = binding.clone();
        expected.publisher_namespace = "taichuy".into();
        expected.plugin_code = "ssh".into();
        expected.plugin_version = "0.1.21".into();
        assert_eq!(
            bound, expected,
            "only physical owner identity changes, not caller/actor/ACL"
        );
    }
    let mut multiple = binding.clone();
    multiple
        .record_permissions
        .as_mut()
        .unwrap()
        .resources
        .push(PluginDataRecordResource {
            resource_code: "own-notes".into(),
            collection_code: "notes".into(),
            id_field: "id".into(),
            action_code: String::new(),
            record_id: None,
        });
    let resolved =
        resolve_plugin_data_binding(&consumer, &providers, &multiple, &request(false)).unwrap();
    assert_eq!(
        resolved.record_permissions, multiple.record_permissions,
        "unrelated host ACL context is retained, never rewritten as provider grants"
    );
    let mut missing = binding.clone();
    missing.record_permissions = None;
    assert!(resolve_plugin_data_binding(&consumer, &providers, &missing, &request(false)).is_err());
    let mut forged = binding.clone();
    forged.record_permissions.as_mut().unwrap().resources[0].id_field = "alias".into();
    assert!(resolve_plugin_data_binding(&consumer, &providers, &forged, &request(false)).is_err());
}
fn assert_import_denied(error: anyhow::Error) {
    match error.downcast_ref::<control_plane_contracts::ControlPlaneContractError>() {
        Some(control_plane_contracts::ControlPlaneContractError::PluginDependency {
            code,
            provider_publisher_namespace,
            provider_plugin_code,
            resource_code,
            ..
        }) => {
            assert_eq!(code, "plugin_resource_import_denied");
            assert_eq!(provider_publisher_namespace, "taichuy");
            assert_eq!(provider_plugin_code, "ssh");
            assert_eq!(resource_code.as_deref(), Some("ssh-hosts-v1"));
        }
        other => panic!("expected structured import denial, got {other:?}: {error}"),
    }
}
#[test]
fn collection_imports_reject_overrequest_and_preserve_requested_subset() {
    for import in ["read_only", "read_write"] {
        for export in ["read_only", "read_write"] {
            let (mut consumer, mut providers, binding) = fixture();
            consumer.metadata_json["resource_imports"][0]["collections"][0]["access"] =
                json!(import);
            providers.values_mut().next().unwrap().metadata_json["resource_exports"][0]
                ["collections"][0]["access"] = json!(export);
            if import == "read_write" && export == "read_only" {
                assert_import_denied(resolve_resource_imports(&consumer, &providers).unwrap_err());
                assert_import_denied(
                    validate_plugin_dependencies(&consumer, &providers).unwrap_err(),
                );
                for write in [false, true] {
                    assert_import_denied(
                        resolve_plugin_data_binding(
                            &consumer,
                            &providers,
                            &binding,
                            &request(write),
                        )
                        .unwrap_err(),
                    );
                }
                continue;
            }
            let resolved = resolve_resource_imports(&consumer, &providers).unwrap();
            assert_eq!(
                serde_json::to_value(resolved[0].collections[0].access).unwrap(),
                json!(import),
                "retain the requested rights, not all exported rights"
            );
            assert!(validate_plugin_dependencies(&consumer, &providers).is_ok());
            assert!(
                resolve_plugin_data_binding(&consumer, &providers, &binding, &request(false))
                    .is_ok()
            );
            assert_eq!(
                resolve_plugin_data_binding(&consumer, &providers, &binding, &request(true))
                    .is_ok(),
                import == "read_write"
            );
        }
    }
}
#[test]
fn credential_imports_reject_overrequest_and_keep_original_owner_for_requested_subset() {
    for import in ["read_only", "read_write"] {
        for export in ["read_only", "read_write"] {
            let (mut consumer, mut providers, _) = fixture();
            consumer.metadata_json["resource_imports"][0]["credentials"] = json!(import);
            providers.values_mut().next().unwrap().metadata_json["resource_exports"][0]
                ["credentials"] = json!(export);
            let overrequest = import == "read_write" && export == "read_only";
            if overrequest {
                assert_import_denied(resolve_resource_imports(&consumer, &providers).unwrap_err());
                assert_import_denied(
                    validate_plugin_dependencies(&consumer, &providers).unwrap_err(),
                );
            } else {
                let resolved = resolve_resource_imports(&consumer, &providers).unwrap();
                assert_eq!(
                    serde_json::to_value(resolved[0].credentials).unwrap(),
                    json!(import)
                );
                assert!(validate_plugin_dependencies(&consumer, &providers).is_ok());
            }
            let binding = PluginCredentialBinding {
                installation_id: consumer.id.to_string(),
                contribution_id: "monitor.read".into(),
                publisher_namespace: consumer.organization.clone(),
                plugin_code: consumer.provider_code.clone(),
                plugin_version: consumer.plugin_version.clone(),
                scope_id: domain::SYSTEM_SCOPE_ID.to_string(),
                deadline_unix_ms: i64::MAX,
            };
            for (req, write) in [
                (
                    PluginCredentialRequest::Get {
                        credential_id: "existing".into(),
                    },
                    false,
                ),
                (
                    PluginCredentialRequest::Put {
                        credential_id: "existing".into(),
                        value: json!({}),
                    },
                    true,
                ),
                (
                    PluginCredentialRequest::Delete {
                        credential_id: "existing".into(),
                    },
                    true,
                ),
            ] {
                let result =
                    resolve_plugin_credential_binding(&consumer, &providers, &binding, &req);
                if overrequest {
                    assert_import_denied(result.unwrap_err());
                    continue;
                }
                assert_eq!(result.is_ok(), !write || import == "read_write");
                if let Ok(bound) = result {
                    let mut expected = binding.clone();
                    expected.publisher_namespace = "taichuy".into();
                    expected.plugin_code = "ssh".into();
                    expected.plugin_version = "0.1.21".into();
                    assert_eq!(bound, expected);
                }
            }
        }
    }
}
#[test]
fn disabled_consumer_can_resolve_imports_before_activation() {
    let (mut consumer, providers, _) = fixture();
    let expected = resolve_resource_imports(&consumer, &providers).unwrap();
    consumer.desired_state = PluginDesiredState::Disabled;
    assert_eq!(
        resolve_resource_imports(&consumer, &providers).unwrap(),
        expected
    );
    assert!(validate_plugin_dependencies(&consumer, &providers).is_ok());
}
#[test]
fn missing_disabled_ambiguous_and_stale_provider_fail_closed_without_registry() {
    let (mut consumer, mut providers, binding) = fixture();
    assert!(resolve_resource_imports(&consumer, &BTreeMap::new()).is_err());
    let id = *providers.keys().next().unwrap();
    providers.get_mut(&id).unwrap().desired_state = PluginDesiredState::Disabled;
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    providers.get_mut(&id).unwrap().desired_state = PluginDesiredState::ActiveRequested;
    let mut duplicate = providers[&id].clone();
    duplicate.id = Uuid::now_v7();
    providers.insert(duplicate.id, duplicate.clone());
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    providers.remove(&duplicate.id);
    let frozen = resolve_resource_imports(&consumer, &providers).unwrap();
    providers.get_mut(&id).unwrap().plugin_version = "3.0.0".into();
    assert_ne!(
        frozen,
        resolve_resource_imports(&consumer, &providers).unwrap()
    );
    providers.get_mut(&id).unwrap().metadata_json["resource_exports"] = json!([]);
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    consumer.metadata_json["resource_imports"] = json!([]);
    consumer.metadata_json["shared_resource_contract"] = json!("old-env-reference");
    let resolved =
        resolve_plugin_data_binding(&consumer, &providers, &binding, &request(false)).unwrap();
    assert_eq!(
        resolved, binding,
        "legacy config cannot delegate any identity"
    );
    let source = include_str!("../mod.rs");
    assert!(
        !source.contains("std::env")
            && !source.contains("OnceLock")
            && !source.contains("PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON")
    );
}
#[test]
fn missing_dependency_unimported_records_duplicate_aliases_and_mixed_owners_are_denied() {
    let (mut consumer, mut providers, binding) = fixture();
    consumer.metadata_json["plugin_dependencies"] = json!([]);
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    let (mut consumer, _, _) = fixture();
    consumer.metadata_json["resource_imports"][0]["record_resources"] = json!([]);
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    let (mut consumer, _, _) = fixture();
    let import = consumer.metadata_json["resource_imports"][0].clone();
    consumer.metadata_json["resource_imports"]
        .as_array_mut()
        .unwrap()
        .push(import);
    assert!(resolve_resource_imports(&consumer, &providers).is_err());
    let (consumer, _, _) = fixture();
    let mut mixed = request(false);
    mixed.operations.push(PluginDataOperation::Count {
        target: PluginDataTarget::OwnedCollection {
            collection_code: "own".into(),
        },
        filters: vec![],
    });
    assert!(resolve_plugin_data_binding(&consumer, &providers, &binding, &mixed).is_err());
    let owner = providers.values().next().unwrap().clone();
    providers.insert(consumer.id, consumer.clone());
    assert_eq!(
        plugin_resource_dependents(&owner, &providers),
        vec!["consumer/monitor@2.4.0"]
    );
    providers.get_mut(&consumer.id).unwrap().desired_state = PluginDesiredState::Disabled;
    assert!(plugin_resource_dependents(&owner, &providers).is_empty());
}

#[test]
fn shared_read_provider_projection_resolves_the_same_imports_as_full_inventory() {
    let (consumer, mut inventory, _) = fixture();
    let provider_id = *inventory.keys().next().unwrap();
    let provider = inventory.get_mut(&provider_id).unwrap();
    provider.metadata_json["managed_service"]["operations"] = json!([{"unrelated":"payload"}]);
    let mut history = provider.clone();
    history.id = Uuid::now_v7();
    history.desired_state = PluginDesiredState::Disabled;
    history.metadata_json["resource_exports"] = json!("invalid historic data");
    inventory.insert(history.id, history.clone());
    let full = resolve_resource_imports(&consumer, &inventory).unwrap();
    assert_eq!(full[0].installation_id, provider_id);

    inventory.remove(&history.id);
    let provider = inventory.get_mut(&provider_id).unwrap();
    provider.metadata_json = json!({
        "managed_service":{"scope":"system"},
        "resource_exports":provider.metadata_json["resource_exports"],
        "data_models":provider.metadata_json["data_models"],
    });
    assert_eq!(
        resolve_resource_imports(&consumer, &inventory).unwrap(),
        full
    );
}
