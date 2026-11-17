use control_plane::plugin_management::ManagedPluginEphemeralService;
use control_plane_contracts::ports::*;
use domain::*;
use domain::{AuditLogRecord, PluginContributionAuthorization};
use extension_contracts::{extension_bus::*, PluginDataOperation, PluginDataResponse};
use extension_contracts::{PluginEphemeralBinding, PluginEphemeralPort, PluginEphemeralRequest};
use serde_json::json;
use std::sync::Arc;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use uuid::Uuid;

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
        Ok(Box::new(Lease {
            state,
            held: self.held.clone(),
        }))
    }
}
fn fixture() -> (State, PluginEphemeralBinding) {
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
        metadata_json: json!({"managed_service":{"scope":"system"},"managed_service_permissions":{"secrets":"host_managed"},"managed":{
            "module":{"bus_version":"v1","module_id":"ssh","module_version":"1.0.0","module_kind":"runtime","contributions":[{
                "contribution_id":"ssh.list", "contributor_module_id":"ssh", "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append", "required_permissions":["service.execute","ephemeral.manage"]
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
                "ephemeral.manage",
                "plugin-ephemeral",
                ContributionResourceScope::System,
            ),
        ],
    };
    let binding = PluginEphemeralBinding {
        installation_id: id.to_string(),
        contribution_id: "ssh.list".into(),
        publisher_namespace: "acme".into(),
        plugin_code: "ssh".into(),
        plugin_version: "1.0.0".into(),
        scope_id: scope.to_string(),
        deadline_unix_ms: (now.unix_timestamp_nanos() / 1_000_000) as i64 + 60_000,
    };
    (
        State {
            installation,
            snapshot,
        },
        binding,
    )
}
fn authority(state: State) -> Arc<Authority> {
    Arc::new(Authority {
        state: Arc::new(tokio::sync::Mutex::new(state)),
        held: Arc::new(AtomicBool::new(false)),
        locks: AtomicUsize::new(0),
    })
}
fn put(ttl_ms: u64) -> PluginEphemeralRequest {
    PluginEphemeralRequest::Put {
        key: "session/token".into(),
        value: json!({"token":"fixture-only"}),
        ttl_ms,
    }
}
fn get() -> PluginEphemeralRequest {
    PluginEphemeralRequest::Get {
        key: "session/token".into(),
    }
}

#[tokio::test]
async fn ephemeral_roundtrip_delete_expiry_and_installation_isolation() {
    let (state, binding) = fixture();
    let authority = authority(state);
    let store = Arc::new(storage_ephemeral::MemoryKvStore::new("plugin-test"));
    let service = ManagedPluginEphemeralService::new(authority.clone(), store.clone());
    service.execute(&binding, &put(60_000)).await.unwrap();
    assert_eq!(
        service.execute(&binding, &get()).await.unwrap().value,
        Some(json!({"token":"fixture-only"}))
    );
    let (other_state, other_binding) = fixture();
    let other_service = ManagedPluginEphemeralService::new(self::authority(other_state), store);
    assert!(other_service
        .execute(&other_binding, &get())
        .await
        .unwrap()
        .value
        .is_none());
    // Delete in another installation cannot touch the first installation's token.
    let delete = PluginEphemeralRequest::Delete {
        key: "session/token".into(),
    };
    other_service
        .execute(&other_binding, &delete)
        .await
        .unwrap();
    assert!(service
        .execute(&binding, &get())
        .await
        .unwrap()
        .value
        .is_some());
    service.execute(&binding, &delete).await.unwrap();
    assert!(service
        .execute(&binding, &get())
        .await
        .unwrap()
        .value
        .is_none());
    service
        .execute(
            &binding,
            &PluginEphemeralRequest::Put {
                key: "session/token".into(),
                value: serde_json::Value::Null,
                ttl_ms: 60_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service.execute(&binding, &get()).await.unwrap().value,
        Some(serde_json::Value::Null)
    );
    service.execute(&binding, &put(1)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(service
        .execute(&binding, &get())
        .await
        .unwrap()
        .value
        .is_none());
    assert!(!authority.held.load(Ordering::SeqCst));
}

#[tokio::test]
async fn ephemeral_revalidates_live_authority_before_get_put_and_delete() {
    for mutation in [
        "disabled",
        "version",
        "publisher",
        "scope",
        "secrets",
        "revoked",
        "missing_grant",
        "contribution",
        "deadline",
    ] {
        let (state, mut binding) = fixture();
        let authority = authority(state);
        let store = Arc::new(storage_ephemeral::MemoryKvStore::new("plugin-test"));
        let service = ManagedPluginEphemeralService::new(authority.clone(), store.clone());
        service.execute(&binding, &put(60_000)).await.unwrap();
        {
            let mut state = authority.state.lock().await;
            match mutation {
                "disabled" => state.installation.desired_state = PluginDesiredState::Disabled,
                "version" => state.installation.plugin_version = "2.0.0".into(),
                "publisher" => state.installation.organization = "other".into(),
                "scope" => binding.scope_id = Uuid::now_v7().to_string(),
                "secrets" => {
                    state.installation.metadata_json["managed_service_permissions"]["secrets"] =
                        json!("none")
                }
                "revoked" => {
                    state.snapshot.authorizations[1].status =
                        ContributionAuthorizationStatus::Revoked
                }
                "missing_grant" => state.snapshot.authorizations.clear(),
                "contribution" => {
                    state.snapshot.authorizations[1].contribution_id = "other.operation".into()
                }
                "deadline" => binding.deadline_unix_ms = 0,
                _ => unreachable!(),
            }
        }
        for request in [
            get(),
            put(60_000),
            PluginEphemeralRequest::Delete {
                key: "session/token".into(),
            },
        ] {
            assert!(
                service.execute(&binding, &request).await.is_err(),
                "{mutation}"
            );
        }
        // Rejected callbacks must leave the existing value untouched.
        let key = format!(
            "plugin_ephemeral:{}:{}:session/token",
            authority.state.lock().await.installation.id,
            domain::SYSTEM_SCOPE_ID
        );
        assert_eq!(
            store.get_json(&key).await.unwrap(),
            Some(json!({"token":"fixture-only"}))
        );
        assert!(!authority.held.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn ephemeral_rejects_unrepresentable_ttl_and_deadline_while_waiting_for_lease() {
    let (state, mut binding) = fixture();
    let authority = authority(state);
    let service = ManagedPluginEphemeralService::new(
        authority.clone(),
        Arc::new(storage_ephemeral::MemoryKvStore::new("plugin-test")),
    );
    for ttl in [0, u64::MAX, i64::MAX as u64] {
        assert!(service.execute(&binding, &put(ttl)).await.is_err());
    }
    let guard = authority.state.lock().await;
    binding.deadline_unix_ms =
        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 10;
    let error = service.execute(&binding, &get()).await.err().unwrap();
    assert_eq!(error.code, "plugin_ephemeral_deadline_exceeded");
    drop(guard);
}

#[tokio::test]
async fn execution_gate_requires_live_declared_current_grant_and_exact_installation_identity() {
    use control_plane::plugin_management::ManagedPluginExecutionGateAuthority;
    use extension_contracts::{PluginExecutionGateAuthority, PluginExecutionGateBinding};
    let (mut state, old) = fixture();
    state.installation.metadata_json["managed"]["module"]["contributions"][0]
        ["required_permissions"] = json!(["execution.gate"]);
    state.installation.metadata_json["managed_service_permissions"]["secrets"] = json!("none");
    let mut grant = state.snapshot.authorizations[0].clone();
    grant.permission = "execution.gate".into();
    grant.permission_contract_id = "execution-gate".into();
    state.snapshot.authorizations = vec![grant];
    let authority = authority(state);
    let service = ManagedPluginExecutionGateAuthority::new(authority.clone());
    let binding = PluginExecutionGateBinding {
        subject: ManagedContributionSubject::new(
            ManagedInstallationId::new(&old.installation_id).unwrap(),
            ManagedWorkspaceId::new(&old.scope_id).unwrap(),
            ContributionId::new(&old.contribution_id).unwrap(),
        ),
        publisher_namespace: old.publisher_namespace,
        plugin_code: old.plugin_code,
        plugin_version: old.plugin_version,
        workspace_id: Uuid::now_v7().to_string(),
        deadline_unix_ms: old.deadline_unix_ms,
    };
    service.authorize(&binding).await.unwrap();
    assert!(!authority.held.load(Ordering::SeqCst));
    let mut forged = binding.clone();
    forged.plugin_code = "forged".into();
    assert!(service.authorize(&forged).await.is_err());
    authority.state.lock().await.snapshot.authorizations.clear();
    assert!(service.authorize(&binding).await.is_err());
}
