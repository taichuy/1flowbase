use super::*;
use serde_json::json;

fn installation(code: &str, metadata_json: serde_json::Value) -> domain::PluginInstallationRecord {
    domain::PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "fixture.publisher".into(),
        provider_code: code.into(),
        plugin_id: format!("{code}@1.0.0"),
        plugin_version: "1.0.0".into(),
        contract_version: "1flowbase.extension-bus/v1".into(),
        protocol: "extension-bus".into(),
        display_name: code.into(),
        source_kind: "upload".into(),
        trust_level: "untrusted".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json,
        is_system_reserved: false,
        created_by: Uuid::nil(),
        updated_by: None,
        created_at: time::OffsetDateTime::UNIX_EPOCH,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
    }
}
pub(super) fn provider() -> domain::PluginInstallationRecord {
    installation(
        "provider",
        json!({
            "managed_service":{"scope":"system"},
            "resource_exports":[{"resource_code":"records","collections":[{"collection_code":"items","access":"read_write"}],"record_resources":[{"resource_code":"item","collection_code":"items","id_field":"id"}],"credentials":"read_write"}],
            "data_models":[{"contribution_version":"1flowbase.plugin-data-model/v1","storage_binding":"main","owned_collections":[{"collection_code":"items","fields":[{"field_code":"id","field_type":"uuid"}]}]}]
        }),
    )
}
pub(super) fn consumer() -> domain::PluginInstallationRecord {
    installation(
        "consumer",
        json!({
            "managed_service":{"scope":"system"},
            "plugin_dependencies":[{"publisher_namespace":"fixture.publisher","plugin_code":"provider"}],
            "resource_imports":[{"provider":{"publisher_namespace":"fixture.publisher","plugin_code":"provider"},"resource_code":"records","collections":[{"collection_code":"items","access":"read_write"}],"record_resources":["item"],"credentials":"read_write"}]
        }),
    )
}
fn facts(installations: &[domain::PluginInstallationRecord]) -> ManagedDependencyFacts {
    let mut facts = ManagedDependencyFacts::default();
    for i in installations {
        facts.installations.insert(i.id, i.clone());
        facts
            .contexts
            .insert(i.id, [("node-a".into(), domain::SYSTEM_SCOPE_ID)].into());
        facts
            .selected_contexts
            .insert(i.id, [("node-a".into(), domain::SYSTEM_SCOPE_ID)].into());
    }
    facts
}
fn assert_dependency(error: anyhow::Error, code: &str) {
    let error = error
        .downcast_ref::<ControlPlaneError>()
        .expect("structured dependency failure");
    match error {
        ControlPlaneError::PluginDependency {
            code: actual,
            provider_publisher_namespace,
            provider_plugin_code,
            ..
        } => {
            assert_eq!(actual, code);
            assert_eq!(provider_publisher_namespace, "fixture.publisher");
            assert_eq!(provider_plugin_code, "provider");
        }
        other => panic!("unexpected error: {other}"),
    }
}
#[test]
fn ac_002_live_provider_enables_consumer_without_environment_registry() {
    let p = provider();
    let c = consumer();
    facts(&[p, c.clone()]).validate(&c, None).unwrap();
}
#[test]
fn ac_002_missing_disabled_uninstalled_wrong_publisher_and_other_node_fail_closed() {
    let c = consumer();
    let p = provider();
    assert_dependency(
        facts(&[c.clone()]).validate(&c, None).unwrap_err(),
        "plugin_dependency_missing",
    );
    let mut disabled = p.clone();
    disabled.desired_state = domain::PluginDesiredState::Disabled;
    assert_dependency(
        facts(&[disabled, c.clone()])
            .validate(&c, None)
            .unwrap_err(),
        "plugin_dependency_disabled",
    );
    let mut wrong = p.clone();
    wrong.organization = "different.publisher".into();
    assert_dependency(
        facts(&[wrong, c.clone()]).validate(&c, None).unwrap_err(),
        "plugin_dependency_missing",
    );
    let mut snapshot = facts(&[p.clone(), c.clone()]);
    snapshot.selected_contexts.remove(&p.id);
    assert_dependency(
        snapshot.validate(&c, None).unwrap_err(),
        "plugin_dependency_not_selected",
    );
    snapshot
        .contexts
        .insert(p.id, [("node-b".into(), domain::SYSTEM_SCOPE_ID)].into());
    snapshot
        .selected_contexts
        .insert(p.id, [("node-b".into(), domain::SYSTEM_SCOPE_ID)].into());
    assert_dependency(
        snapshot.validate(&c, None).unwrap_err(),
        "plugin_dependency_not_ready",
    );
}
#[test]
fn ac_002_absent_export_and_excess_collection_or_credential_access_reject() {
    let c = consumer();
    for change in 0..4 {
        let mut p = provider();
        match change {
            0 => p.metadata_json["resource_exports"] = json!([]),
            1 => {
                p.metadata_json["resource_exports"][0]["collections"][0]["access"] =
                    json!("read_only")
            }
            2 => p.metadata_json["resource_exports"][0]["credentials"] = json!("read_only"),
            _ => p.metadata_json["data_models"] = json!([]),
        }
        assert_dependency(
            facts(&[p, c.clone()]).validate(&c, None).unwrap_err(),
            if change == 0 {
                "plugin_resource_export_missing"
            } else {
                "plugin_resource_import_denied"
            },
        );
    }
}
#[test]
fn ac_004_enabled_dependents_block_disable_and_uninstall_but_disabled_upload_does_not() {
    let p = provider();
    let mut c = consumer();
    let error = facts(&[p.clone(), c.clone()])
        .guard_removal(&[p.id], None, None)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("fixture.publisher/consumer@1.0.0")
    );
    assert_dependency(error, "plugin_dependency_in_use");
    c.desired_state = domain::PluginDesiredState::Disabled;
    facts(&[p.clone(), c])
        .guard_removal(&[p.id], None, None)
        .unwrap();
}
#[test]
fn ac_004_compatible_provider_switch_succeeds_incompatible_exports_and_acl_binding_reject() {
    let p = provider();
    let c = consumer();
    let mut candidate = p.clone();
    candidate.id = Uuid::now_v7();
    candidate.plugin_version = "2.0.0".into();
    candidate.desired_state = domain::PluginDesiredState::Disabled;
    facts(&[p.clone(), c.clone(), candidate.clone()])
        .guard_removal(&[p.id], Some(candidate.id), None)
        .unwrap();
    for change in 0..4 {
        let mut next = candidate.clone();
        match change {
            0 => next.metadata_json["resource_exports"] = json!([]),
            1 => {
                next.metadata_json["resource_exports"][0]["collections"][0]["access"] =
                    json!("read_only")
            }
            2 => {
                next.metadata_json["resource_exports"][0]["record_resources"][0]["id_field"] =
                    json!("other_id")
            }
            _ => next.metadata_json["resource_exports"][0]["credentials"] = json!("read_only"),
        }
        assert_dependency(
            facts(&[p.clone(), c.clone(), next.clone()])
                .guard_removal(&[p.id], Some(next.id), None)
                .unwrap_err(),
            "plugin_dependency_in_use",
        );
    }
}
#[test]
fn generic_prerequisite_keeps_publisher_node_and_selected_workspace_identity() {
    let workspace = Uuid::now_v7();
    let other = Uuid::now_v7();
    let mut p = provider();
    let mut c = consumer();
    p.metadata_json = json!({});
    c.metadata_json
        .as_object_mut()
        .unwrap()
        .remove("resource_imports");
    let mut snapshot = facts(&[p.clone(), c.clone()]);
    snapshot
        .contexts
        .insert(c.id, [("node-a".into(), workspace)].into());
    snapshot
        .selected_contexts
        .insert(p.id, [("node-a".into(), other)].into());
    assert_dependency(
        snapshot.validate(&c, None).unwrap_err(),
        "plugin_dependency_not_selected",
    );
    snapshot
        .selected_contexts
        .insert(p.id, [("node-a".into(), workspace)].into());
    snapshot.validate(&c, None).unwrap();
    snapshot.selected_contexts.remove(&p.id); // Historical grants are not a current selection.
    snapshot
        .contexts
        .insert(p.id, [("node-a".into(), workspace)].into());
    assert_dependency(
        snapshot.validate(&c, None).unwrap_err(),
        "plugin_dependency_not_selected",
    );
}
#[test]
fn generic_dependency_alone_blocks_removal_and_malformed_metadata_fails_closed() {
    let p = provider();
    let mut c = consumer();
    c.metadata_json
        .as_object_mut()
        .unwrap()
        .remove("resource_imports");
    assert_dependency(
        facts(&[p.clone(), c.clone()])
            .guard_removal(&[p.id], None, None)
            .unwrap_err(),
        "plugin_dependency_in_use",
    );
    c.metadata_json["plugin_dependencies"] = json!({"invalid":true});
    assert!(
        facts(&[p.clone(), c])
            .guard_removal(&[p.id], None, None)
            .is_err()
    );
}

#[test]
fn concurrent_provider_version_activation_cannot_invalidate_enabled_dependents() {
    let p = provider();
    let c = consumer();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    next.desired_state = domain::PluginDesiredState::Disabled;
    let snapshot = facts(&[p.clone(), c, next.clone()]);
    assert_dependency(
        snapshot.guard_provider_activation(&next).unwrap_err(),
        "plugin_dependency_in_use",
    );
    snapshot.guard_provider_activation(&p).unwrap();
}

#[test]
fn compatible_switch_may_reduce_unrequested_rights_and_generic_dependencies_need_no_export() {
    let p = provider();
    let mut c = consumer();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    next.desired_state = domain::PluginDesiredState::Disabled;
    c.metadata_json["resource_imports"][0]["collections"][0]["access"] = json!("read_only");
    c.metadata_json["resource_imports"][0]["credentials"] = json!("read_only");
    next.metadata_json["resource_exports"][0]["collections"][0]["access"] = json!("read_only");
    next.metadata_json["resource_exports"][0]["credentials"] = json!("read_only");
    facts(&[p.clone(), c.clone(), next.clone()])
        .guard_removal(&[p.id], Some(next.id), None)
        .unwrap();
    c.metadata_json
        .as_object_mut()
        .unwrap()
        .remove("resource_imports");
    next.metadata_json["resource_exports"] = json!([]);
    facts(&[p.clone(), c, next.clone()])
        .guard_removal(&[p.id], Some(next.id), None)
        .unwrap();
}

#[test]
fn low_level_projection_guards_identity_exports_and_newly_visible_versions() {
    let p = provider();
    let c = consumer();
    let before = facts(&[p.clone(), c.clone()]);
    for change in 0..4 {
        let mut after = before.clone();
        let provider = after.installations.get_mut(&p.id).unwrap();
        match change {
            0 => provider.organization = "different.publisher".into(),
            1 => provider.metadata_json["resource_exports"] = json!([]),
            2 => {
                provider.metadata_json["resource_exports"][0]["record_resources"][0]["id_field"] =
                    json!("other_id")
            }
            _ => {
                after.selected_contexts.remove(&p.id);
            }
        }
        assert_dependency(
            before.guard_write(&after, p.id, true).unwrap_err(),
            "plugin_dependency_in_use",
        );
    }
    before.guard_write(&before, p.id, true).unwrap();
    let mut after = before.clone();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    after.installations.insert(next.id, next.clone());
    after
        .contexts
        .insert(next.id, [("node-a".into(), domain::SYSTEM_SCOPE_ID)].into());
    after
        .selected_contexts
        .insert(next.id, [("node-a".into(), domain::SYSTEM_SCOPE_ID)].into());
    assert_dependency(
        before.guard_write(&after, next.id, true).unwrap_err(),
        "plugin_dependency_in_use",
    );
}
