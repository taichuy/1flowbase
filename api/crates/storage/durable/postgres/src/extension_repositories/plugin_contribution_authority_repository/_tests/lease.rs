//! Adapter proof: provider inventory uses the leased connection and system lifecycle fence.
use super::*;
use control_plane_contracts::ports::{PluginRepository, UpsertPluginInstallationInput};
use serde_json::json;
use sqlx::Connection;

async fn install(
    store: &PgControlPlaneStore,
    actor: Uuid,
    code: &str,
) -> domain::PluginInstallationRecord {
    store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "lease.fixture".into(),
            provider_code: code.into(),
            plugin_id: format!("{code}@1.0.0"),
            plugin_version: "1.0.0".into(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: code.into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"managed_service":{"scope":"system"}}),
            is_system_reserved: false,
            actor_user_id: actor,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn system_lease_inventory_reuses_single_connection_and_fences_live_provider_changes() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    // A nested pool read while leased would time out: there is deliberately only one connection.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .connect(database.database_url())
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "authority inventory")
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "fixture",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    let consumer = install(&store, actor.id, "consumer").await;
    let provider = install(&store, actor.id, "provider").await;
    let lease = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        store.lock_installation_contribution_authority(consumer.id, domain::SYSTEM_SCOPE_ID),
    )
    .await
    .expect("inventory must not borrow a second pool connection")
    .unwrap();
    assert_eq!(lease.snapshot().installation_id, consumer.id);
    assert_eq!(
        lease.snapshots().len(),
        1,
        "provider facts are not provider grants"
    );
    assert_eq!(
        lease.installation(provider.id).unwrap().provider_code,
        "provider"
    );
    assert_eq!(lease.installations().len(), 2);
    let mut writer = sqlx::PgConnection::connect(database.database_url())
        .await
        .unwrap();
    let mut mutation = writer.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar(
        "select pg_try_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::text, 0))",
    )
    .bind(domain::SYSTEM_SCOPE_ID.to_string())
    .fetch_one(&mut *mutation)
    .await
    .unwrap();
    assert!(
        !acquired,
        "provider lifecycle fence must remain held until real call ends"
    );
    lease.release().await.unwrap();
    let acquired: bool = sqlx::query_scalar(
        "select pg_try_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::text, 0))",
    )
    .bind(domain::SYSTEM_SCOPE_ID.to_string())
    .fetch_one(&mut *mutation)
    .await
    .unwrap();
    assert!(acquired, "released lease permits the lifecycle writer");
    // Fixture mutation only: the real lifecycle owner performs its dependency blocker checks.
    sqlx::query("update extension_installations set desired_state='disabled' where id=$1")
        .bind(provider.id)
        .execute(&mut *mutation)
        .await
        .unwrap();
    mutation.commit().await.unwrap();
    writer.close().await.unwrap();
    let fresh = store
        .lock_installation_contribution_authority(consumer.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    assert_eq!(
        fresh.installation(provider.id).unwrap().desired_state,
        domain::PluginDesiredState::Disabled
    );
    assert_eq!(fresh.snapshot().installation_id, consumer.id);
    assert_eq!(fresh.snapshots().len(), 1);
    fresh.release().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn system_resource_authority_filters_history_and_projects_provider_metadata() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "shared resource projection")
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "fixture",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    let consumer = install(&store, actor.id, "consumer").await;
    let provider = install(&store, actor.id, "provider").await;
    let history = install(&store, actor.id, "history").await;
    let irrelevant = "x".repeat(1_000_000);
    let consumer_metadata = json!({"managed_service":{"scope":"system","untouched":null},"managed":{"module":{"private":irrelevant}}});
    let provider_metadata = json!({
        "plugin_dependencies": [], "resource_imports": null,
        "resource_exports": [{"resource_code":"hosts","collections":[],"credentials":null}],
        "data_models":[], "binding_targets":["system"],
        "managed_service":{"scope":"system","operations":[{"private":irrelevant}]},
        "unrelated":irrelevant,
    });
    sqlx::query("update extension_installations set metadata_json=$2 where id=$1")
        .bind(consumer.id)
        .bind(&consumer_metadata)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update extension_installations set metadata_json=$2 where id=$1")
        .bind(provider.id)
        .bind(&provider_metadata)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "update extension_installations set desired_state='disabled', metadata_json=$2 where id=$1",
    )
    .bind(history.id)
    .bind(json!({"managed_service":irrelevant,"binding_targets":["system"],"resource_exports":"malformed"}))
    .execute(&pool)
    .await
    .unwrap();

    let exclusive = store
        .lock_installation_contribution_authority(consumer.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    assert_eq!(exclusive.installations().len(), 3);
    assert_eq!(
        exclusive.installation(provider.id).unwrap().metadata_json,
        provider_metadata
    );
    exclusive.release().await.unwrap();
    let shared = store
        .lock_installation_resource_authority(consumer.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    assert_eq!(shared.installations().len(), 2);
    assert!(shared.installation(history.id).is_none());
    assert_eq!(
        shared.installation(consumer.id).unwrap().metadata_json,
        consumer_metadata
    );
    let provider = shared.installation(provider.id).unwrap();
    assert_eq!(
        domain::managed_installation_scope(provider, domain::DEFAULT_SCOPE_ID),
        domain::SYSTEM_SCOPE_ID
    );
    assert_eq!(
        provider.metadata_json,
        json!({
            "plugin_dependencies":[],"resource_imports":null,
            "resource_exports":provider_metadata["resource_exports"],"data_models":[],
            "binding_targets":["system"],"managed_service":{"scope":"system"},
        })
    );
    shared.release().await.unwrap();
    store
        .lock_installation_contribution_authority(history.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    let disabled_subject = store
        .lock_installation_resource_authority(history.id, domain::SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    assert_eq!(
        disabled_subject
            .installation(history.id)
            .unwrap()
            .metadata_json["managed_service"],
        irrelevant
    );
    disabled_subject.release().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn execution_leases_overlap_and_hold_the_exclusive_writer_fence() {
    use extension_contracts::extension_bus::{
        ContributionId, ManagedInstallationId, ManagedWorkspaceId,
    };
    use std::time::Duration;

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "execution authority")
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "fixture",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    let target = install(&store, actor.id, "target").await;
    let provider = install(&store, actor.id, "provider").await;
    let history = install(&store, actor.id, "history").await;
    sqlx::query("update extension_installations set desired_state='disabled' where id=$1")
        .bind(history.id)
        .execute(&pool)
        .await
        .unwrap();
    let subject = ManagedContributionSubject::new(
        ManagedInstallationId::new(target.id.to_string()).unwrap(),
        ManagedWorkspaceId::new(domain::SYSTEM_SCOPE_ID.to_string()).unwrap(),
        ContributionId::new("fixture.execute").unwrap(),
    );
    store
        .lock_contribution_authority(&subject)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();

    let (first, second, callback) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::try_join!(
            store.lock_contribution_execution_authority(&subject),
            store.lock_contribution_execution_authority(&subject),
            store.lock_managed_callback_authority(&subject),
        )
    })
    .await
    .expect("execution and resource callback readers must overlap")
    .unwrap();
    for lease in [&first, &second] {
        assert_eq!(lease.snapshot().installation_id, target.id);
        assert_eq!(lease.installations().len(), 1);
        assert_eq!(
            lease.installation(target.id).unwrap().metadata_json,
            target.metadata_json
        );
        assert!(lease.installation(provider.id).is_none());
        assert!(lease.installation(history.id).is_none());
    }
    assert!(callback.installation(provider.id).is_some());
    callback.release().await.unwrap();

    let writer = tokio::spawn(async move { store.lock_contribution_authority(&subject).await });
    let writer_waits = || async {
        sqlx::query_scalar::<_, bool>(
            "select exists (select from pg_locks where locktype='advisory' and mode='ExclusiveLock'
             and not granted and database=(select oid from pg_database where datname=current_database()))",
        ).fetch_one(&pool).await.unwrap()
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        while !writer_waits().await {
            assert!(!writer.is_finished(), "writer crossed both readers");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("writer must queue on the workspace advisory lock");
    first.release().await.unwrap();
    assert!(
        writer_waits().await,
        "writer must still wait for the second reader"
    );
    drop(second);
    let lease = tokio::time::timeout(Duration::from_secs(5), writer)
        .await
        .expect("last reader drop must unblock the writer")
        .unwrap()
        .unwrap();
    assert_eq!(
        lease.installations().len(),
        3,
        "exclusive inventory remains complete"
    );
    lease.release().await.unwrap();
    pool.close().await;
}
