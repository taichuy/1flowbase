//! P10: actual installed SDK, PostgreSQL commit barriers and the real server shutdown owner.
use super::{
    managed_event_authority_tests::{grant, manifest, package},
    managed_snapshot_tests::{create, RuntimeFixture},
};
use control_plane::{lifecycle_outbox_dispatcher::*, plugin_management::*, ports::AuthRepository};
use control_plane_contracts::ports::*;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
struct Completion;
impl LifecycleDeliveryCompletionPort for Completion {
    fn complete(&self, _: extension_contracts::CompletionOutcome<LifecycleFactDeliveryCompletion>) {
    }
}
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_owned_transactions_shutdown() {
    let source = std::path::PathBuf::from(
        std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE")
            .expect("prebuild real managed_event_worker"),
    );
    assert!(source.is_absolute() && source.is_file());
    let (initial, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .unwrap();
    let mut state = (*initial).clone();
    state.store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone());
    let runtime = RuntimeFixture::new(&state);
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = runtime
        .store
        .load_actor_context_for_user(actor_id)
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let management = PluginManagementService::new(
        runtime.store.clone(),
        crate::provider_runtime::ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id);
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&state.api_node_id);
    let mut publisher = None;
    for name in ["a", "b"] {
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: format!("budget-{name}.1flowbasepkg"),
                package_bytes: package(&manifest(name)),
            })
            .await
            .unwrap()
            .installation
            .id;
        if name == "a" {
            publisher = Some(installed);
        }
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor_id,
                installation_id: installed,
            })
            .await
            .unwrap();
        for permission in if name == "a" {
            vec!["event.subscribe", "event.publish"]
        } else {
            vec!["event.subscribe"]
        } {
            authority
                .grant(&actor, installed, grant(name, permission))
                .await
                .unwrap();
        }
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id: installed,
            })
            .await
            .unwrap();
    }
    create(&runtime.store, actor_id, workspace).await;
    let records = runtime
        .store
        .claim_lifecycle_facts(Uuid::now_v7(), 32, time::Duration::seconds(30))
        .await
        .unwrap();
    let cause = records
        .iter()
        .find(|r| r.subscriber_id.ends_with(".acme.composition-a.events"))
        .unwrap()
        .clone();
    let initial_events: i64 = sqlx::query_scalar("select count(*) from lifecycle_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    let initial_deliveries: i64 =
        sqlx::query_scalar("select count(*) from lifecycle_outbox_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap();
    let key = (Uuid::now_v7().as_u128() as u64 & 0x7fff_ffff_ffff_ffff) as i64;
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("select pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    sqlx::query(&format!("create function root_2007_shutdown_barrier() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock({key}); return NEW; end $$")).execute(&pool).await.unwrap();
    sqlx::query("create constraint trigger root_2007_shutdown_barrier after insert on lifecycle_outbox deferrable initially deferred for each row execute function root_2007_shutdown_barrier()").execute(&pool).await.unwrap();
    let create_waiter = {
        let store = runtime.store.clone();
        tokio::spawn(async move { create(&store, actor_id, workspace).await })
    };
    let derived_waiter = {
        let delivery = runtime.delivery.clone();
        tokio::spawn(async move { delivery.deliver(&cause).await })
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("select count(*) from pg_locks where locktype='advisory' and not granted and objid=$1::bigint::oid").bind(key & 0xffff_ffff).fetch_one(&pool).await.unwrap();
            if waiting >= 2 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let publisher = publisher.unwrap();
    let contending = {
        let store = runtime.store.clone();
        tokio::spawn(async move {
            let lease = store
                .lock_installation_contribution_authority(publisher, workspace)
                .await?;
            lease.release().await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_locks where locktype='advisory' and not granted and objid=(hashtextextended('managed-workspace:' || $1::text,0) & 4294967295)::oid)")
                .bind(workspace.to_string()).fetch_one(&pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    // A foreign scope is still rejected, but its unrelated workspace lock can make progress
    // while the first workspace's admitted E2 holds its commit/authority lease.
    let foreign = tokio::time::timeout(
        Duration::from_secs(2),
        runtime
            .store
            .lock_installation_contribution_authority(publisher, Uuid::now_v7()),
    )
    .await
    .expect("workspace authority locks must not serialize unrelated workspaces");
    assert!(foreign.is_err());
    assert!(
        !contending.is_finished(),
        "same-workspace authority waits for the existing E2 transaction"
    );
    create_waiter.abort();
    derived_waiter.abort();
    assert!(create_waiter.await.unwrap_err().is_cancelled());
    assert!(derived_waiter.await.unwrap_err().is_cancelled());
    let operations = runtime.store.managed_operation_lifetime();
    assert_eq!(operations.snapshot().active, [0, 1, 1, 0]);
    // Close the real dispatcher before its first poll; no new claim may race this barrier.
    let worker = LifecycleOutboxDispatcher::new(
        runtime.store.clone(),
        runtime.delivery.clone(),
        Arc::new(Completion),
    )
    .spawn();
    worker.close();
    let shutdown = Arc::new(crate::ApiRuntimeShutdown {
        host: runtime.host.clone(),
        services: runtime.services.clone(),
        operations: operations.clone(),
        lifecycle_worker: worker,
    });
    let stopping = {
        let shutdown = shutdown.clone();
        tokio::spawn(async move { shutdown.stop().await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while !operations.snapshot().closed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!stopping.is_finished());
    assert!(operations
        .wait(Duration::ZERO)
        .await
        .unwrap_err()
        .to_string()
        .contains("unfinished"));
    assert_eq!(operations.snapshot().active, [0, 1, 1, 0]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        initial_events
    );
    sqlx::query("select pg_advisory_unlock($1)")
        .bind(key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    drop(blocker);
    tokio::time::timeout(Duration::from_secs(5), stopping)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), contending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(operations.snapshot().active, [0; 4]);
    assert!(operations.snapshot().closed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        initial_events + 2,
        "both actual owners commit despite protocol cancellation"
    );
    assert!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap()
            > initial_deliveries
    );
    assert!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from lifecycle_outbox_deliveries where status <> 'delivered'"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
            > 0,
        "shutdown cleanup preserves durable backlog"
    );
    assert!(runtime.composition.snapshot(workspace).await.is_none());
    assert_eq!(
        runtime.host.lifecycle(),
        runtime_core::runtime_backend::RuntimeBackendLifecycle::Stopped
    );
    sqlx::query("drop trigger root_2007_shutdown_barrier on lifecycle_outbox")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("drop function root_2007_shutdown_barrier()")
        .execute(&pool)
        .await
        .unwrap();
}
