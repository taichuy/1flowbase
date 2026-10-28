use super::*;
use control_plane_contracts::ports::*;
use domain::{AuditLogRecord, PluginContributionAuthorization};
use extension_contracts::{extension_bus::*, PluginDataOperation, PluginDataResponse};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
type TestFuture<T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>>;
pub(super) struct State {
    pub(super) installation: PluginInstallationRecord,
    pub(super) snapshot: PluginContributionAuthoritySnapshot,
}
pub(super) struct Authority {
    pub(super) state: Arc<tokio::sync::Mutex<State>>,
    pub(super) held: Arc<AtomicBool>,
    pub(super) locks: AtomicUsize,
}
struct Lease {
    state: tokio::sync::OwnedMutexGuard<State>,
    pub(super) held: Arc<AtomicBool>,
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
    ) -> TestFuture<()> {
        Box::pin(async move {
            drop(self);
            anyhow::ensure!(
                installation_ids.is_empty() && catalog.is_none(),
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
pub(super) fn fixture(actor: ActorContext) -> (Arc<Authority>, FileImportBinding) {
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
                "contribution_id":"ssh.list", "contributor_module_id":"ssh", "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append", "required_permissions":["service.execute","file.import.resource"]
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
                "file.import.resource",
                "file-import",
                ContributionResourceScope::System,
            ),
        ],
    };
    let binding = FileImportBinding {
        subject: ManagedContributionSubject::new(
            ManagedInstallationId::new(id.to_string()).unwrap(),
            ManagedWorkspaceId::new(scope.to_string()).unwrap(),
            ContributionId::new("ssh.list").unwrap(),
        ),
        publisher_namespace: "acme".into(),
        plugin_code: "ssh".into(),
        plugin_version: "1.0.0".into(),
        caller: runtime_core::runtime_backend::RuntimeCallerContext {
            actor,
            interface_id: "fixture".into(),
            session_identity: None,
            session_version: 7,
            selected_mcp_instance_ids: Some(vec!["selected".into()]),
        },
        deadline_unix_ms: i64::MAX,
    };
    (
        Arc::new(Authority {
            state: Arc::new(tokio::sync::Mutex::new(State {
                installation,
                snapshot,
            })),
            held: Arc::new(AtomicBool::new(false)),
            locks: AtomicUsize::new(0),
        }),
        binding,
    )
}

#[tokio::test]
async fn exchange_host_services_recheck_installed_grants_and_reject_revocation_or_contract_mismatch(
) {
    let actor: ActorContext = serde_json::from_value(json!({"user_id":"00000000-0000-0000-0000-000000000001","tenant_id":"00000000-0000-0000-0000-000000000000","current_workspace_id":"00000000-0000-0000-0000-000000000002","effective_display_role":"root","is_root":true,"permissions":[]})).unwrap();
    for (permission, contract) in [
        ("application.logs.read", "application-logs"),
        ("application.logs.import", "application-logs"),
        ("network.http.request", "http-egress"),
    ] {
        let (authority, binding) = fixture(actor.clone());
        assert!(crate::file_management::authorize_file_service(
            authority.as_ref(),
            &binding,
            permission
        )
        .await
        .is_err());
        {
            let mut state = authority.state.lock().await;
            state.installation.metadata_json["managed"]["module"]["contributions"][0]
                ["required_permissions"] = json!(["service.execute", permission]);
            let mut grant = state.snapshot.authorizations[1].clone();
            grant.permission = permission.into();
            grant.permission_contract_id = contract.into();
            state.snapshot.authorizations[1] = grant;
        }
        let lease = crate::file_management::authorize_file_service(
            authority.as_ref(),
            &binding,
            permission,
        )
        .await
        .unwrap();
        assert!(authority.held.load(Ordering::SeqCst));
        lease.release().await.unwrap();
        assert!(!authority.held.load(Ordering::SeqCst));
        {
            let mut state = authority.state.lock().await;
            state.snapshot.authorizations[1].permission_contract_id = "wrong-contract".into();
        }
        assert!(crate::file_management::authorize_file_service(
            authority.as_ref(),
            &binding,
            permission
        )
        .await
        .is_err());
        {
            let mut state = authority.state.lock().await;
            state.snapshot.authorizations[1].permission_contract_id = contract.into();
            state.snapshot.authorizations[1].status = ContributionAuthorizationStatus::Revoked;
        }
        assert!(crate::file_management::authorize_file_service(
            authority.as_ref(),
            &binding,
            permission
        )
        .await
        .is_err());
    }
}
