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
    installation: PluginInstallationRecord,
    snapshot: PluginContributionAuthoritySnapshot,
}
struct Authority {
    state: Arc<tokio::sync::Mutex<State>>,
    held: Arc<AtomicBool>,
    locks: AtomicUsize,
}
struct Lease {
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
        Ok(Box::new(Lease {
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
            installation,
            snapshot,
        },
        binding,
        request,
    )
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
