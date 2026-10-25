use super::*;
use serde_json::json;

fn fixture() -> (
    domain::PluginInstallationRecord,
    plugin_framework::PluginManifestV1,
) {
    let mut manifest = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../../extension-package-runtime/src/_tests/managed_manifest.yaml"
    ))
    .unwrap();
    let managed = manifest.managed.as_mut().unwrap();
    managed.module.contributions = vec![serde_json::from_value(json!({
        "contribution_id":"managed_fixture.list", "contributor_module_id":"managed_fixture",
        "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append",
        "required_permissions":["service.execute", "credential.manage", "plugin_data.owned.write"]
    }))
    .unwrap()];
    manifest.data_models = vec![serde_json::from_value(json!({
        "contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
        "owned_collections":[{"collection_code":"connections","fields":[]}]
    }))
    .unwrap()];
    let now = time::OffsetDateTime::now_utc();
    let installation = domain::PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "acme".into(),
        provider_code: manifest.plugin_id.clone(),
        plugin_id: "managed_fixture@0.1.0".into(),
        plugin_version: manifest.version.clone(),
        contract_version: "1flowbase.extension-bus/v1".into(),
        protocol: "stdio_json".into(),
        display_name: "fixture".into(),
        source_kind: "uploaded".into(),
        trust_level: "unverified".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::Disabled,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({"managed_service":{"scope":"system"},"managed_service_permissions":{"secrets":"host_managed"}}),
        is_system_reserved: false,
        created_by: Uuid::nil(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    };
    (installation, manifest)
}

#[test]
fn root_activation_materializes_declared_host_permissions_and_owned_collections() {
    let (installation, manifest) = fixture();
    let policy = HostContributionGrantPolicy::root_composition();
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    assert_eq!(grants.len(), 3);
    assert!(grants
        .iter()
        .any(|grant| grant.permission == "service.execute"
            && grant.resource_scope == ContributionResourceScope::System));
    assert!(grants
        .iter()
        .any(|grant| grant.permission == "credential.manage"
            && grant.permission_contract_id == "plugin-credential"));
    assert!(grants
        .iter()
        .any(|grant| grant.permission == "plugin_data.owned.write"
            && grant.resource_scope
                == ContributionResourceScope::OwnedCollection {
                    collection_code: "connections".into()
                }));
}

#[test]
fn root_activation_does_not_invent_collections_or_override_host_capability_contracts() {
    let (mut installation, mut manifest) = fixture();
    let policy = HostContributionGrantPolicy::root_composition();
    manifest.data_models.clear();
    assert!(policy
        .declared_activation_grants(&installation, &manifest)
        .is_err());
    let (_, declared) = fixture();
    installation.metadata_json["managed_service_permissions"]["secrets"] = json!("none");
    assert!(policy
        .declared_activation_grants(&installation, &declared)
        .is_err());
}

fn authentication_fixture() -> (
    domain::PluginInstallationRecord,
    plugin_framework::PluginManifestV1,
) {
    let (mut installation, mut manifest) = fixture();
    installation.metadata_json = json!({"binding_targets":["system"]});
    manifest.data_models.clear();
    manifest.managed.as_mut().unwrap().module.contributions = vec![serde_json::from_value(json!({
        "contribution_id":"managed_fixture.auth", "contributor_module_id":"managed_fixture",
        "point_id":"1flowbase.authentication.provider", "contract_version":"1", "mode":"append",
        "required_permissions":["authentication.verify", "network.http.request"]
    }))
    .unwrap()];
    (installation, manifest)
}

#[test]
fn authentication_activation_grants_exact_system_verification_and_http_permissions() {
    let (installation, manifest) = authentication_fixture();
    let policy = HostContributionGrantPolicy::root_composition();
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    assert_eq!(grants.len(), 2);
    assert!(grants
        .iter()
        .all(|grant| grant.resource_scope == ContributionResourceScope::System));
    assert!(grants
        .iter()
        .any(|grant| grant.permission == "authentication.verify"
            && grant.permission_contract_id == "authentication"));
    assert!(grants
        .iter()
        .any(|grant| grant.permission == "network.http.request"
            && grant.permission_contract_id == "http-egress"));
    let contribution = &manifest.managed.as_ref().unwrap().module.contributions[0];
    let grant = &grants[0];
    for invalid in [
        GrantContributionPermission {
            resource_scope: ContributionResourceScope::Workspace,
            ..grant.clone()
        },
        GrantContributionPermission {
            permission_contract_id: "managed-service".into(),
            ..grant.clone()
        },
        GrantContributionPermission {
            permission_contract_version: "2".into(),
            ..grant.clone()
        },
        GrantContributionPermission {
            permission: "credential.manage".into(),
            ..grant.clone()
        },
    ] {
        assert!(!policy.admits(&installation, contribution, &invalid));
    }
    let mut workspace = installation;
    workspace.scope_id = domain::DEFAULT_SCOPE_ID;
    workspace.metadata_json = json!({"binding_targets":["workspace"]});
    assert!(policy
        .declared_activation_grants(&workspace, &manifest)
        .is_err());
}

#[test]
fn authentication_effective_authority_rejects_missing_revoked_and_wrong_contribution_grants() {
    let (installation, manifest) = authentication_fixture();
    let policy = HostContributionGrantPolicy::root_composition();
    let contribution = &manifest.managed.as_ref().unwrap().module.contributions[0];
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    let mut snapshot = domain::PluginContributionAuthoritySnapshot {
        installation_id: installation.id,
        workspace_id: domain::SYSTEM_SCOPE_ID,
        revision: 1,
        authorizations: grants
            .iter()
            .map(|grant| domain::PluginContributionAuthorization {
                id: Uuid::now_v7(),
                installation_id: installation.id,
                workspace_id: domain::SYSTEM_SCOPE_ID,
                contribution_id: grant.contribution_id.clone(),
                point_id: contribution.point_id.as_str().into(),
                permission: grant.permission.clone(),
                resource_scope: grant.resource_scope.clone(),
                permission_contract_id: grant.permission_contract_id.clone(),
                permission_contract_version: grant.permission_contract_version.clone(),
                status: domain::ContributionAuthorizationStatus::Active,
                granted_by: Uuid::nil(),
                revoked_by: None,
                granted_at: time::OffsetDateTime::now_utc(),
                revoked_at: None,
                revision: 1,
            })
            .collect(),
    };
    assert_eq!(
        policy
            .effective_permissions(
                &installation,
                domain::SYSTEM_SCOPE_ID,
                contribution,
                &snapshot
            )
            .unwrap()
            .len(),
        2
    );
    // Public OAuth has no user actor: only the exact active installed authentication
    // contribution can obtain the Host HTTP service, and both grants remain live.
    let binding = runtime_core::runtime_backend::HttpEgressBinding {
        subject: extension_contracts::ManagedContributionSubject::new(
            extension_contracts::ManagedInstallationId::new(installation.id.to_string()).unwrap(),
            extension_contracts::ManagedWorkspaceId::new(domain::SYSTEM_SCOPE_ID.to_string())
                .unwrap(),
            contribution.contribution_id.clone(),
        ),
        publisher_namespace: installation.organization.clone(),
        plugin_code: installation.provider_code.clone(),
        plugin_version: installation.plugin_version.clone(),
        caller: None,
        deadline_unix_ms: i64::MAX,
    };
    let mut active = installation.clone();
    active.desired_state = domain::PluginDesiredState::ActiveRequested;
    active.metadata_json["managed"] =
        serde_json::to_value(manifest.managed.as_ref().unwrap()).unwrap();
    let validate = crate::plugin_management::authentication_egress::validate_authentication_egress;
    assert!(validate(&binding, &active, &snapshot).is_ok());
    let mut disabled = active.clone();
    disabled.desired_state = domain::PluginDesiredState::Disabled;
    assert!(validate(&binding, &disabled, &snapshot).is_err());
    let mut wrong_version = binding.clone();
    wrong_version.plugin_version = "99.0.0".into();
    assert!(validate(&wrong_version, &active, &snapshot).is_err());
    let mut expired = binding.clone();
    expired.deadline_unix_ms = 0;
    assert!(validate(&expired, &active, &snapshot).is_err());
    let mut revoked = snapshot.clone();
    revoked.authorizations[0].status = domain::ContributionAuthorizationStatus::Revoked;
    assert!(validate(&binding, &active, &revoked).is_err());
    snapshot.authorizations[0].status = domain::ContributionAuthorizationStatus::Revoked;
    assert!(policy
        .effective_permissions(
            &installation,
            domain::SYSTEM_SCOPE_ID,
            contribution,
            &snapshot
        )
        .is_err());
    snapshot.authorizations[0].status = domain::ContributionAuthorizationStatus::Active;
    snapshot.authorizations[0].contribution_id = "different.auth".into();
    assert!(policy
        .effective_permissions(
            &installation,
            domain::SYSTEM_SCOPE_ID,
            contribution,
            &snapshot
        )
        .is_err());
    snapshot.authorizations.clear();
    assert!(policy
        .effective_permissions(
            &installation,
            domain::SYSTEM_SCOPE_ID,
            contribution,
            &snapshot
        )
        .is_err());
}

#[test]
fn communication_receive_activation_grants_only_its_declared_workspace_contract() {
    let (mut installation, mut manifest) = fixture();
    installation.scope_id = domain::DEFAULT_SCOPE_ID;
    installation.metadata_json = json!({"binding_targets":["workspace"]});
    manifest.data_models.clear();
    manifest.managed.as_mut().unwrap().module.contributions = vec![serde_json::from_value(json!({
        "contribution_id":"managed_fixture.receive", "contributor_module_id":"managed_fixture",
        "point_id":extension_contracts::NOTIFICATION_RECEIVE_POINT,
        "contract_version":extension_contracts::NOTIFICATION_CONTRACT_V1, "mode":"append",
        "required_permissions":[extension_contracts::NOTIFICATION_RECEIVE_PERMISSION]
    }))
    .unwrap()];
    let policy = HostContributionGrantPolicy::root_composition();
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(
        grants[0].permission,
        extension_contracts::NOTIFICATION_RECEIVE_PERMISSION
    );
    assert_eq!(
        grants[0].resource_scope,
        ContributionResourceScope::Workspace
    );
    let contribution = &manifest.managed.as_ref().unwrap().module.contributions[0];
    for invalid in [
        GrantContributionPermission {
            resource_scope: ContributionResourceScope::System,
            ..grants[0].clone()
        },
        GrantContributionPermission {
            permission: extension_contracts::NOTIFICATION_SEND_PERMISSION.into(),
            ..grants[0].clone()
        },
        GrantContributionPermission {
            permission_contract_id: "managed-service".into(),
            ..grants[0].clone()
        },
        GrantContributionPermission {
            permission_contract_version: "2".into(),
            ..grants[0].clone()
        },
    ] {
        assert!(!policy.admits(&installation, contribution, &invalid));
    }
    // Controlled pre-fix host ceiling must reject the same declared receive contribution.
    let mut without_receive = HostContributionGrantPolicy::root_composition();
    without_receive
        .rules
        .retain(|rule| rule.point_id != extension_contracts::NOTIFICATION_RECEIVE_POINT);
    assert!(without_receive
        .declared_activation_grants(&installation, &manifest)
        .is_err());
}
