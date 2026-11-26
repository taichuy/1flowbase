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
fn provider() -> domain::PluginInstallationRecord {
    installation(
        "provider",
        json!({
            "managed_service":{"scope":"system"},
            "resource_exports":[{"resource_code":"records","collections":[{"collection_code":"items","access":"read_write"}],"record_resources":[{"resource_code":"item","collection_code":"items","id_field":"id"}],"credentials":"read_write"}],
            "data_models":[{"contribution_version":"1flowbase.plugin-data-model/v1","storage_binding":"main","owned_collections":[{"collection_code":"items","fields":[{"field_code":"id","field_type":"uuid"}]}]}]
        }),
    )
}
fn consumer() -> domain::PluginInstallationRecord {
    installation(
        "consumer",
        json!({
            "managed_service":{"scope":"system"},
            "plugin_dependencies":[{"publisher_namespace":"fixture.publisher","plugin_code":"provider"}],
            "resource_imports":[{"provider":{"publisher_namespace":"fixture.publisher","plugin_code":"provider"},"resource_code":"records","collections":[{"collection_code":"items","access":"read_write"}],"record_resources":["item"],"credentials":"read_write"}]
        }),
    )
}

#[test]
fn consumer_preflight_accepts_live_exports_without_environment_configuration() {
    let p = provider();
    let c = consumer();
    validate(&c, &[(p.id, p)].into()).unwrap();
}
fn assert_dependency_reason(error: anyhow::Error, expected: &str, resource: Option<&str>) {
    match error
        .downcast_ref::<ControlPlaneError>()
        .expect("structured dependency failure")
    {
        ControlPlaneError::PluginDependency {
            code,
            provider_publisher_namespace,
            provider_plugin_code,
            resource_code,
            dependents,
        } => {
            assert_eq!(code, expected);
            assert_eq!(provider_publisher_namespace, "fixture.publisher");
            assert_eq!(provider_plugin_code, "provider");
            assert_eq!(resource_code.as_deref(), resource);
            assert_eq!(dependents, &["fixture.publisher/consumer@1.0.0"]);
        }
        other => panic!("unexpected error: {other}"),
    }
}
#[test]
fn consumer_preflight_preserves_precise_missing_disabled_and_missing_export_reasons() {
    let c = consumer();
    assert_dependency_reason(
        validate(&c, &BTreeMap::new()).unwrap_err(),
        "plugin_dependency_missing",
        None,
    );
    let mut p = provider();
    p.desired_state = domain::PluginDesiredState::Disabled;
    assert_dependency_reason(
        validate(&c, &[(p.id, p)].into()).unwrap_err(),
        "plugin_dependency_disabled",
        None,
    );
    let mut p = provider();
    p.organization = "other.publisher".into();
    assert_dependency_reason(
        validate(&c, &[(p.id, p)].into()).unwrap_err(),
        "plugin_dependency_missing",
        None,
    );
    let mut p = provider();
    p.metadata_json["resource_exports"] = json!([]);
    assert_dependency_reason(
        validate(&c, &[(p.id, p)].into()).unwrap_err(),
        "plugin_resource_export_missing",
        Some("records"),
    );
    let p = provider();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    assert_dependency_reason(
        validate(&c, &[(p.id, p), (next.id, next)].into()).unwrap_err(),
        "plugin_dependency_ambiguous",
        None,
    );
}
#[test]
fn provider_disable_and_family_removal_report_enabled_consumer_identity() {
    let p = provider();
    let mut c = consumer();
    let error = provider_change_preflight(&p, None, &[p.clone(), c.clone()]).unwrap_err();
    let error = error.downcast_ref::<ControlPlaneError>().unwrap();
    match error {
        ControlPlaneError::PluginDependency {
            code,
            provider_publisher_namespace,
            provider_plugin_code,
            dependents,
            ..
        } => {
            assert_eq!(code, "plugin_dependency_in_use");
            assert_eq!(provider_publisher_namespace, "fixture.publisher");
            assert_eq!(provider_plugin_code, "provider");
            assert_eq!(dependents, &["fixture.publisher/consumer@1.0.0"]);
        }
        _ => panic!("expected structured lifecycle conflict"),
    }
    c.desired_state = domain::PluginDesiredState::Disabled;
    provider_change_preflight(&p, None, &[p.clone(), c]).unwrap();
}
#[test]
fn provider_version_preflight_accepts_compatible_but_rejects_missing_export_or_changed_acl_identity(
) {
    let p = provider();
    let c = consumer();
    let mut next = p.clone();
    next.id = Uuid::now_v7();
    next.plugin_version = "2.0.0".into();
    next.desired_state = domain::PluginDesiredState::Disabled;
    provider_change_preflight(&p, Some(&next), &[p.clone(), c.clone(), next.clone()]).unwrap();
    next.metadata_json["resource_exports"][0]["record_resources"][0]["id_field"] =
        json!("different_id");
    assert!(
        provider_change_preflight(&p, Some(&next), &[p.clone(), c.clone(), next.clone()]).is_err()
    );
    next.metadata_json["resource_exports"] = json!([]);
    assert!(provider_change_preflight(&p, Some(&next), &[p.clone(), c, next.clone()]).is_err());
}
#[test]
fn resource_import_does_not_replace_explicit_plugin_dependency() {
    let mut c = consumer();
    let p = provider();
    c.metadata_json["plugin_dependencies"] = json!([]);
    assert!(validate(&c, &[(p.id, p)].into()).is_err());
}

#[test]
fn lifecycle_rejects_rights_above_export_even_if_runtime_resolution_intersects() {
    let c = consumer();
    for field in ["collections", "credentials"] {
        let mut p = provider();
        if field == "collections" {
            p.metadata_json["resource_exports"][0]["collections"][0]["access"] = json!("read_only");
        } else {
            p.metadata_json["resource_exports"][0]["credentials"] = json!("read_only");
        }
        let result = validate(&c, &[(p.id, p.clone())].into());
        assert!(
            result.is_err(),
            "enable must not grant a silently reduced contract"
        );
        let mut next = p.clone();
        next.id = Uuid::now_v7();
        next.desired_state = domain::PluginDesiredState::Disabled;
        let old = provider();
        assert!(provider_change_preflight(
            &old,
            Some(&next),
            &[old.clone(), c.clone(), next.clone()]
        )
        .is_err());
    }
}

#[test]
fn standalone_provider_has_no_dependency_or_import_requirement() {
    validate(&provider(), &BTreeMap::new()).unwrap();
}
