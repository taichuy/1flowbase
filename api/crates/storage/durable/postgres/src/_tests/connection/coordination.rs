use super::*;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn local_fifo_followers_take_no_sessions_and_business_sql_progresses_at_p1_and_p5() {
    for business in [1, 5] {
        let fixture = Fixture::new(settings(business)).await;
        let registry = fixture.store.connections().clone();
        let (_, _, key) = managed_key();
        let holder = registry.acquire(key.clone(), deadline()).await.unwrap();
        let (entered, mut entries) = mpsc::unbounded_channel();
        let mut followers = Vec::new();
        let mut releases = Vec::new();
        for id in 0..4 {
            let (release, released) = oneshot::channel();
            releases.push(release);
            let registry = registry.clone();
            let key = key.clone();
            let entered = entered.clone();
            followers.push(tokio::spawn(async move {
                let lease = registry.acquire(key, deadline()).await.unwrap();
                entered.send(id).unwrap();
                released.await.unwrap();
                lease.release().await
            }));
            eventually(|| fixture.store.connections().observe().managed.local_queue == id + 1)
                .await;
        }
        let observation = registry.observe();
        assert_eq!(observation.managed.active, 1);
        assert_eq!(observation.managed.pool.size, 1);
        assert_eq!(observation.local_key_cells, 1);
        let open_managed: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock'")
            .fetch_one(&fixture.observer).await.unwrap();
        assert_eq!(open_managed, 1);
        let managed_transactions: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock' AND xact_start IS NOT NULL")
            .fetch_one(&fixture.observer).await.unwrap();
        assert_eq!(managed_transactions, 0);
        // Direct consumers retain native SQLx limits, independent of the lock lane.
        let value: i32 = timeout(
            Duration::from_secs(2),
            sqlx::query_scalar("SELECT 7").fetch_one(fixture.store.pool()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(value, 7);
        let mut transaction = timeout(Duration::from_secs(2), fixture.store.pool().begin())
            .await
            .unwrap()
            .unwrap();
        sqlx::query("SELECT 1")
            .execute(&mut *transaction)
            .await
            .unwrap();
        transaction.rollback().await.unwrap();
        assert!(!holder.release().await.lost);
        for (id, release) in releases.into_iter().enumerate() {
            assert_eq!(
                timeout(Duration::from_secs(2), entries.recv())
                    .await
                    .unwrap(),
                Some(id)
            );
            assert!(entries.try_recv().is_err());
            release.send(()).unwrap();
        }
        for follower in followers {
            assert!(!follower.await.unwrap().lost);
        }
        eventually(|| registry.observe().local_key_cells == 0).await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn independent_coordinators_keep_existing_key_seed_and_legacy_xact_exclusion() {
    let fixture = Fixture::new(settings(1)).await;
    let second = fixture.another_store(settings(1)).await;
    let (installation, scope, key) = managed_key();
    let mut legacy = fixture.store.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
        .bind(installation.to_string()).bind(scope.to_string()).execute(&mut *legacy).await.unwrap();
    let second_registry = second.connections().clone();
    let pending_key = key.clone();
    let pending = tokio::spawn(async move {
        second_registry
            .acquire(pending_key, deadline())
            .await
            .unwrap()
    });
    eventually_async(|| async { fixture.lock_waiter_pid().await.is_some() }).await;
    assert!(!pending.is_finished());
    legacy.rollback().await.unwrap();
    let lease = pending.await.unwrap();
    let mut probe = fixture.observer.begin().await.unwrap();
    let legacy_acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
        .bind(installation.to_string()).bind(scope.to_string()).fetch_one(&mut *probe).await.unwrap();
    assert!(!legacy_acquired);
    probe.rollback().await.unwrap();
    let other = fixture
        .store
        .connections()
        .acquire(managed_key().2, deadline())
        .await
        .unwrap();
    let own_registry = fixture.store.connections().clone();
    let pending = tokio::spawn(async move { own_registry.acquire(key, deadline()).await.unwrap() });
    eventually(|| fixture.store.connections().observe().local_key_cells == 2).await;
    // The other key uses this node's one session. Releasing it admits the PG waiter.
    other.release().await;
    eventually_async(|| async { fixture.lock_waiter_pid().await.is_some() }).await;
    assert!(!pending.is_finished());
    lease.release().await;
    pending.await.unwrap().release().await;
    second.connections().close().await;
    fixture.close().await;
}

#[tokio::test]
async fn cancelling_local_and_pool_waiters_reclaims_key_cells_without_new_sessions() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections().clone();
    let (_, _, key) = managed_key();
    let holder = registry.acquire(key.clone(), deadline()).await.unwrap();
    let queued_registry = registry.clone();
    let queued = tokio::spawn(async move { queued_registry.acquire(key, deadline()).await });
    eventually(|| registry.observe().managed.local_queue == 1).await;
    queued.abort();
    assert!(queued.await.unwrap_err().is_cancelled());
    eventually(|| registry.observe().managed.local_queue == 0).await;
    let mut waiters = Vec::new();
    for _ in 0..16 {
        let registry = registry.clone();
        waiters.push(tokio::spawn(async move {
            registry.acquire(managed_key().2, deadline()).await
        }));
    }
    eventually(|| registry.observe().local_key_cells == 17).await;
    assert_eq!(registry.observe().managed.pool.size, 1);
    for waiter in waiters {
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
    }
    eventually(|| registry.observe().local_key_cells == 1).await;
    holder.release().await;
    eventually(|| registry.observe().local_key_cells == 0).await;
    // Distinct keys leave no historical cell entries, including idle pooled sessions.
    for _ in 0..32 {
        registry
            .acquire(managed_key().2, deadline())
            .await
            .unwrap()
            .release()
            .await;
    }
    assert_eq!(registry.observe().local_key_cells, 0);
    fixture.close().await;
}

#[tokio::test]
async fn expired_local_and_pool_deadlines_never_admit_work() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections();
    assert!(registry
        .acquire(managed_key().2, Instant::now())
        .await
        .is_err());
    assert_eq!(registry.observe().managed.pool.size, 0);
    assert_eq!(registry.observe().local_key_cells, 0);
    let (_, _, key) = managed_key();
    let holder = registry.acquire(key.clone(), deadline()).await.unwrap();
    let short = || Instant::now() + Duration::from_millis(50);
    let local_error = registry.acquire(key, short()).await.unwrap_err();
    assert!(local_error.to_string().contains("local queue deadline"));
    let pool_error = registry
        .acquire(managed_key().2, short())
        .await
        .unwrap_err();
    assert!(pool_error.to_string().contains("lane acquire deadline"));
    let observation = registry.observe();
    assert_eq!(observation.managed.active, 1);
    assert_eq!(observation.managed.pool.size, 1);
    assert_eq!(observation.local_key_cells, 1);
    holder.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn cancelled_pg_acquisition_closes_session_instead_of_returning_it_to_pool() {
    let fixture = Fixture::new(settings(1)).await;
    let (installation, scope, key) = managed_key();
    let mut legacy = fixture.observer.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
        .bind(installation.to_string()).bind(scope.to_string()).execute(&mut *legacy).await.unwrap();
    // Use the business pool as observer here because the sole observer connection owns legacy.
    let registry = fixture.store.connections().clone();
    let pending_registry = registry.clone();
    let pending_key = key.clone();
    let pending =
        tokio::spawn(async move { pending_registry.acquire(pending_key, deadline()).await });
    eventually_async(|| async {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND NOT granted AND database = (SELECT oid FROM pg_database WHERE datname = current_database()))")
            .fetch_one(fixture.store.pool()).await.unwrap()
    }).await;
    let pid: i32 = sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock'")
        .fetch_one(fixture.store.pool()).await.unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    legacy.rollback().await.unwrap();
    eventually_async(|| async { !fixture.session_exists(pid).await }).await;
    eventually(|| {
        registry.observe().managed.closing == 0 && registry.observe().local_key_cells == 0
    })
    .await;
    let fresh = registry.acquire(key, deadline()).await.unwrap();
    assert_ne!(fixture.managed_pid().await, pid);
    fresh.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn pg_acquire_error_and_deadline_both_discard_uncertain_connections() {
    for server_cancel in [true, false] {
        let fixture = Fixture::new(settings(1)).await;
        let (installation, scope, key) = managed_key();
        let mut legacy = fixture.store.pool().begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
            .bind(installation.to_string()).bind(scope.to_string()).execute(&mut *legacy).await.unwrap();
        let registry = fixture.store.connections().clone();
        let pending_registry = registry.clone();
        let pending = tokio::spawn(async move {
            pending_registry
                .acquire(key, Instant::now() + Duration::from_secs(1))
                .await
        });
        eventually_async(|| async { fixture.lock_waiter_pid().await.is_some() }).await;
        let pid = fixture.lock_waiter_pid().await.unwrap();
        if server_cancel {
            let cancelled: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)")
                .bind(pid)
                .fetch_one(&fixture.observer)
                .await
                .unwrap();
            assert!(cancelled);
        }
        assert!(pending.await.unwrap().is_err());
        legacy.rollback().await.unwrap();
        eventually_async(|| async { !fixture.session_exists(pid).await }).await;
        eventually(|| {
            registry.observe().managed.closing == 0 && registry.observe().local_key_cells == 0
        })
        .await;
        assert_eq!(registry.observe().managed.pool.size, 0);
        fixture.close().await;
    }
}

#[tokio::test]
async fn clean_unlock_ack_allows_reuse_without_residual_or_reentrant_lock() {
    let fixture = Fixture::new(settings(1)).await;
    let (installation, scope, key) = managed_key();
    let registry = fixture.store.connections();
    let holder = registry.acquire(key.clone(), deadline()).await.unwrap();
    let pid = fixture.managed_pid().await;
    assert!(!holder.release().await.lost);
    eventually(|| registry.observe().managed.pool.idle == 1).await;
    let mut legacy = fixture.observer.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
        .bind(installation.to_string()).bind(scope.to_string()).fetch_one(&mut *legacy).await.unwrap();
    assert!(acquired);
    legacy.rollback().await.unwrap();
    let again = registry.acquire(key, deadline()).await.unwrap();
    assert_eq!(fixture.managed_pid().await, pid);
    again.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn unlock_without_ack_closes_and_reports_uncertainty() {
    let mut configuration = settings(1);
    // Make the failure occur in unlock, without a preceding health probe.
    configuration.health_check_interval = Duration::from_secs(3600);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections();
    let lease = registry.acquire(managed_key().2, deadline()).await.unwrap();
    let pid = fixture.managed_pid().await;
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&fixture.observer)
        .await
        .unwrap();
    assert!(terminated);
    let status = lease.release().await;
    assert!(status.lost && status.closed);
    eventually_async(|| async { !fixture.session_exists(pid).await }).await;
    let fresh = registry.acquire(managed_key().2, deadline()).await.unwrap();
    assert_ne!(fixture.managed_pid().await, pid);
    fresh.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn healthy_retained_clone_holds_lock_after_caller_drop_but_status_receiver_does_not() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections().clone();
    let (installation, scope, key) = managed_key();
    let caller = registry.acquire(key.clone(), deadline()).await.unwrap();
    let retained = caller.clone();
    let status = caller.subscribe();
    drop(caller);
    let pending_registry = registry.clone();
    let pending =
        tokio::spawn(async move { pending_registry.acquire(key, deadline()).await.unwrap() });
    eventually(|| registry.observe().managed.local_queue == 1).await;
    let mut legacy = fixture.observer.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
        .bind(installation.to_string()).bind(scope.to_string()).fetch_one(&mut *legacy).await.unwrap();
    assert!(!acquired);
    legacy.rollback().await.unwrap();
    assert!(!retained.status().lost && !retained.status().closed);
    assert!(!pending.is_finished());
    retained.release().await;
    let next = timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(status.borrow().closed);
    assert!(!status.borrow().lost);
    next.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn loss_notifies_retained_owner_and_keeps_local_key_until_last_clone_releases() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections().clone();
    let (_, _, key) = managed_key();
    let lease = registry.acquire(key.clone(), deadline()).await.unwrap();
    let retained = lease.clone();
    let pending_registry = registry.clone();
    let pending =
        tokio::spawn(async move { pending_registry.acquire(key, deadline()).await.unwrap() });
    eventually(|| registry.observe().managed.local_queue == 1).await;
    let pid = fixture.managed_pid().await;
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&fixture.observer)
        .await
        .unwrap();
    assert!(terminated);
    let mut notification = lease.subscribe();
    timeout(Duration::from_secs(2), async {
        while !notification.borrow_and_update().lost {
            notification.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(lease.status().lost);
    assert!(!lease.status().closed);
    assert!(!pending.is_finished());
    let release = tokio::spawn(lease.release());
    assert!(!retained.status().closed);
    drop(retained);
    assert!(release.await.unwrap().closed);
    pending.await.unwrap().release().await;
    eventually(|| registry.observe().local_key_cells == 0).await;
    fixture.close().await;
}

#[tokio::test]
async fn dispatch_nonowner_requires_negative_pg_ack_and_budget_exhaustion_is_error() {
    let mut configuration = settings(1);
    configuration.dispatch_max_connections = Some(2);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections();
    let key = PgAdvisoryLockKey::dispatch(Uuid::now_v7(), Uuid::now_v7());
    let holder = registry
        .try_acquire(key.clone(), deadline())
        .await
        .unwrap()
        .unwrap();
    assert!(registry
        .try_acquire(key, deadline())
        .await
        .unwrap()
        .is_none());
    let other_key = PgAdvisoryLockKey::dispatch(Uuid::now_v7(), Uuid::now_v7());
    let other = registry
        .try_acquire(other_key, deadline())
        .await
        .unwrap()
        .unwrap();
    let error = registry
        .try_acquire(
            PgAdvisoryLockKey::dispatch(Uuid::now_v7(), Uuid::now_v7()),
            Instant::now() + Duration::from_millis(50),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("lane acquire deadline"));
    // Parent dispatch saturation cannot consume the managed child or business lanes.
    let managed = registry.acquire(managed_key().2, deadline()).await.unwrap();
    sqlx::query("SELECT 1")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(registry.observe().dispatch.active, 2);
    assert_eq!(registry.observe().managed.active, 1);
    managed.release().await;
    holder.release().await;
    other.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn compatibility_dispatch_and_template_lanes_admit_more_than_five_independent_runs() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections();
    assert_eq!(registry.observe().dispatch.pool.size, 0);
    assert_eq!(registry.observe().template_install.pool.size, 0);
    let mut leases = Vec::new();
    for _ in 0..6 {
        leases.push(
            registry
                .try_acquire(
                    PgAdvisoryLockKey::dispatch(Uuid::now_v7(), Uuid::now_v7()),
                    deadline(),
                )
                .await
                .unwrap()
                .unwrap(),
        );
        leases.push(
            registry
                .acquire(
                    PgAdvisoryLockKey::template_install(Uuid::now_v7()),
                    deadline(),
                )
                .await
                .unwrap(),
        );
    }
    let observation = registry.observe();
    assert_eq!(observation.dispatch.active, 6);
    assert_eq!(observation.template_install.active, 6);
    assert_eq!(observation.dispatch.pool.max_connections, None);
    assert_eq!(observation.template_install.pool.max_connections, None);
    for lease in leases {
        lease.release().await;
    }
    fixture.close().await;
}

#[tokio::test]
async fn direct_pool_execute_and_begin_still_obey_native_p_limit() {
    let fixture = Fixture::new(settings(1)).await;
    let held = fixture.store.pool().acquire().await.unwrap();
    assert!(timeout(
        Duration::from_millis(50),
        sqlx::query("SELECT 1").execute(fixture.store.pool())
    )
    .await
    .is_err());
    assert!(
        timeout(Duration::from_millis(50), fixture.store.pool().begin())
            .await
            .is_err()
    );
    assert_eq!(fixture.store.pool().size(), 1);
    drop(held);
    sqlx::query("SELECT 1")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    fixture
        .store
        .pool()
        .begin()
        .await
        .unwrap()
        .rollback()
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn idle_sessions_recycle_on_configured_timeout_and_fresh_sessions_use_the_same_factory() {
    let mut configuration = settings(1);
    configuration.business.idle_timeout = Some(Duration::from_millis(100));
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections();
    let first = registry.acquire(managed_key().2, deadline()).await.unwrap();
    let pid = fixture.managed_pid().await;
    first.release().await;
    eventually(|| registry.observe().managed.pool.size == 0).await;
    let second = registry.acquire(managed_key().2, deadline()).await.unwrap();
    assert_ne!(fixture.managed_pid().await, pid);
    second.release().await;
    fixture.close().await;
}
