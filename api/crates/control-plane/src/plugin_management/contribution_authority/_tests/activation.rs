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
        "required_permissions":["service.execute", "credential.manage", "ephemeral.manage", "plugin_data.owned.write"]
    }))
    .unwrap()];
    manifest.data_models = vec![
        serde_json::from_value(json!({
            "contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
            "owned_collections":[{"collection_code":"connections","fields":[]}]
        }))
        .unwrap(),
    ];
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
    assert_eq!(grants.len(), 4);
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "service.execute"
                && grant.resource_scope == ContributionResourceScope::System)
    );
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "credential.manage"
                && grant.permission_contract_id == "plugin-credential")
    );
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "ephemeral.manage"
                && grant.permission_contract_id == "plugin-ephemeral")
    );
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "plugin_data.owned.write"
                && grant.resource_scope
                    == ContributionResourceScope::OwnedCollection {
                        collection_code: "connections".into()
                    })
    );
}

#[test]
fn root_activation_does_not_invent_collections_or_override_host_capability_contracts() {
    let (mut installation, mut manifest) = fixture();
    let policy = HostContributionGrantPolicy::root_composition();
    manifest.data_models.clear();
    assert!(
        policy
            .declared_activation_grants(&installation, &manifest)
            .is_err()
    );
    let (_, declared) = fixture();
    installation.metadata_json["managed_service_permissions"]["secrets"] = json!("none");
    assert!(
        policy
            .declared_activation_grants(&installation, &declared)
            .is_err()
    );
}

fn authentication_fixture() -> (
    domain::PluginInstallationRecord,
    plugin_framework::PluginManifestV1,
) {
    let (mut installation, mut manifest) = fixture();
    installation.metadata_json = json!({"binding_targets":["system"]});
    manifest.data_models.clear();
    manifest.managed.as_mut().unwrap().module.contributions =
        vec![serde_json::from_value(json!({
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
    assert!(
        grants
            .iter()
            .all(|grant| grant.resource_scope == ContributionResourceScope::System)
    );
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "authentication.verify"
                && grant.permission_contract_id == "authentication")
    );
    assert!(
        grants
            .iter()
            .any(|grant| grant.permission == "network.http.request"
                && grant.permission_contract_id == "http-egress")
    );
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
    assert!(
        policy
            .declared_activation_grants(&workspace, &manifest)
            .is_err()
    );
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
    assert!(
        policy
            .effective_permissions(
                &installation,
                domain::SYSTEM_SCOPE_ID,
                contribution,
                &snapshot
            )
            .is_err()
    );
    snapshot.authorizations[0].status = domain::ContributionAuthorizationStatus::Active;
    snapshot.authorizations[0].contribution_id = "different.auth".into();
    assert!(
        policy
            .effective_permissions(
                &installation,
                domain::SYSTEM_SCOPE_ID,
                contribution,
                &snapshot
            )
            .is_err()
    );
    snapshot.authorizations.clear();
    assert!(
        policy
            .effective_permissions(
                &installation,
                domain::SYSTEM_SCOPE_ID,
                contribution,
                &snapshot
            )
            .is_err()
    );
}

#[test]
fn communication_receive_activation_grants_only_its_declared_workspace_contract() {
    let (mut installation, mut manifest) = fixture();
    installation.scope_id = domain::DEFAULT_SCOPE_ID;
    installation.metadata_json = json!({"binding_targets":["workspace"]});
    manifest.data_models.clear();
    manifest.managed.as_mut().unwrap().module.contributions =
        vec![serde_json::from_value(json!({
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
    assert!(
        without_receive
            .declared_activation_grants(&installation, &manifest)
            .is_err()
    );
}

#[test]
fn frontend_activation_delegates_only_exact_trusted_host_workspace_contract() {
    use crate::frontend_block_catalog::*;
    let (mut installation, mut manifest) = fixture();
    installation.scope_id = domain::DEFAULT_SCOPE_ID;
    installation.metadata_json = json!({"binding_targets": ["workspace"]});
    manifest.data_models.clear();
    manifest.managed.as_mut().unwrap().module.contributions = vec![serde_json::from_value(json!({
        "contribution_id": "managed_fixture.frontend", "contributor_module_id": "managed_fixture",
        "point_id": FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, "contract_version": "1", "mode": "append",
        "required_permissions": [FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION]
    }))
    .unwrap()];
    let policy = HostContributionGrantPolicy::root_composition();
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    assert_eq!(grants.len(), 1);
    let grant = &grants[0];
    assert_eq!(grant.permission, FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION);
    assert_eq!(
        grant.permission_contract_id,
        FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID
    );
    assert_eq!(grant.permission_contract_version, "1");
    assert_eq!(grant.resource_scope, ContributionResourceScope::Workspace);
    let contribution = &manifest.managed.as_ref().unwrap().module.contributions[0];
    for permission in [
        "*",
        "frontend-block.ui-mount.*",
        FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
    ] {
        let mut denied = grant.clone();
        denied.permission = permission.into();
        assert!(!policy.admits(&installation, contribution, &denied));
    }
    for scope in [
        ContributionResourceScope::System,
        ContributionResourceScope::OwnedCollection {
            collection_code: "views".into(),
        },
    ] {
        let mut denied = grant.clone();
        denied.resource_scope = scope;
        assert!(!policy.admits(&installation, contribution, &denied));
    }
    let mut denied = grant.clone();
    denied.permission_contract_id = "managed-hook".into();
    assert!(!policy.admits(&installation, contribution, &denied));
    denied = grant.clone();
    denied.permission_contract_version = "2".into();
    assert!(!policy.admits(&installation, contribution, &denied));
    for (point, version) in [
        ("acme.frontend", "1"),
        ("1flowbase.workspace.frontend-block-contribution.extra", "1"),
        (FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, "2"),
    ] {
        let mut wrong_manifest = manifest.clone();
        let descriptor = &mut wrong_manifest
            .managed
            .as_mut()
            .unwrap()
            .module
            .contributions[0];
        descriptor.point_id =
            plugin_framework::extension_bus::ExtensionPointId::new(point).unwrap();
        descriptor.contract_version =
            plugin_framework::extension_bus::ContractVersion::new(version).unwrap();
        assert!(
            policy
                .declared_activation_grants(&installation, &wrong_manifest)
                .is_err()
        );
    }
    let mut old_ceiling = policy.clone();
    old_ceiling
        .rules
        .retain(|rule| rule.point_id != FRONTEND_BLOCK_CONTRIBUTION_POINT_ID);
    assert_ne!(
        policy.identity(),
        old_ceiling.identity(),
        "compiled ceiling changes must invalidate authority fingerprints"
    );
    assert!(
        old_ceiling
            .declared_activation_grants(&installation, &manifest)
            .is_err(),
        "regression: QA1 ceiling rejected real frontend enable"
    );
}

#[test]
fn execution_gate_is_explicitly_delegated_without_secret_management_authority() {
    let (mut installation, mut manifest) = fixture();
    let contribution = &mut manifest.managed.as_mut().unwrap().module.contributions[0];
    contribution.required_permissions = [
        plugin_framework::extension_bus::PermissionCode::new("service.execute").unwrap(),
        plugin_framework::extension_bus::PermissionCode::new("execution.gate").unwrap(),
    ]
    .into();
    installation.metadata_json["managed_service_permissions"]["secrets"] = json!("none");
    let policy = HostContributionGrantPolicy::root_composition();
    let grants = policy
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    assert_eq!(grants.len(), 2);
    let mut gate = grants
        .into_iter()
        .find(|grant| grant.permission == "execution.gate")
        .unwrap();
    assert_eq!(gate.permission_contract_id, "execution-gate");
    assert_eq!(gate.resource_scope, ContributionResourceScope::System);
    gate.permission_contract_id = "plugin-ephemeral".into();
    assert!(!policy.admits(
        &installation,
        &manifest.managed.as_ref().unwrap().module.contributions[0],
        &gate
    ));
}

#[test]
fn root_enable_derives_exact_import_scopes_without_consumer_schema_or_extra_collections() {
    let (installation, mut manifest) = fixture();
    manifest.data_models.clear();
    manifest.plugin_dependencies = vec![extension_contracts::PluginDependency {
        publisher_namespace: "provider".into(),
        plugin_code: "ssh".into(),
    }];
    manifest.resource_imports = vec![serde_json::from_value(json!({"provider":{"publisher_namespace":"provider","plugin_code":"ssh"},"resource_code":"hosts-v1","collections":[{"collection_code":"hosts","access":"read_only"}],"record_resources":[],"credentials":"read_only"})).unwrap()];
    let grants = HostContributionGrantPolicy::root_composition()
        .declared_activation_grants(&installation, &manifest)
        .unwrap();
    let collections = grants
        .iter()
        .filter_map(|g| match &g.resource_scope {
            ContributionResourceScope::OwnedCollection { collection_code } => {
                Some(collection_code.as_str())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(collections, ["hosts"].into_iter().collect());
    assert!(manifest.data_models.is_empty());
}

// Exercise the service method and durable snapshot protocol, not only grant-policy derivation.
mod service_activation_tests {
    use super::*;
    use crate::ports::*;
    use domain::AuditLogRecord;
    use extension_contracts::extension_bus::ManagedContributionSubject;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Repository {
        installation: domain::PluginInstallationRecord,
        state: Arc<Mutex<State>>,
    }
    #[derive(Default)]
    struct State {
        snapshot: Option<PluginContributionAuthoritySnapshot>,
        grants: usize,
    }
    fn repository() -> (Repository, plugin_framework::PluginManifestV1, ActorContext) {
        let (mut installation, manifest) = fixture();
        installation.organization = manifest.publisher_namespace.clone();
        installation.provider_code = manifest.plugin_code().unwrap().to_string();
        installation.metadata_json["managed"] = serde_json::to_value(&manifest.managed).unwrap();
        let actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
        (
            Repository {
                installation,
                state: Arc::new(Mutex::new(State::default())),
            },
            manifest,
            actor,
        )
    }
    fn assert_approval_required(error: anyhow::Error) {
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::PermissionDenied(
                "managed_contribution_authorization_required"
            ))
        ));
    }
    #[async_trait::async_trait]
    impl RoleConsolePolicyReader for Repository {
        async fn load_role_console_policies_for_user(
            &self,
            _: &ActorContext,
        ) -> Result<Vec<domain::RoleConsolePolicy>> {
            panic!("enable authority reuse must not require authorization-view/grant operations")
        }
    }
    #[async_trait::async_trait]
    impl PluginRepository for Repository {
        async fn upsert_installation(
            &self,
            _input: &UpsertPluginInstallationInput,
        ) -> anyhow::Result<domain::PluginInstallationRecord> {
            panic!("unexpected upsert_installation")
        }
        async fn get_installation(
            &self,
            installation_id: Uuid,
        ) -> anyhow::Result<Option<domain::PluginInstallationRecord>> {
            Ok((installation_id == self.installation.id).then(|| self.installation.clone()))
        }
        async fn list_installations(
            &self,
        ) -> anyhow::Result<Vec<domain::PluginInstallationRecord>> {
            panic!("unexpected list_installations")
        }
        async fn upsert_plugin_package_catalog_projection(
            &self,
            _input: &UpsertPluginPackageCatalogProjectionInput,
        ) -> anyhow::Result<domain::PluginPackageCatalogProjectionRecord> {
            panic!("unexpected upsert_plugin_package_catalog_projection")
        }
        async fn get_plugin_package_catalog_projection(
            &self,
            _installation_id: Uuid,
        ) -> anyhow::Result<Option<domain::PluginPackageCatalogProjectionRecord>> {
            panic!("unexpected get_plugin_package_catalog_projection")
        }
        async fn list_plugin_package_catalog_projections(
            &self,
        ) -> anyhow::Result<Vec<domain::PluginPackageCatalogProjectionRecord>> {
            panic!("unexpected list_plugin_package_catalog_projections")
        }
        async fn delete_installation(&self, _installation_id: Uuid) -> anyhow::Result<()> {
            panic!("unexpected delete_installation")
        }
        async fn list_pending_restart_host_extensions(
            &self,
        ) -> anyhow::Result<Vec<domain::PluginInstallationRecord>> {
            panic!("unexpected list_pending_restart_host_extensions")
        }
        async fn update_desired_state(
            &self,
            _input: &UpdatePluginDesiredStateInput,
        ) -> anyhow::Result<domain::PluginInstallationRecord> {
            panic!("unexpected update_desired_state")
        }
        async fn upsert_artifact_instance(
            &self,
            _input: &UpsertPluginArtifactInstanceInput,
        ) -> anyhow::Result<domain::PluginArtifactInstanceRecord> {
            panic!("unexpected upsert_artifact_instance")
        }
        async fn get_artifact_instance(
            &self,
            _node_id: &str,
            _installation_id: Uuid,
        ) -> anyhow::Result<Option<domain::PluginArtifactInstanceRecord>> {
            panic!("unexpected get_artifact_instance")
        }
        async fn list_artifact_instances(
            &self,
            _node_id: &str,
        ) -> anyhow::Result<Vec<domain::PluginArtifactInstanceRecord>> {
            panic!("unexpected list_artifact_instances")
        }
        async fn create_assignment(
            &self,
            _input: &CreatePluginAssignmentInput,
        ) -> anyhow::Result<domain::PluginAssignmentRecord> {
            panic!("unexpected create_assignment")
        }
        async fn list_assignments(
            &self,
            _workspace_id: Uuid,
        ) -> anyhow::Result<Vec<domain::PluginAssignmentRecord>> {
            panic!("unexpected list_assignments")
        }
        async fn list_assigned_installation_ids(&self) -> anyhow::Result<Vec<Uuid>> {
            panic!("unexpected list_assigned_installation_ids")
        }
        async fn create_task(
            &self,
            _input: &CreatePluginTaskInput,
        ) -> anyhow::Result<domain::PluginTaskRecord> {
            panic!("unexpected create_task")
        }
        async fn update_task_status(
            &self,
            _input: &UpdatePluginTaskStatusInput,
        ) -> anyhow::Result<domain::PluginTaskRecord> {
            panic!("unexpected update_task_status")
        }
        async fn get_task(
            &self,
            _task_id: Uuid,
        ) -> anyhow::Result<Option<domain::PluginTaskRecord>> {
            panic!("unexpected get_task")
        }
        async fn list_tasks(&self) -> anyhow::Result<Vec<domain::PluginTaskRecord>> {
            panic!("unexpected list_tasks")
        }
    }
    #[async_trait::async_trait]
    impl PluginContributionAuthorityRepository for Repository {
        async fn lock_managed_installation_switch(
            &self,
            _input: &ManagedInstallationSwitch,
        ) -> Result<Box<dyn ManagedInstallationSwitchLease>> {
            panic!("unexpected lock_managed_installation_switch")
        }
        async fn lock_contribution_authority_batch(
            &self,
            _scopes: &[(Uuid, Uuid)],
        ) -> Result<Box<dyn ContributionAuthorityLease>> {
            panic!("unexpected lock_contribution_authority_batch")
        }
        async fn contribution_authority_workspaces(
            &self,
            _installation_id: Uuid,
        ) -> Result<Vec<Uuid>> {
            panic!("unexpected contribution_authority_workspaces")
        }
        async fn lock_installation_contribution_authority(
            &self,
            _installation_id: Uuid,
            _workspace_id: Uuid,
        ) -> Result<Box<dyn ContributionAuthorityLease>> {
            panic!("unexpected lock_installation_contribution_authority")
        }
        async fn grant_contribution_authorization(
            &self,
            input: &GrantContributionAuthorizationInput,
        ) -> Result<PluginContributionAuthoritySnapshot> {
            assert_eq!(input.installation_id, self.installation.id);
            assert_eq!(input.workspace_id, domain::SYSTEM_SCOPE_ID);
            assert_eq!(
                input.expected_installation_updated_at,
                self.installation.updated_at
            );
            let mut state = self.state.lock().unwrap();
            state.grants += 1;
            let snapshot = state
                .snapshot
                .get_or_insert(PluginContributionAuthoritySnapshot {
                    installation_id: input.installation_id,
                    workspace_id: input.workspace_id,
                    revision: 0,
                    authorizations: Vec::new(),
                });
            snapshot.revision += 1;
            snapshot
                .authorizations
                .push(domain::PluginContributionAuthorization {
                    id: Uuid::now_v7(),
                    installation_id: input.installation_id,
                    workspace_id: input.workspace_id,
                    contribution_id: input.contribution_id.clone(),
                    point_id: input.point_id.clone(),
                    permission: input.permission.clone(),
                    resource_scope: input.resource_scope.clone(),
                    permission_contract_id: input.permission_contract_id.clone(),
                    permission_contract_version: input.permission_contract_version.clone(),
                    status: domain::ContributionAuthorizationStatus::Active,
                    granted_by: input.actor_user_id,
                    revoked_by: None,
                    granted_at: time::OffsetDateTime::now_utc(),
                    revoked_at: None,
                    revision: snapshot.revision,
                });
            Ok(snapshot.clone())
        }
        async fn revoke_contribution_authorization(
            &self,
            input: &RevokeContributionAuthorizationInput,
        ) -> Result<PluginContributionAuthoritySnapshot> {
            let mut state = self.state.lock().unwrap();
            let snapshot = state.snapshot.as_mut().unwrap();
            assert_eq!(snapshot.revision, input.expected_revision);
            snapshot.revision += 1;
            let authorization = snapshot
                .authorizations
                .iter_mut()
                .find(|a| a.id == input.authorization_id)
                .unwrap();
            authorization.status = domain::ContributionAuthorizationStatus::Revoked;
            authorization.revoked_by = Some(input.actor_user_id);
            authorization.revoked_at = Some(time::OffsetDateTime::now_utc());
            authorization.revision = snapshot.revision;
            Ok(snapshot.clone())
        }
        async fn query_contribution_authority(
            &self,
            installation_id: Uuid,
            workspace_id: Uuid,
            audit_log: &AuditLogRecord,
        ) -> Result<PluginContributionAuthoritySnapshot> {
            assert_eq!(installation_id, self.installation.id);
            assert_eq!(workspace_id, domain::SYSTEM_SCOPE_ID);
            let _ = audit_log;
            self.state.lock().unwrap().snapshot.clone().ok_or_else(|| {
                ControlPlaneError::PermissionDenied("contribution_workspace_history_required")
                    .into()
            })
        }
        async fn lock_contribution_authority(
            &self,
            _subject: &ManagedContributionSubject,
        ) -> Result<Box<dyn ContributionAuthorityLease>> {
            panic!("unexpected lock_contribution_authority")
        }
    }

    #[tokio::test]
    async fn explicit_root_enable_grants_once_nonroot_reuses_but_cannot_resurrect_revoke() {
        let (repository, manifest, root) = repository();
        let id = repository.installation.id;
        let service = PluginContributionAuthorityService::new(
            repository.clone(),
            HostContributionGrantPolicy::root_composition(),
        );
        assert!(
            service
                .query(&root, id)
                .await
                .unwrap()
                .authorizations
                .is_empty()
        );
        assert!(repository.state.lock().unwrap().snapshot.is_none());
        assert_eq!(repository.state.lock().unwrap().grants, 0);
        let member = ActorContext::scoped(
            Uuid::now_v7(),
            root.current_workspace_id,
            "member",
            std::iter::empty::<String>(),
        );
        assert_approval_required(
            service
                .prepare_activation_authority(&member, id, &manifest)
                .await
                .unwrap_err(),
        );
        assert_eq!(repository.state.lock().unwrap().grants, 0);
        service
            .prepare_activation_authority(&root, id, &manifest)
            .await
            .unwrap();
        let approved = repository.state.lock().unwrap().snapshot.clone().unwrap();
        assert_eq!(approved.authorizations.len(), 4);
        assert_eq!(repository.state.lock().unwrap().grants, 4);
        service
            .prepare_activation_authority(&root, id, &manifest)
            .await
            .unwrap();
        service
            .prepare_activation_authority(&member, id, &manifest)
            .await
            .unwrap();
        assert_eq!(
            repository.state.lock().unwrap().snapshot,
            Some(approved.clone())
        );
        assert_eq!(repository.state.lock().unwrap().grants, 4);
        service
            .revoke(
                &root,
                id,
                RevokeContributionPermission {
                    authorization_id: approved.authorizations[0].id,
                    expected_revision: approved.revision,
                },
            )
            .await
            .unwrap();
        let revoked = repository.state.lock().unwrap().snapshot.clone();
        assert_approval_required(
            service
                .prepare_activation_authority(&member, id, &manifest)
                .await
                .unwrap_err(),
        );
        assert_eq!(repository.state.lock().unwrap().snapshot, revoked);
        assert_eq!(repository.state.lock().unwrap().grants, 4);
    }

    #[tokio::test]
    async fn nonroot_empty_history_and_manifest_mismatch_never_grant() {
        let (repository, mut manifest, root) = repository();
        let id = repository.installation.id;
        repository.state.lock().unwrap().snapshot = Some(PluginContributionAuthoritySnapshot {
            installation_id: id,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            revision: 0,
            authorizations: Vec::new(),
        });
        let service = PluginContributionAuthorityService::new(
            repository.clone(),
            HostContributionGrantPolicy::root_composition(),
        );
        let member = ActorContext::scoped(
            Uuid::now_v7(),
            root.current_workspace_id,
            "member",
            std::iter::empty::<String>(),
        );
        assert_approval_required(
            service
                .prepare_activation_authority(&member, id, &manifest)
                .await
                .unwrap_err(),
        );
        manifest.version = "9.9.9".into();
        let error = service
            .prepare_activation_authority(&root, id, &manifest)
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::Conflict(
                "managed_contribution_identity_mismatch"
            ))
        ));
        assert_eq!(repository.state.lock().unwrap().grants, 0);
        assert!(
            repository
                .state
                .lock()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .authorizations
                .is_empty()
        );
    }
}
