use control_plane::ports::{
    CreateDataSourceInstanceInput, DataSourceInstanceVisibility, DataSourceRepository,
    PluginRepository, UpsertPluginInstallationInput,
};
use domain::{
    AuthenticatorRecord, DataSourceDefaults, DataSourceInstanceStatus, PluginArtifactStatus,
    PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
    PASSWORD_LOCAL_AUTHENTICATOR_ID,
};
use serde_json::json;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seeded_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
    Uuid,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, &format!("data-source-{}", Uuid::now_v7()))
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&AuthenticatorRecord {
            id: PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            &format!("data-source-owner-{}", Uuid::now_v7()),
            &format!("data-source-owner-{}@example.test", Uuid::now_v7()),
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Owner",
            "Owner",
        )
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            provider_code: "acme_hubspot_source".into(),
            plugin_id: "acme_hubspot_source@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Acme HubSpot Source".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/plugin-installed/acme_hubspot_source/0.1.0".into(),
            checksum: Some("abc123".into()),
            manifest_fingerprint: None,
            signature_status: Some("unsigned".into()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    (store, workspace, actor, installation_id)
}

async fn create_instance(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    installation_id: Uuid,
    created_by: Uuid,
    display_name: &str,
) -> domain::DataSourceInstanceRecord {
    DataSourceRepository::create_instance(
        store,
        &CreateDataSourceInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: display_name.into(),
            status: DataSourceInstanceStatus::Ready,
            config_json: json!({ "client_id": "abc" }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn ac_005_data_source_visibility_filters_persisted_owner_and_workspace() {
    let (store, workspace, actor, installation_id) = seeded_store().await;
    let peer = store
        .upsert_root_user(
            workspace.id,
            &format!("data-source-peer-{}", Uuid::now_v7()),
            &format!("data-source-peer-{}@example.test", Uuid::now_v7()),
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Peer",
            "Peer",
        )
        .await
        .unwrap();
    let foreign_workspace = store
        .upsert_workspace(workspace.tenant_id, &format!("foreign-{}", Uuid::now_v7()))
        .await
        .unwrap();
    let own = create_instance(&store, workspace.id, installation_id, actor.id, "Own").await;
    let peer_instance =
        create_instance(&store, workspace.id, installation_id, peer.id, "Peer").await;
    let foreign_instance = create_instance(
        &store,
        foreign_workspace.id,
        installation_id,
        peer.id,
        "Foreign",
    )
    .await;

    let own_rows = DataSourceRepository::list_instances(
        &store,
        workspace.id,
        actor.id,
        DataSourceInstanceVisibility::Own,
    )
    .await
    .unwrap();
    assert_eq!(
        own_rows
            .iter()
            .map(|instance| instance.id)
            .collect::<Vec<_>>(),
        vec![own.id]
    );

    let scope_rows = DataSourceRepository::list_instances(
        &store,
        workspace.id,
        actor.id,
        DataSourceInstanceVisibility::ScopeAll,
    )
    .await
    .unwrap();
    assert_eq!(
        scope_rows
            .iter()
            .map(|instance| instance.id)
            .collect::<Vec<_>>(),
        vec![own.id, peer_instance.id]
    );

    assert!(DataSourceRepository::get_instance_for_visibility(
        &store,
        workspace.id,
        peer_instance.id,
        actor.id,
        DataSourceInstanceVisibility::Own,
    )
    .await
    .unwrap()
    .is_none());
    assert!(DataSourceRepository::get_instance_for_visibility(
        &store,
        foreign_workspace.id,
        foreign_instance.id,
        actor.id,
        DataSourceInstanceVisibility::Own,
    )
    .await
    .unwrap()
    .is_none());
}
