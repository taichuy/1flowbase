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
    let (probed, resume) = second_registry.pause_next_retry_delay();
    let pending_key = key.clone();
    let pending = tokio::spawn(async move {
        second_registry
            .acquire(pending_key, deadline())
            .await
            .unwrap()
    });
    timeout(Duration::from_secs(2), probed)
        .await
        .unwrap()
        .unwrap();
    assert!(fixture.lock_waiter_pid().await.is_none());
    assert!(!pending.is_finished());
    resume.send(()).unwrap();
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
    let (probed, resume) = own_registry.pause_next_retry_delay();
    let pending = tokio::spawn(async move { own_registry.acquire(key, deadline()).await.unwrap() });
    eventually(|| fixture.store.connections().observe().local_key_cells == 2).await;
    // The other key uses this node's one session. Releasing it admits a short PG probe.
    other.release().await;
    timeout(Duration::from_secs(2), probed)
        .await
        .unwrap()
        .unwrap();
    assert!(fixture.lock_waiter_pid().await.is_none());
    assert!(!pending.is_finished());
    resume.send(()).unwrap();
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
    let (_, _, key) = managed_key();
    let registry = fixture.store.connections().clone();
    // Real PG has acquired the lock, but the caller has not received ownership yet.
    // The fixture gate makes cancellation at that handoff deterministic; it is not
    // evidence of a real network response-loss experiment.
    let (probed, _resume) = registry.pause_next_probe_result();
    let pending_registry = registry.clone();
    let pending_key = key.clone();
    let pending =
        tokio::spawn(async move { pending_registry.acquire(pending_key, deadline()).await });
    assert!(timeout(Duration::from_secs(2), probed)
        .await
        .unwrap()
        .unwrap());
    let pid = fixture.managed_pid().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
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
async fn unacknowledged_probe_deadline_discards_owned_connection_before_any_reuse() {
    for cap in [1, 2] {
        let mut configuration = settings(1);
        configuration.managed_max_connections = Some(cap);
        let fixture = Fixture::new(configuration).await;
        let (_, _, key) = managed_key();
        let registry = fixture.store.connections().clone();
        let (probed, _resume) = registry.pause_next_probe_result();
        let pending_registry = registry.clone();
        let pending_key = key.clone();
        let pending = tokio::spawn(async move {
            pending_registry
                .acquire(pending_key, Instant::now() + Duration::from_secs(1))
                .await
        });
        assert!(timeout(Duration::from_secs(2), probed)
            .await
            .unwrap()
            .unwrap());
        let pid = fixture.managed_pid().await;
        assert!(pending.await.unwrap().is_err());
        eventually_async(|| async { !fixture.session_exists(pid).await }).await;
        eventually(|| {
            registry.observe().managed.closing == 0 && registry.observe().local_key_cells == 0
        })
        .await;
        assert_eq!(registry.observe().managed.pool.size, 0);
        let fresh = registry.acquire(key, deadline()).await.unwrap();
        assert_ne!(fixture.managed_pid().await, pid);
        fresh.release().await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn repeated_timeouts_and_cancels_do_not_accumulate_server_waiters_while_remote_holder_stays()
{
    for cap in [1, 2] {
        let mut configuration = settings(1);
        configuration.managed_max_connections = Some(cap);
        let fixture = Fixture::new(configuration).await;
        let registry = fixture.store.connections().clone();
        for transaction in [true, false] {
            let (installation, scope, key) = managed_key();
            let holder = fixture
                .remote_holder(installation, scope, transaction)
                .await;
            for attempt in 0..6 {
                let cancelled = attempt % 2 == 0;
                let gate = cancelled.then(|| registry.pause_next_retry_delay());
                let pending_registry = registry.clone();
                let pending_key = key.clone();
                let pending = tokio::spawn(async move {
                    let limit = if cancelled {
                        deadline()
                    } else {
                        Instant::now() + Duration::from_millis(250)
                    };
                    pending_registry.acquire(pending_key, limit).await
                });
                if let Some((probed, _resume)) = gate {
                    // Cancel after this attempt's actual false ACK and native clean return.
                    timeout(Duration::from_secs(2), probed)
                        .await
                        .unwrap()
                        .unwrap();
                    let snapshot = fixture.server_lock_snapshot(&holder).await;
                    assert!(snapshot.no_waiter_accumulation(cap), "{snapshot:?}");
                    pending.abort();
                    assert!(pending.await.unwrap_err().is_cancelled());
                } else {
                    eventually_async(|| async { fixture.has_managed_probe().await }).await;
                    let snapshot = fixture.server_lock_snapshot(&holder).await;
                    assert!(snapshot.no_waiter_accumulation(cap), "{snapshot:?}");
                    let error = pending.await.unwrap().unwrap_err();
                    assert!(error.to_string().contains("lock acquire deadline"));
                }
                eventually(|| {
                    let observation = registry.observe();
                    observation.local_key_cells == 0
                        && observation.managed.active == 0
                        && observation.managed.closing == 0
                })
                .await;
                // Inspect PG BEFORE releasing the holder. Rust counters alone cannot
                // distinguish a closed client from a backend still blocked on its key.
                let snapshot = fixture.server_lock_snapshot(&holder).await;
                assert!(
                    snapshot.no_waiter_accumulation(cap),
                    "cap={cap} attempt={attempt}: {snapshot:?}"
                );
                timeout(
                    Duration::from_secs(2),
                    sqlx::query("SELECT 1").execute(fixture.store.pool()),
                )
                .await
                .unwrap()
                .unwrap();
            }
            assert!(fixture.server_lock_snapshot(&holder).await.holder_holds);
            holder.release().await;
            registry
                .acquire(key, deadline())
                .await
                .unwrap()
                .release()
                .await;
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn legacy_blocking_probe_negative_exceeds_cap_in_same_server_waiter_oracle() {
    for cap in [1, 2] {
        let mut configuration = settings(1);
        configuration.managed_max_connections = Some(cap);
        let fixture = Fixture::new(configuration).await;
        let registry = fixture.store.connections().clone();
        // Select only the old SQL in the actual production factory/Session cleanup.
        // Its test-only per-session interval=0 reproduces the documented default.
        registry.use_legacy_blocking_probe();
        let (installation, scope, key) = managed_key();
        let holder = fixture.remote_holder(installation, scope, true).await;
        for cancelled in 0..3 {
            let pending_registry = registry.clone();
            let pending_key = key.clone();
            let pending =
                tokio::spawn(
                    async move { pending_registry.acquire(pending_key, deadline()).await },
                );
            eventually_async(|| async {
                fixture
                    .server_lock_snapshot(&holder)
                    .await
                    .blocked_pids
                    .len()
                    == cancelled + 1
            })
            .await;
            pending.abort();
            assert!(pending.await.unwrap_err().is_cancelled());
            eventually(|| {
                let observation = registry.observe();
                observation.local_key_cells == 0
                    && observation.managed.active == 0
                    && observation.managed.closing == 0
            })
            .await;
            let snapshot = fixture.server_lock_snapshot(&holder).await;
            assert!(snapshot.holder_holds);
            assert_eq!(snapshot.blocked_pids.len(), cancelled + 1, "{snapshot:?}");
            assert!(
                !snapshot.no_waiter_accumulation(cap),
                "legacy must fail the candidate oracle: {snapshot:?}"
            );
        }
        let snapshot = fixture.server_lock_snapshot(&holder).await;
        assert!(
            snapshot.coordinator_pids.len() > cap as usize,
            "{snapshot:?}"
        );
        // Let the owned remote transaction end only after observing all three stale
        // server waiters. Then wait for their late Terminate/socket handling for cleanup.
        holder.release().await;
        eventually_async(|| async {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock'")
                .fetch_one(&fixture.observer).await.unwrap() == 0
        }).await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn false_probe_returns_and_reuses_connection_but_retains_fifo_head_during_backoff() {
    let mut configuration = settings(1);
    configuration.managed_max_connections = Some(2);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections().clone();
    let (installation, scope, key) = managed_key();
    let remote = fixture.remote_holder(installation, scope, true).await;
    let (backoff, resume) = registry.pause_next_retry_delay();
    let pending_registry = registry.clone();
    let pending_key = key.clone();
    let head = tokio::spawn(async move {
        pending_registry
            .acquire(pending_key, deadline())
            .await
            .unwrap()
    });
    timeout(Duration::from_secs(2), backoff)
        .await
        .unwrap()
        .unwrap();
    let pid = fixture.managed_pid().await;
    let observation = registry.observe();
    assert_eq!(observation.managed.active, 0);
    assert_eq!(observation.managed.pool.checked_out, 0);
    assert_eq!(observation.managed.pool.idle, 1);
    assert_eq!(observation.local_key_cells, 1);
    let mut followers = Vec::new();
    for index in 0..3 {
        let pending_registry = registry.clone();
        let pending_key = key.clone();
        followers.push(tokio::spawn(async move {
            pending_registry
                .acquire(pending_key, deadline())
                .await
                .unwrap()
        }));
        eventually(|| registry.observe().managed.local_queue == index + 1).await;
    }
    // cap=2 leaves spare capacity: followers still must not borrow. A different key
    // can reuse the exact idle PID while the original FIFO head owns only its turn.
    let child = registry.acquire(managed_key().2, deadline()).await.unwrap();
    assert_eq!(fixture.managed_pid().await, pid);
    assert_eq!(registry.observe().managed.pool.size, 1);
    timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(fixture.store.pool()),
    )
    .await
    .unwrap()
    .unwrap();
    child.release().await;
    let snapshot = fixture.server_lock_snapshot(&remote).await;
    assert!(snapshot.no_waiter_accumulation(2), "{snapshot:?}");
    assert!(followers.iter().all(|follower| !follower.is_finished()));
    remote.release().await;
    assert!(followers.iter().all(|follower| !follower.is_finished()));
    resume.send(()).unwrap();
    let lease = timeout(Duration::from_secs(2), head)
        .await
        .unwrap()
        .unwrap();
    assert!(followers.iter().all(|follower| !follower.is_finished()));
    lease.release().await;
    for follower in followers {
        timeout(Duration::from_secs(2), follower)
            .await
            .unwrap()
            .unwrap()
            .release()
            .await;
    }
    assert_eq!(registry.observe().local_key_cells, 0);
    fixture.close().await;
}

#[tokio::test]
async fn absolute_deadline_expires_in_connection_free_retry_phase_without_discarding_clean_session()
{
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections().clone();
    let (installation, scope, key) = managed_key();
    let remote = fixture.remote_holder(installation, scope, true).await;
    let (backoff, _resume) = registry.pause_next_retry_delay();
    let pending_registry = registry.clone();
    let pending = tokio::spawn(async move {
        pending_registry
            .acquire(key, Instant::now() + Duration::from_secs(1))
            .await
    });
    timeout(Duration::from_secs(2), backoff)
        .await
        .unwrap()
        .unwrap();
    let pid = fixture.managed_pid().await;
    let error = timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("lock acquire deadline"));
    let observation = registry.observe();
    assert_eq!(observation.local_key_cells, 0);
    assert_eq!(observation.managed.active, 0);
    assert_eq!(observation.managed.closing, 0);
    assert_eq!(observation.managed.pool.idle, 1);
    let snapshot = fixture.server_lock_snapshot(&remote).await;
    assert!(snapshot.no_waiter_accumulation(1), "{snapshot:?}");
    assert_eq!(snapshot.coordinator_pids, vec![pid]);
    let other = registry.acquire(managed_key().2, deadline()).await.unwrap();
    assert_eq!(fixture.managed_pid().await, pid);
    other.release().await;
    remote.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn acquire_waiting_keeps_template_fifo_beyond_physical_timeout_then_progresses() {
    for lane_cap in [None, Some(2)] {
        let mut configuration = settings(1);
        let physical_timeout = configuration.business.acquire_timeout;
        configuration.template_install_max_connections = lane_cap;
        let fixture = Fixture::new(configuration).await;
        let registry = fixture.store.connections().clone();
        let workspace = Uuid::now_v7();
        let key = PgAdvisoryLockKey::template_install(workspace);
        let remote = fixture.remote_template_holder(workspace, false).await;
        // Saturate P=1 as a control: logical queue/probes must use only their own lane.
        let mut business = fixture.store.pool().acquire().await.unwrap();
        let (backoff, resume) = registry.pause_next_retry_delay();
        let head_registry = registry.clone();
        let head_key = key.clone();
        let mut head = tokio::spawn(async move { head_registry.acquire_waiting(head_key).await });
        timeout(physical_timeout * 2, backoff)
            .await
            .unwrap()
            .unwrap();
        let mut followers = Vec::new();
        for index in 0..3 {
            let follower_registry = registry.clone();
            let follower_key = key.clone();
            followers.push(tokio::spawn(async move {
                follower_registry.acquire_waiting(follower_key).await
            }));
            eventually(|| registry.observe().template_install.local_queue == index + 1).await;
        }
        // Keep the normal physical timeout for healthy connections. Spend 101% of that
        // window only at the connection-free gate; neither the head nor its followers
        // may expire/rejoin the FIFO while no physical attempt is running.
        let logical_wait = physical_timeout + physical_timeout / 100;
        assert!(timeout(logical_wait, &mut head).await.is_err());
        assert!(followers.iter().all(|follower| !follower.is_finished()));
        let observation = registry.observe();
        assert_eq!(observation.local_key_cells, 1);
        assert_eq!(observation.template_install.local_queue, 3);
        assert_eq!(observation.template_install.active, 0);
        assert_eq!(observation.template_install.pool.checked_out, 0);
        let snapshot = fixture.server_lock_snapshot(&remote).await;
        assert!(
            snapshot.holder_holds && snapshot.blocked_pids.is_empty(),
            "{snapshot:?}"
        );
        assert_eq!(
            snapshot.coordinator_pids.len(),
            if lane_cap.is_some() { 1 } else { 0 }
        );
        timeout(
            Duration::from_secs(2),
            sqlx::query("SELECT 1").execute(&mut *business),
        )
        .await
        .unwrap()
        .unwrap();
        // Install the next gate before resuming. Reaching it requires another actual
        // PG false ACK and clean return after the logical wait exceeded one physical
        // window, without spending a second arbitrary interval polling real connections.
        let (retried, resume_retry) = registry.pause_next_retry_delay();
        resume.send(()).unwrap();
        timeout(physical_timeout * 2, retried)
            .await
            .unwrap()
            .unwrap();
        assert!(!head.is_finished());
        assert!(followers.iter().all(|follower| !follower.is_finished()));
        let observation = registry.observe();
        assert_eq!(observation.local_key_cells, 1);
        assert_eq!(observation.template_install.local_queue, 3);
        assert_eq!(observation.template_install.active, 0);
        assert_eq!(observation.template_install.pool.checked_out, 0);
        let snapshot = fixture.server_lock_snapshot(&remote).await;
        assert!(
            snapshot.holder_holds && snapshot.blocked_pids.is_empty(),
            "{snapshot:?}"
        );
        assert_eq!(
            snapshot.coordinator_pids.len(),
            if lane_cap.is_some() { 1 } else { 0 }
        );
        remote.release().await;
        resume_retry.send(()).unwrap();
        let lease = timeout(physical_timeout * 2, head)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(followers.iter().all(|follower| !follower.is_finished()));
        lease.release().await;
        for follower in followers {
            timeout(physical_timeout * 2, follower)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .release()
                .await;
        }
        assert_eq!(registry.observe().local_key_cells, 0);
        drop(business);
        fixture.close().await;
    }
}

#[tokio::test]
async fn acquire_waiting_cancellation_leaves_no_template_server_waiter_or_local_owner() {
    for lane_cap in [None, Some(1)] {
        let mut configuration = settings(1);
        configuration.business.acquire_timeout = Duration::from_millis(250);
        configuration.template_install_max_connections = lane_cap;
        let fixture = Fixture::new(configuration).await;
        let registry = fixture.store.connections().clone();
        let workspace = Uuid::now_v7();
        let key = PgAdvisoryLockKey::template_install(workspace);
        let remote = fixture.remote_template_holder(workspace, true).await;
        for _ in 0..3 {
            let (backoff, _resume) = registry.pause_next_retry_delay();
            let pending_registry = registry.clone();
            let pending_key = key.clone();
            let pending =
                tokio::spawn(async move { pending_registry.acquire_waiting(pending_key).await });
            timeout(Duration::from_secs(2), backoff)
                .await
                .unwrap()
                .unwrap();
            pending.abort();
            assert!(pending.await.unwrap_err().is_cancelled());
            eventually(|| {
                let observation = registry.observe();
                observation.local_key_cells == 0
                    && observation.template_install.local_queue == 0
                    && observation.template_install.active == 0
                    && observation.template_install.closing == 0
            })
            .await;
            if lane_cap.is_none() {
                eventually_async(|| async {
                    fixture
                        .server_lock_snapshot(&remote)
                        .await
                        .coordinator_pids
                        .is_empty()
                })
                .await;
            }
            let snapshot = fixture.server_lock_snapshot(&remote).await;
            assert!(snapshot.no_waiter_accumulation(1), "{snapshot:?}");
        }
        remote.release().await;
        registry.acquire_waiting(key).await.unwrap().release().await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn acquire_waiting_probe_timeout_returns_error_and_discards_unknown_session_without_retry() {
    let mut configuration = settings(1);
    configuration.business.acquire_timeout = Duration::from_millis(250);
    configuration.template_install_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections().clone();
    let key = PgAdvisoryLockKey::template_install(Uuid::now_v7());
    let (probed, _resume) = registry.pause_next_probe_result();
    let pending_registry = registry.clone();
    let pending_key = key.clone();
    let pending = tokio::spawn(async move { pending_registry.acquire_waiting(pending_key).await });
    // Real PG true, before ownership handoff; the phase gate simulates an unknown
    // completion, not an actual network response-loss experiment.
    assert!(timeout(Duration::from_secs(2), probed)
        .await
        .unwrap()
        .unwrap());
    let pid = fixture.template_pid().await;
    let error = timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("lock acquire deadline"));
    eventually_async(|| async { !fixture.session_exists(pid).await }).await;
    eventually(|| {
        let observation = registry.observe();
        observation.local_key_cells == 0
            && observation.template_install.pool.size == 0
            && observation.template_install.active == 0
            && observation.template_install.closing == 0
    })
    .await;
    let fresh = registry.acquire_waiting(key).await.unwrap();
    assert_ne!(fixture.template_pid().await, pid);
    fresh.release().await;
    fixture.close().await;
}

#[tokio::test]
async fn acquire_waiting_still_errors_when_finite_template_lane_has_no_physical_slot() {
    let mut configuration = settings(1);
    configuration.business.acquire_timeout = Duration::from_millis(250);
    configuration.template_install_max_connections = Some(1);
    let fixture = Fixture::new(configuration).await;
    let registry = fixture.store.connections();
    let holder = registry
        .acquire_waiting(PgAdvisoryLockKey::template_install(Uuid::now_v7()))
        .await
        .unwrap();
    let error = timeout(
        Duration::from_secs(2),
        registry.acquire_waiting(PgAdvisoryLockKey::template_install(Uuid::now_v7())),
    )
    .await
    .expect("physical admission must have a finite timeout")
    .unwrap_err();
    assert!(
        error.to_string().contains("lane acquire deadline")
            || matches!(
                error.downcast_ref::<sqlx::Error>(),
                Some(sqlx::Error::PoolTimedOut)
            ),
        "{error:#}"
    );
    assert!(!holder.status().lost && !holder.status().closed);
    let observation = registry.observe();
    assert_eq!(observation.local_key_cells, 1);
    assert_eq!(observation.template_install.active, 1);
    assert_eq!(observation.template_install.pool.size, 1);
    sqlx::query("SELECT 1")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    holder.release().await;
    fixture.close().await;
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
