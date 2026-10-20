use super::*;
use extension_contracts::extension_bus::{
    ContributionId, ManagedContributionSubject, ManagedInstallationId, ManagedWorkspaceId,
};
use std::time::Duration;

async fn fixture() -> (PgControlPlaneStore, Uuid, Uuid, ManagedContributionSubject) {
    let store = store().await;
    let actor = actor(&store).await;
    let installation = store
        .commit_plugin_installation(&managed_input(actor, "7.0.0"))
        .await
        .unwrap();
    desired(
        &store,
        installation.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    store
        .lock_installation_contribution_authority(installation.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    let subject = ManagedContributionSubject::new(
        ManagedInstallationId::new(installation.id.to_string()).unwrap(),
        ManagedWorkspaceId::new(domain::SYSTEM_SCOPE_ID.to_string()).unwrap(),
        ContributionId::new("fixture.authenticate").unwrap(),
    );
    (store, actor, installation.id, subject)
}

async fn readers(
    store: &PgControlPlaneStore,
    subject: &ManagedContributionSubject,
) -> (
    Box<dyn ContributionAuthorityLease>,
    Box<dyn ContributionAuthorityLease>,
) {
    let first = store
        .lock_authentication_http_authority(subject)
        .await
        .unwrap();
    // The first guard is retained: exclusive reader implementations fail here.
    let second = tokio::time::timeout(
        Duration::from_secs(2),
        store.lock_authentication_http_authority(subject),
    )
    .await
    .expect("independent HTTP reads must overlap")
    .unwrap();
    (first, second)
}

async fn pending<T>(writer: &mut tokio::task::JoinHandle<T>) {
    assert!(
        tokio::time::timeout(Duration::from_millis(150), writer)
            .await
            .is_err(),
        "authority writer crossed an active HTTP guard"
    );
}

#[tokio::test]
async fn readers_overlap_and_exclusive_authority_waits_for_release_and_drop() {
    let (store, _, _, subject) = fixture().await;
    let (first, second) = readers(&store, &subject).await;
    let writer_store = store.clone();
    let mut writer =
        tokio::spawn(async move { writer_store.lock_contribution_authority(&subject).await });
    pending(&mut writer).await;
    first.release().await.unwrap();
    pending(&mut writer).await;
    drop(second);
    let writer_lease = tokio::time::timeout(Duration::from_secs(2), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    writer_lease.release().await.unwrap();
}

#[tokio::test]
async fn revocation_waits_for_every_http_reader_and_next_snapshot_is_revoked() {
    let (store, actor, installation, subject) = fixture().await;
    let authorization = Uuid::now_v7();
    sqlx::query("update plugin_contribution_authorization_revisions set revision=1 where installation_id=$1 and workspace_id=$2")
        .bind(installation).bind(domain::SYSTEM_SCOPE_ID).execute(store.pool()).await.unwrap();
    sqlx::query("insert into plugin_contribution_authorizations(id,installation_id,workspace_id,contribution_id,point_id,permission,resource_scope,permission_contract_id,permission_contract_version,status,granted_by,revision) values($1,$2,$3,'fixture.authenticate','1flowbase.authentication.provider','network.http.request','{\"kind\":\"workspace\"}'::jsonb,'authentication','1','active',$4,1)")
        .bind(authorization).bind(installation).bind(domain::SYSTEM_SCOPE_ID).bind(actor).execute(store.pool()).await.unwrap();
    let (first, second) = readers(&store, &subject).await;
    let writer_store = store.clone();
    let mut event = audit(actor, installation);
    event.event_code = "extension_center.contribution_authorizations.revoke".into();
    let mut writer = tokio::spawn(async move {
        writer_store
            .revoke_contribution_authorization(&RevokeContributionAuthorizationInput {
                installation_id: installation,
                workspace_id: domain::SYSTEM_SCOPE_ID,
                authorization_id: authorization,
                expected_revision: 1,
                actor_user_id: actor,
                audit_log: event,
            })
            .await
    });
    pending(&mut writer).await;
    first.release().await.unwrap();
    pending(&mut writer).await;
    second.release().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let next = store
        .lock_authentication_http_authority(&subject)
        .await
        .unwrap();
    assert_eq!(next.snapshot().revision, 2);
    assert_eq!(
        next.snapshot().authorizations[0].status,
        domain::ContributionAuthorizationStatus::Revoked
    );
    next.release().await.unwrap();
}

#[tokio::test]
async fn disable_waits_for_http_readers_and_next_guard_sees_disabled_installation() {
    let (store, actor, installation, subject) = fixture().await;
    let (first, second) = readers(&store, &subject).await;
    let writer_store = store.clone();
    let mut writer = tokio::spawn(async move {
        desired(
            &writer_store,
            installation,
            actor,
            PluginDesiredState::Disabled,
        )
        .await
    });
    pending(&mut writer).await;
    first.release().await.unwrap();
    pending(&mut writer).await;
    second.release().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let next = store
        .lock_authentication_http_authority(&subject)
        .await
        .unwrap();
    assert_eq!(
        next.installation(installation).unwrap().desired_state,
        PluginDesiredState::Disabled
    );
    next.release().await.unwrap();
}
