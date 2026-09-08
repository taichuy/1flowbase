use super::fixture::Fixture;
use control_plane::{
    plugin_management::{
        GrantContributionPermission, HostContributionGrantPolicy,
        PluginContributionAuthorityService, RevokeContributionPermission,
    },
    ports::{CreatePluginAssignmentInput, PluginRepository, UpsertPluginInstallationInput},
};
use control_plane_contracts::ports::PluginContributionAuthorityRepository;
use domain::ContributionResourceScope;
use extension_contracts::extension_bus::{
    ContributionId, ManagedContributionSubject, ManagedInstallationId, ManagedWorkspaceId,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn root_2007_ac_002_009_contribution_authority() {
    let fixture = Fixture::new().await;
    let raw = include_str!("../../../extension-package-runtime/src/_tests/managed_manifest.yaml")
        .replace("point_id: acme.compute", "point_id: 1flowbase.model-definitions.create.before")
        .replace("contract_version: acme.compute/v1", "contract_version: 1\n        required_permissions: [hook.model_definitions.create.before]");
    let manifest = plugin_framework::parse_plugin_manifest(&raw).unwrap();
    let installation = fixture
        .store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "acme".into(),
            provider_code: "managed_fixture".into(),
            plugin_id: "managed_fixture@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Contribution authority fixture".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::Disabled,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"managed": manifest.managed}),
            is_system_reserved: false,
            actor_user_id: fixture.actor.user_id,
        })
        .await
        .unwrap();
    fixture
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: fixture.actor.current_workspace_id,
            provider_code: installation.provider_code.clone(),
            actor_user_id: fixture.actor.user_id,
        })
        .await
        .unwrap();
    let service = PluginContributionAuthorityService::new(
        fixture.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let grant = GrantContributionPermission {
        contribution_id: "managed_fixture.compute".into(),
        permission: "hook.model_definitions.create.before".into(),
        resource_scope: ContributionResourceScope::Workspace,
        permission_contract_id: "managed-hook".into(),
        permission_contract_version: "1".into(),
    };
    let initial = service
        .query(&fixture.actor, installation.id)
        .await
        .unwrap();
    assert_eq!(initial.revision, 0);
    assert!(
        initial.authorizations.is_empty(),
        "installed declarations do not grant rights"
    );
    let mut configure_only = fixture.actor.clone();
    configure_only.is_root = false;
    configure_only.user_id = Uuid::now_v7();
    configure_only
        .permissions
        .insert("plugin_config.configure.all".into());
    assert!(service
        .grant(&configure_only, installation.id, grant.clone())
        .await
        .is_err());
    assert!(service
        .query(&configure_only, installation.id)
        .await
        .is_err());
    let mut oversized = grant.clone();
    oversized.permission = "all".into();
    assert!(service
        .grant(&fixture.actor, installation.id, oversized)
        .await
        .is_err());
    let mut wrong_scope = fixture.actor.clone();
    wrong_scope.current_workspace_id = Uuid::now_v7();
    assert!(service
        .grant(&wrong_scope, installation.id, grant.clone())
        .await
        .is_err());
    let granted = service
        .grant(&fixture.actor, installation.id, grant.clone())
        .await
        .unwrap();
    assert_eq!(granted.revision, 1);
    assert_eq!(granted.authorizations.len(), 1);
    let authorization_id = granted.authorizations[0].id;
    assert_eq!(
        granted.authorizations[0].status,
        domain::ContributionAuthorizationStatus::Active
    );
    let restarted = PluginContributionAuthorityService::new(
        fixture.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    assert_eq!(
        restarted
            .query(&fixture.actor, installation.id)
            .await
            .unwrap()
            .revision,
        1
    );
    let subject = ManagedContributionSubject::new(
        ManagedInstallationId::new(installation.id.to_string()).unwrap(),
        ManagedWorkspaceId::new(fixture.actor.current_workspace_id.to_string()).unwrap(),
        ContributionId::new("managed_fixture.compute").unwrap(),
    );
    let lease = fixture
        .store
        .lock_contribution_authority(&subject)
        .await
        .unwrap();
    assert_eq!(lease.snapshot().revision, 1);
    assert_eq!(lease.snapshot().authorizations.len(), 1);
    // Holding this lease is the concrete serialization point P02C uses for admission.
    let revoke = RevokeContributionPermission {
        authorization_id,
        expected_revision: 1,
    };
    let revoke_store = fixture.store.clone();
    let revoke_actor = fixture.actor.clone();
    let revoke_command = revoke.clone();
    let installation_id = installation.id;
    let pending_revoke = tokio::spawn(async move {
        PluginContributionAuthorityService::new(
            revoke_store,
            HostContributionGrantPolicy::root_composition(),
        )
        .revoke(&revoke_actor, installation_id, revoke_command)
        .await
    });
    // Observe PostgreSQL's actual blocked writer, rather than interpreting elapsed time
    // as evidence that the revoke reached its lock boundary. The relation OID pins this
    // isolated fixture schema, so another concurrently running fixture cannot satisfy it.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_locks l join pg_stat_activity a on a.pid=l.pid where l.relation='plugin_contribution_authorization_revisions'::regclass and a.wait_event_type='Lock' and cardinality(pg_blocking_pids(a.pid))>0)")
                .fetch_one(fixture.store.pool()).await.unwrap();
            if blocked { break; }
            assert!(!pending_revoke.is_finished(), "revoke must wait for the authority lease");
            tokio::task::yield_now().await;
        }
    }).await.expect("revoke should reach its database lock");
    assert!(!pending_revoke.is_finished());
    lease.release().await.unwrap();
    let revoked = pending_revoke.await.unwrap().unwrap();
    assert_eq!(revoked.revision, 2);
    assert_eq!(
        revoked.authorizations[0].status,
        domain::ContributionAuthorizationStatus::Revoked
    );
    assert!(restarted
        .revoke(&fixture.actor, installation.id, revoke)
        .await
        .is_err());
    let fresh = fixture
        .store
        .lock_contribution_authority(&subject)
        .await
        .unwrap();
    assert_eq!(fresh.snapshot().revision, 2);
    assert!(fresh
        .snapshot()
        .authorizations
        .iter()
        .all(|authorization| authorization.status
            == domain::ContributionAuthorizationStatus::Revoked));
    fresh.release().await.unwrap();
    let audits: i64 = sqlx::query_scalar("select count(*) from audit_logs where target_id=$1 and event_code in ('plugin.contribution_authorization.granted','plugin.contribution_authorization.revoked')")
        .bind(installation.id).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(audits, 2);
    sqlx::raw_sql("CREATE FUNCTION reject_authority_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_code='plugin.contribution_authorization.granted' THEN RAISE EXCEPTION 'authority_audit_failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_authority_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_authority_audit();")
        .execute(fixture.store.pool()).await.unwrap();
    assert!(service
        .grant(&fixture.actor, installation.id, grant)
        .await
        .is_err());
    assert_eq!(
        service
            .query(&fixture.actor, installation.id)
            .await
            .unwrap()
            .revision,
        2,
        "audit failure rolls back grant and revision together"
    );
}
