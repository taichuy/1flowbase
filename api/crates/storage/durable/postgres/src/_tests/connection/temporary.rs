use super::*;
use sqlx::postgres::PgTransactionManager;
use sqlx_core::transaction::TransactionManager;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use storage_durable_postgres::PgInstanceConnectionBudget;
use tokio::sync::{mpsc, Notify};

#[tokio::test]
async fn temporary_quota_survives_cancelled_close_until_native_checkouts_drain() {
    let mut configuration = settings(1);
    configuration.temporary_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections().clone();
    let first = registry
        .temporary_pool(PgPoolOptions::new(), deadline())
        .await
        .unwrap();
    let checked_out = first.pool().acquire().await.unwrap();
    let close = tokio::spawn(first.close());
    eventually(|| registry.observe().temporary.closing == 1).await;
    close.abort();
    assert!(close.await.unwrap_err().is_cancelled());
    let pending_registry = registry.clone();
    let pending = tokio::spawn(async move {
        pending_registry
            .temporary_pool(PgPoolOptions::new(), deadline())
            .await
            .unwrap()
    });
    eventually(|| registry.observe().temporary.waiting == 1).await;
    assert_eq!(registry.observe().temporary.active, 0);
    assert_eq!(registry.observe().temporary.closing, 1);
    assert!(!pending.is_finished());
    // A new pool cannot be constructed while the old native checkout remains alive.
    drop(checked_out);
    let second = timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(registry.observe().temporary.active, 1);
    assert_eq!(registry.observe().temporary.closing, 0);
    second.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn cancelled_or_expired_temporary_quota_wait_creates_no_pool_or_connection() {
    let mut configuration = settings(1);
    configuration.temporary_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections().clone();
    let first = registry
        .temporary_pool(PgPoolOptions::new(), deadline())
        .await
        .unwrap();
    let connects = Arc::new(AtomicUsize::new(0));
    let connected = connects.clone();
    let options = PgPoolOptions::new().after_connect(move |_, _| {
        connected.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    });
    let pending_registry = registry.clone();
    let pending_options = options.clone();
    let pending = tokio::spawn(async move {
        pending_registry
            .temporary_pool(pending_options, deadline())
            .await
    });
    eventually(|| registry.observe().temporary.waiting == 1).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    eventually(|| registry.observe().temporary.waiting == 0).await;
    let error = registry
        .temporary_pool(options, Instant::now() + Duration::from_millis(50))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("quota deadline"));
    assert_eq!(connects.load(Ordering::SeqCst), 0);
    assert_eq!(registry.observe().temporary.active, 1);
    first.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn cancellation_during_real_after_connect_releases_quota_only_via_pool_drain() {
    let mut configuration = settings(1);
    configuration.temporary_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections().clone();
    let (connected, mut connects) = mpsc::unbounded_channel();
    let barrier = Arc::new(Notify::new());
    let options = PgPoolOptions::new().after_connect(move |connection, _| {
        let connected = connected.clone();
        let barrier = barrier.clone();
        Box::pin(async move {
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *connection)
                .await?;
            connected.send(pid).unwrap();
            barrier.notified().await;
            Ok(())
        })
    });
    let pending_registry = registry.clone();
    let pending =
        tokio::spawn(async move { pending_registry.temporary_pool(options, deadline()).await });
    let pid = timeout(Duration::from_secs(2), connects.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(registry.observe().temporary.active, 1);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    eventually(|| {
        registry.observe().temporary.active == 0 && registry.observe().temporary.closing == 0
    })
    .await;
    eventually_async(|| async { !fixture.session_exists(pid).await }).await;
    registry
        .temporary_pool(PgPoolOptions::new(), deadline())
        .await
        .unwrap()
        .close()
        .await;
    fixture.close().await;
}

#[tokio::test]
async fn caller_transaction_hooks_apply_to_fresh_and_idle_connections_and_drop_rolls_back() {
    let mut configuration = settings(1);
    configuration.temporary_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    sqlx::query("CREATE TABLE connection_factory_fixture (id integer PRIMARY KEY)")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    let connects = Arc::new(AtomicUsize::new(0));
    let checks = Arc::new(AtomicUsize::new(0));
    let connected = connects.clone();
    let checked = checks.clone();
    let options = PgPoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .after_connect(move |connection, _| {
            connected.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move { PgTransactionManager::begin(connection, None).await })
        })
        .before_acquire(move |connection, _| {
            checked.fetch_add(1, Ordering::SeqCst);
            Box::pin(
                async move { Ok(PgTransactionManager::get_transaction_depth(connection) == 1) },
            )
        });
    let owner = fixture
        .store
        .connections()
        .temporary_pool(options, deadline())
        .await
        .unwrap();
    eventually(|| owner.pool().num_idle() == 1).await;
    let mut connection = owner.pool().acquire().await.unwrap();
    assert_eq!(PgTransactionManager::get_transaction_depth(&connection), 1);
    sqlx::query("INSERT INTO connection_factory_fixture VALUES (1)")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    assert_eq!(connects.load(Ordering::SeqCst), 1);
    assert!(checks.load(Ordering::SeqCst) >= 1);
    owner.close().await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM connection_factory_fixture")
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    fixture.close().await;
}

#[tokio::test]
async fn compatibility_temporary_factory_admits_six_and_counts_every_owner() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections();
    let mut owners = Vec::new();
    for _ in 0..6 {
        owners.push(
            registry
                .temporary_pool(PgPoolOptions::new(), deadline())
                .await
                .unwrap(),
        );
    }
    assert_eq!(registry.observe().temporary.active, 6);
    assert_eq!(registry.observe().temporary.max_connections, None);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:temporary'")
        .fetch_one(&fixture.observer).await.unwrap();
    assert_eq!(sessions, 6);
    for owner in owners {
        owner.close().await;
    }
    assert_eq!(registry.observe().temporary.active, 0);
    assert_eq!(registry.observe().temporary.closing, 0);
    fixture.close().await;
}

#[tokio::test]
async fn fully_allocated_lanes_conserve_the_instance_budget_while_dispatch_is_full() {
    let mut configuration = settings(1);
    configuration.managed_max_connections = Some(1);
    configuration.dispatch_max_connections = Some(2);
    configuration.template_install_max_connections = Some(1);
    configuration.temporary_max_connections = Some(1);
    configuration.instance_budget = Some(PgInstanceConnectionBudget {
        max_connections: 8,
        reserved_connections: 2,
    });
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections();
    let mut parents = Vec::new();
    for _ in 0..2 {
        parents.push(
            registry
                .try_acquire(
                    PgAdvisoryLockKey::dispatch(Uuid::now_v7(), Uuid::now_v7()),
                    deadline(),
                )
                .await
                .unwrap()
                .unwrap(),
        );
    }
    let child = registry.acquire(managed_key().2, deadline()).await.unwrap();
    let install = registry
        .acquire(
            PgAdvisoryLockKey::template_install(Uuid::now_v7()),
            deadline(),
        )
        .await
        .unwrap();
    let temporary = registry
        .temporary_pool(PgPoolOptions::new(), deadline())
        .await
        .unwrap();
    sqlx::query("SELECT 1")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    let observation = registry.observe();
    let sessions = observation.business.size
        + observation.managed.pool.size
        + observation.dispatch.pool.size
        + observation.template_install.pool.size
        + observation.temporary.active as u32;
    assert_eq!(sessions, 6);
    let actual_sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND backend_type = 'client backend' AND application_name <> 'w1-fixture:observer'")
        .fetch_one(&fixture.observer).await.unwrap();
    assert_eq!(actual_sessions, i64::from(sessions));
    let budget = observation.instance_budget.unwrap();
    assert_eq!(
        sessions + budget.reserved_connections,
        budget.max_connections
    );
    assert!(registry
        .temporary_pool(
            PgPoolOptions::new(),
            Instant::now() + Duration::from_millis(50)
        )
        .await
        .is_err());
    assert!(registry
        .acquire(
            PgAdvisoryLockKey::template_install(Uuid::now_v7()),
            Instant::now() + Duration::from_millis(50)
        )
        .await
        .is_err());
    temporary.close().await;
    child.release().await;
    install.release().await;
    for parent in parents {
        parent.release().await;
    }
    fixture.close().await;
}
