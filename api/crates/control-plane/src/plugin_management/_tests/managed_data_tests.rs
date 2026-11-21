use super::*;
use control_plane_contracts::ports::*;
use domain::{AuditLogRecord, PluginContributionAuthorization};
use extension_contracts::{extension_bus::*, PluginDataOperationResult, PluginDataResponse};
use serde_json::json;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

type TestFuture<T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>>;
struct State {
    providers: std::collections::BTreeMap<Uuid, PluginInstallationRecord>,
    installation: PluginInstallationRecord,
    snapshot: PluginContributionAuthoritySnapshot,
}
struct Authority {
    state: Arc<tokio::sync::Mutex<State>>,
    held: Arc<AtomicBool>,
    locks: AtomicUsize,
}
struct Lease {
    installations: std::collections::BTreeMap<Uuid, PluginInstallationRecord>,
    state: tokio::sync::OwnedMutexGuard<State>,
    held: Arc<AtomicBool>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.held.store(false, Ordering::SeqCst);
    }
}
impl ContributionAuthorityLease for Lease {
    fn snapshot(&self) -> &PluginContributionAuthoritySnapshot {
        &self.state.snapshot
    }
    fn snapshots(&self) -> &[PluginContributionAuthoritySnapshot] {
        std::slice::from_ref(&self.state.snapshot)
    }
    fn installation(&self, id: Uuid) -> Option<&PluginInstallationRecord> {
        (id == self.state.installation.id).then_some(&self.state.installation)
    }
    fn installations(&self) -> &std::collections::BTreeMap<Uuid, PluginInstallationRecord> {
        &self.installations
    }
    fn load_communication_command_context<'a>(
        &'a mut self,
        _: &'a ManagedContributionSubject,
        _: Uuid,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = anyhow::Result<
                        Option<
                            control_plane_contracts::ports::CommunicationCommandValidationContext,
                        >,
                    >,
                > + Send
                + 'a,
        >,
    > {
        panic!("unexpected command context read")
    }
    fn commit_communication_command_reply(
        self: Box<Self>,
        _: control_plane_contracts::ports::CompleteManagedCommunicationCommand,
    ) -> TestFuture<control_plane_contracts::ports::CommunicationCommandRecord> {
        panic!("unexpected command commit")
    }
    fn commit_owned_event_effect(
        self: Box<Self>,
        _: ManagedContributionSubject,
        _: Uuid,
        _: String,
        _: Vec<PluginDataOperation>,
        _: i64,
    ) -> TestFuture<PluginDataResponse> {
        panic!("unexpected event commit")
    }
    fn commit_derived_lifecycle_fact(
        self: Box<Self>,
        _: RecordLifecycleFactInput,
        _: FrozenLifecyclePublication,
    ) -> TestFuture<LifecycleOutboxRecord> {
        panic!("unexpected event commit")
    }
    fn commit_resume_managed_delivery(
        self: Box<Self>,
        _: ResumeManagedLifecycleDelivery,
    ) -> TestFuture<LifecycleOutboxRecord> {
        panic!("unexpected resume")
    }
    fn commit_managed_settings_templates(
        self: Box<Self>,
        installation_ids: Vec<Uuid>,
        catalog: Option<control_plane_contracts::CompiledConsolePolicyCatalog>,
        _event_catalog: Option<domain::event_center::EventCatalogSnapshot>,
    ) -> TestFuture<()> {
        Box::pin(async move {
            drop(self);
            anyhow::ensure!(
                installation_ids.is_empty() && catalog.is_none() && _event_catalog.is_none(),
                "settings templates are not supported by this fixture"
            );
            Ok(())
        })
    }
    fn release(self: Box<Self>) -> TestFuture<()> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }
}
#[async_trait::async_trait]
impl PluginContributionAuthorityRepository for Authority {
    async fn lock_managed_installation_switch(
        &self,
        _: &ManagedInstallationSwitch,
    ) -> anyhow::Result<Box<dyn ManagedInstallationSwitchLease>> {
        panic!("unexpected switch")
    }
    async fn lock_contribution_authority_batch(
        &self,
        _: &[(Uuid, Uuid)],
    ) -> anyhow::Result<Box<dyn ContributionAuthorityLease>> {
        panic!("unexpected batch")
    }
    async fn contribution_authority_workspaces(&self, _: Uuid) -> anyhow::Result<Vec<Uuid>> {
        panic!("unexpected workspace query")
    }
    async fn lock_installation_contribution_authority(
        &self,
        _: Uuid,
        _: Uuid,
    ) -> anyhow::Result<Box<dyn ContributionAuthorityLease>> {
        panic!("unexpected installation lock")
    }
    async fn grant_contribution_authorization(
        &self,
        _: &GrantContributionAuthorizationInput,
    ) -> anyhow::Result<PluginContributionAuthoritySnapshot> {
        panic!("unexpected grant")
    }
    async fn revoke_contribution_authorization(
        &self,
        _: &RevokeContributionAuthorizationInput,
    ) -> anyhow::Result<PluginContributionAuthoritySnapshot> {
        panic!("unexpected revoke")
    }
    async fn query_contribution_authority(
        &self,
        _: Uuid,
        _: Uuid,
        _: &AuditLogRecord,
    ) -> anyhow::Result<PluginContributionAuthoritySnapshot> {
        panic!("unexpected query")
    }
    async fn lock_contribution_authority(
        &self,
        subject: &ManagedContributionSubject,
    ) -> anyhow::Result<Box<dyn ContributionAuthorityLease>> {
        self.locks.fetch_add(1, Ordering::SeqCst);
        let state = self.state.clone().lock_owned().await;
        assert_eq!(
            subject.installation_id().as_str(),
            state.installation.id.to_string()
        );
        self.held.store(true, Ordering::SeqCst);
        let mut installations = state.providers.clone();
        installations.insert(state.installation.id, state.installation.clone());
        Ok(Box::new(Lease {
            installations,
            state,
            held: self.held.clone(),
        }))
    }
}
struct Inner {
    held: Arc<AtomicBool>,
    calls: AtomicUsize,
}
impl PluginDataPort for Inner {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        _: &'a PluginDataRequest,
    ) -> PluginDataFuture<'a> {
        Box::pin(async move {
            assert_eq!(
                self.held.load(Ordering::SeqCst),
                binding.managed_subject.is_some()
            );
            tokio::task::yield_now().await;
            assert_eq!(
                self.held.load(Ordering::SeqCst),
                binding.managed_subject.is_some()
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(PluginDataResponse {
                results: vec![PluginDataOperationResult::Count { count: 7 }],
                replayed: false,
            })
        })
    }
}
fn fixture() -> (State, PluginDataBinding, PluginDataRequest) {
    let id = Uuid::now_v7();
    let scope = domain::SYSTEM_SCOPE_ID;
    let now = time::OffsetDateTime::now_utc();
    let installation = PluginInstallationRecord {
        id,
        scope_id: scope,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "acme".into(),
        provider_code: "ssh".into(),
        plugin_id: "ssh@1.0.0".into(),
        plugin_version: "1.0.0".into(),
        contract_version: "1flowbase.extension-bus/v1".into(),
        protocol: "stdio_json_multiplex_v1".into(),
        display_name: "SSH fixture".into(),
        source_kind: "uploaded".into(),
        trust_level: "unverified".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({"managed_service":{"scope":"system"},"managed_service_permissions":{"storage":"host_managed"},"managed":{
            "module":{"bus_version":"v1","module_id":"ssh","module_version":"1.0.0","module_kind":"runtime","contributions":[{
                "contribution_id":"ssh.list", "contributor_module_id":"ssh", "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append", "required_permissions":["service.execute","plugin_data.owned.write"]
            }]},"execution_bindings":[]
        }}),
        is_system_reserved: false,
        created_by: id,
        updated_by: None,
        created_at: now,
        updated_at: now,
    };
    let grant =
        |permission: &str, contract: &str, resource_scope| PluginContributionAuthorization {
            id: Uuid::now_v7(),
            installation_id: id,
            workspace_id: scope,
            contribution_id: "ssh.list".into(),
            point_id: plugin_framework::MANAGED_SERVICE_POINT.into(),
            permission: permission.into(),
            resource_scope,
            permission_contract_id: contract.into(),
            permission_contract_version: "1".into(),
            status: ContributionAuthorizationStatus::Active,
            granted_by: id,
            revoked_by: None,
            granted_at: now,
            revoked_at: None,
            revision: 1,
        };
    let snapshot = PluginContributionAuthoritySnapshot {
        installation_id: id,
        workspace_id: scope,
        revision: 1,
        authorizations: vec![
            grant(
                "service.execute",
                "managed-service",
                ContributionResourceScope::System,
            ),
            grant(
                "plugin_data.owned.write",
                "plugin-data",
                ContributionResourceScope::OwnedCollection {
                    collection_code: "connections".into(),
                },
            ),
        ],
    };
    let binding = PluginDataBinding {
        record_permissions: None,
        managed_subject: Some(ManagedContributionSubject::new(
            ManagedInstallationId::new(id.to_string()).unwrap(),
            ManagedWorkspaceId::new(scope.to_string()).unwrap(),
            ContributionId::new("ssh.list").unwrap(),
        )),
        publisher_namespace: "acme".into(),
        plugin_code: "ssh".into(),
        plugin_version: "1.0.0".into(),
        storage_binding: "main".into(),
        workspace_id: scope.to_string(),
        actor_id: None,
        provider_instance_id: id.to_string(),
        permissions: [PluginDataPermission::Read, PluginDataPermission::Write].into(),
        deadline_unix_ms: (now.unix_timestamp_nanos() / 1_000_000) as i64 + 60_000,
    };
    let request = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Count {
            target: PluginDataTarget::OwnedCollection {
                collection_code: "connections".into(),
            },
            filters: vec![],
        }],
    };
    (
        State {
            providers: Default::default(),
            installation,
            snapshot,
        },
        binding,
        request,
    )
}
fn add_provider(state: &mut State, access: &str) -> Uuid {
    let mut provider = state.installation.clone();
    provider.id = Uuid::now_v7();
    provider.organization = "original".into();
    provider.provider_code = "shared_hosts".into();
    provider.plugin_version = "2.4.0".into();
    provider.metadata_json = json!({
        "managed_service":{"scope":"system"},
        "resource_exports":[{"resource_code":"hosts", "collections":[{"collection_code":"connections","access":access}], "credentials":access}],
        "data_models":[{"contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main", "owned_collections":[{"collection_code":"connections","fields":[{"field_code":"name","field_type":"string"}]}]}]
    });
    state.installation.metadata_json["plugin_dependencies"] =
        json!([{"publisher_namespace":"original","plugin_code":"shared_hosts"}]);
    state.installation.metadata_json["resource_imports"] = json!([{
        "provider":{"publisher_namespace":"original","plugin_code":"shared_hosts"}, "resource_code":"hosts",
        "collections":[{"collection_code":"connections","access":access}], "credentials":access
    }]);
    let id = provider.id;
    state.providers.insert(id, provider);
    id
}
#[tokio::test]
async fn managed_callbacks_refresh_authority_hold_lease_and_preserve_legacy_path() {
    let (state, mut binding, request) = fixture();
    let authority = Arc::new(Authority {
        state: Arc::new(tokio::sync::Mutex::new(state)),
        held: Arc::new(AtomicBool::new(false)),
        locks: AtomicUsize::new(0),
    });
    let inner = Arc::new(Inner {
        held: authority.held.clone(),
        calls: AtomicUsize::new(0),
    });
    let service = ManagedPluginDataService::new(authority.clone(), inner.clone());
    assert_eq!(
        service.execute(&binding, &request).await.unwrap().results,
        vec![PluginDataOperationResult::Count { count: 7 }]
    );
    assert!(!authority.held.load(Ordering::SeqCst));
    authority.state.lock().await.snapshot.authorizations[1].status =
        ContributionAuthorizationStatus::Revoked;
    assert_eq!(
        service.execute(&binding, &request).await.unwrap_err().kind,
        PluginDataErrorKind::PermissionDenied
    );
    assert_eq!(inner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(authority.locks.load(Ordering::SeqCst), 2);
    binding.managed_subject = None;
    service.execute(&binding, &request).await.unwrap();
    assert_eq!(authority.locks.load(Ordering::SeqCst), 2);
    assert_eq!(inner.calls.load(Ordering::SeqCst), 2);
    binding.managed_subject = fixture().1.managed_subject;
    binding.deadline_unix_ms = 0;
    assert_eq!(
        service.execute(&binding, &request).await.unwrap_err().kind,
        PluginDataErrorKind::DeadlineExceeded
    );
    assert_eq!(authority.locks.load(Ordering::SeqCst), 2);
}
#[test]
fn managed_data_rejects_stale_identity_storage_and_collection_grants() {
    for mutation in [
        "disabled",
        "version",
        "publisher",
        "plugin",
        "scope",
        "storage",
        "collection",
        "revoked",
        "permission",
        "projection",
        "contribution",
        "grant_installation",
        "grant_workspace",
        "grant_contract",
        "grant_version",
    ] {
        let (mut state, mut binding, mut request) = fixture();
        assert!(
            validate_authority(&binding, &request, &state.installation, &state.snapshot).is_ok()
        );
        match mutation {
            "disabled" => state.installation.desired_state = PluginDesiredState::Disabled,
            "version" => state.installation.plugin_version = "2.0.0".into(),
            "publisher" => state.installation.organization = "other".into(),
            "plugin" => state.installation.provider_code = "other".into(),
            "scope" => binding.workspace_id = Uuid::now_v7().to_string(),
            "storage" => {
                state.installation.metadata_json["managed_service_permissions"]["storage"] =
                    json!("none")
            }
            "collection" => {
                state.snapshot.authorizations[1].resource_scope =
                    ContributionResourceScope::OwnedCollection {
                        collection_code: "other".into(),
                    }
            }
            "revoked" => {
                state.snapshot.authorizations[1].status = ContributionAuthorizationStatus::Revoked
            }
            "permission" => binding.permissions.clear(),
            "projection" => {
                request.operations = vec![PluginDataOperation::Count {
                    target: PluginDataTarget::ExtensionProjection {
                        target_table: "users".into(),
                    },
                    filters: vec![],
                }]
            }
            "contribution" => state.snapshot.authorizations[1].contribution_id = "ssh.other".into(),
            "grant_installation" => {
                state.snapshot.authorizations[1].installation_id = Uuid::now_v7()
            }
            "grant_workspace" => state.snapshot.authorizations[1].workspace_id = Uuid::now_v7(),
            "grant_contract" => {
                state.snapshot.authorizations[1].permission_contract_id = "provider".into()
            }
            "grant_version" => {
                state.snapshot.authorizations[1].permission_contract_version = "2".into()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_authority(&binding, &request, &state.installation, &state.snapshot)
                .unwrap_err()
                .kind,
            PluginDataErrorKind::PermissionDenied,
            "{mutation}"
        );
    }
}

#[tokio::test]
async fn shared_callbacks_remap_only_after_live_caller_grants_and_hold_original_lease() {
    struct SharedInner {
        held: Arc<AtomicBool>,
        calls: AtomicUsize,
        original_subject: ManagedContributionSubject,
    }
    impl PluginDataPort for SharedInner {
        fn execute<'a>(
            &'a self,
            binding: &'a PluginDataBinding,
            _: &'a PluginDataRequest,
        ) -> PluginDataFuture<'a> {
            Box::pin(async move {
                assert!(self.held.load(Ordering::SeqCst));
                assert_eq!(binding.publisher_namespace, "original");
                assert_eq!(binding.plugin_code, "shared_hosts");
                assert_eq!(binding.plugin_version, "2.4.0");
                assert_eq!(
                    binding.managed_subject.as_ref(),
                    Some(&self.original_subject)
                );
                tokio::task::yield_now().await;
                assert!(self.held.load(Ordering::SeqCst));
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(PluginDataResponse {
                    results: vec![],
                    replayed: false,
                })
            })
        }
    }
    let (mut state, binding, request) = fixture();
    add_provider(&mut state, "read_write");
    let authority = Arc::new(Authority {
        state: Arc::new(tokio::sync::Mutex::new(state)),
        held: Arc::new(AtomicBool::new(false)),
        locks: AtomicUsize::new(0),
    });
    let inner = Arc::new(SharedInner {
        held: authority.held.clone(),
        calls: AtomicUsize::new(0),
        original_subject: binding.managed_subject.clone().unwrap(),
    });
    let service = ManagedPluginDataService::new(authority.clone(), inner.clone());
    service.execute(&binding, &request).await.unwrap();
    assert!(!authority.held.load(Ordering::SeqCst));
    authority.state.lock().await.snapshot.authorizations[1].status =
        ContributionAuthorizationStatus::Revoked;
    assert!(service.execute(&binding, &request).await.is_err());
    authority.state.lock().await.snapshot.authorizations[1].status =
        ContributionAuthorizationStatus::Active;
    authority.state.lock().await.installation.desired_state = PluginDesiredState::Disabled;
    assert!(service.execute(&binding, &request).await.is_err());
    authority.state.lock().await.installation.desired_state = PluginDesiredState::ActiveRequested;
    authority
        .state
        .lock()
        .await
        .providers
        .values_mut()
        .next()
        .unwrap()
        .desired_state = PluginDesiredState::Disabled;
    assert!(service.execute(&binding, &request).await.is_err());
    assert_eq!(inner.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shared_credentials_keep_live_consumer_authority_and_read_existing_owner_credentials() {
    use crate::plugin_management::ManagedPluginCredentialService;
    use extension_contracts::{
        PluginCredentialBinding, PluginCredentialError, PluginCredentialPort,
        PluginCredentialRequest, PluginCredentialResponse,
    };
    struct ExistingCredentials {
        held: Arc<AtomicBool>,
        calls: AtomicUsize,
    }
    #[async_trait::async_trait]
    impl PluginCredentialRepository for ExistingCredentials {
        async fn execute_plugin_credential(
            &self,
            binding: &PluginCredentialBinding,
            request: &PluginCredentialRequest,
        ) -> Result<PluginCredentialResponse, PluginCredentialError> {
            assert!(self.held.load(Ordering::SeqCst));
            assert_eq!(binding.publisher_namespace, "original");
            assert_eq!(binding.plugin_code, "shared_hosts");
            assert_eq!(binding.plugin_version, "2.4.0");
            assert_eq!(binding.contribution_id, "ssh.list");
            assert_eq!(binding.scope_id, domain::SYSTEM_SCOPE_ID.to_string());
            assert_eq!(request.credential_id(), "existing-host");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(PluginCredentialResponse {
                value: Some(json!({"password":"existing-owner-secret"})),
            })
        }
    }
    let (mut state, data_binding, _) = fixture();
    add_provider(&mut state, "read_write");
    state.installation.metadata_json["managed_service_permissions"]["secrets"] =
        json!("host_managed");
    state.installation.metadata_json["managed"]["module"]["contributions"][0]
        ["required_permissions"] = json!(["service.execute", "credential.manage"]);
    state.snapshot.authorizations[1].permission = "credential.manage".into();
    state.snapshot.authorizations[1].permission_contract_id = "plugin-credential".into();
    state.snapshot.authorizations[1].resource_scope = ContributionResourceScope::System;
    let binding = PluginCredentialBinding {
        installation_id: state.installation.id.to_string(),
        contribution_id: "ssh.list".into(),
        publisher_namespace: data_binding.publisher_namespace,
        plugin_code: data_binding.plugin_code,
        plugin_version: data_binding.plugin_version,
        scope_id: data_binding.workspace_id,
        deadline_unix_ms: data_binding.deadline_unix_ms,
    };
    let authority = Arc::new(Authority {
        state: Arc::new(tokio::sync::Mutex::new(state)),
        held: Arc::new(AtomicBool::new(false)),
        locks: AtomicUsize::new(0),
    });
    let repository = Arc::new(ExistingCredentials {
        held: authority.held.clone(),
        calls: AtomicUsize::new(0),
    });
    let service = ManagedPluginCredentialService::new(authority.clone(), repository.clone());
    let get = PluginCredentialRequest::Get {
        credential_id: "existing-host".into(),
    };
    assert_eq!(
        service.execute(&binding, &get).await.unwrap().value,
        Some(json!({"password":"existing-owner-secret"}))
    );
    assert!(!authority.held.load(Ordering::SeqCst));
    authority.state.lock().await.snapshot.authorizations[1].status =
        ContributionAuthorizationStatus::Revoked;
    assert!(service.execute(&binding, &get).await.is_err());
    authority.state.lock().await.snapshot.authorizations[1].status =
        ContributionAuthorizationStatus::Active;
    authority.state.lock().await.installation.desired_state = PluginDesiredState::Disabled;
    assert!(service.execute(&binding, &get).await.is_err());
    authority.state.lock().await.installation.desired_state = PluginDesiredState::ActiveRequested;
    authority.state.lock().await.installation.metadata_json["resource_imports"][0]["credentials"] =
        json!("read_only");
    assert!(service.execute(&binding, &get).await.is_ok());
    for write in [
        PluginCredentialRequest::Put {
            credential_id: "existing-host".into(),
            value: json!({"password":"replacement"}),
        },
        PluginCredentialRequest::Delete {
            credential_id: "existing-host".into(),
        },
    ] {
        assert!(service.execute(&binding, &write).await.is_err());
    }
    assert_eq!(repository.calls.load(Ordering::SeqCst), 2);
    authority
        .state
        .lock()
        .await
        .providers
        .values_mut()
        .next()
        .unwrap()
        .desired_state = PluginDesiredState::Disabled;
    assert!(service.execute(&binding, &get).await.is_err());
    authority.state.lock().await.providers.clear();
    assert!(service.execute(&binding, &get).await.is_err());
    assert_eq!(repository.calls.load(Ordering::SeqCst), 2);
}
