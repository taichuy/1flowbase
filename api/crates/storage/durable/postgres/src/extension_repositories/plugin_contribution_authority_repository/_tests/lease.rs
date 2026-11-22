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
