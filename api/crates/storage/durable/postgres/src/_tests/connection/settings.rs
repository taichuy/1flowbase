use super::*;
use storage_durable_postgres::PgInstanceConnectionBudget;

#[test]
fn compatibility_keeps_p_and_marks_unallocated_lanes_explicitly() {
    let settings = settings(5);
    settings.validate().unwrap();
    assert_eq!(settings.business.max_connections, 5);
    assert_eq!(settings.managed_limit(), 5);
    assert_eq!(settings.dispatch_max_connections, None);
    assert_eq!(settings.template_install_max_connections, None);
    assert_eq!(settings.temporary_max_connections, None);
    assert_eq!(settings.instance_budget, None);
}

#[test]
fn full_budget_requires_every_lane_and_checks_sum_reserve_and_overflow() {
    let mut settings = settings(5);
    settings.instance_budget = Some(PgInstanceConnectionBudget {
        max_connections: 20,
        reserved_connections: 2,
    });
    assert!(settings.validate().is_err());
    settings.managed_max_connections = Some(5);
    settings.dispatch_max_connections = Some(4);
    settings.template_install_max_connections = Some(2);
    settings.temporary_max_connections = Some(2);
    settings.validate().unwrap(); // 5 + 5 + 4 + 2 + 2 + 2 = 20
    settings.instance_budget.as_mut().unwrap().max_connections = 19;
    assert!(settings.validate().is_err());
    settings.instance_budget.as_mut().unwrap().max_connections = u32::MAX;
    settings
        .instance_budget
        .as_mut()
        .unwrap()
        .reserved_connections = u32::MAX;
    assert!(settings
        .validate()
        .unwrap_err()
        .to_string()
        .contains("overflow"));
}

#[test]
fn partial_budgets_also_reject_zero_and_known_allocation_overflow() {
    for lane in 0..4 {
        let mut settings = settings(5);
        match lane {
            0 => settings.managed_max_connections = Some(0),
            1 => settings.dispatch_max_connections = Some(0),
            2 => settings.template_install_max_connections = Some(0),
            _ => settings.temporary_max_connections = Some(0),
        }
        assert!(settings.validate().is_err());
    }
    let mut settings = settings(5);
    settings.dispatch_max_connections = Some(u32::MAX);
    assert!(settings
        .validate()
        .unwrap_err()
        .to_string()
        .contains("overflow"));
    settings.dispatch_max_connections = None;
    settings.business.acquire_timeout = Duration::ZERO;
    assert!(settings.validate().is_err());
    settings.business.acquire_timeout = Duration::from_secs(1);
    settings.health_check_interval = Duration::ZERO;
    assert!(settings.validate().is_err());
    settings.health_check_interval = Duration::MAX;
    assert!(settings.validate().is_err());
    settings.health_check_interval = Duration::from_secs(1);
    settings.business.idle_timeout = Some(Duration::MAX);
    assert!(settings.validate().is_err());
}

#[tokio::test]
async fn store_clones_actor_and_temporary_scope_share_factories_and_key_cells() {
    let fixture = Fixture::new(settings(1)).await;
    let registry = fixture.store.connections().clone();
    let clone = fixture.store.clone();
    let actor = clone.for_actor(domain::ActorContext::root(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "root",
    ));
    let temporary = registry
        .temporary_pool(PgPoolOptions::new(), deadline())
        .await
        .unwrap();
    let scoped = actor.with_portable_template_pool(temporary.pool().clone());
    let (_, _, key) = managed_key();
    let lease = registry.acquire(key.clone(), deadline()).await.unwrap();
    for store in [&clone, &actor, &scoped] {
        let observations = store.connections().observe();
        assert_eq!(observations.managed.active, 1);
        assert_eq!(observations.temporary.active, 1);
        assert_eq!(observations.local_key_cells, 1);
        assert_eq!(observations.business.max_connections, Some(1));
    }
    let pending_registry = scoped.connections().clone();
    let pending = tokio::spawn(async move { pending_registry.acquire(key, deadline()).await });
    eventually(|| registry.observe().managed.local_queue == 1).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    lease.release().await;
    eventually(|| registry.observe().local_key_cells == 0).await;
    temporary.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn mismatched_native_pool_limit_is_rejected_before_any_lane_is_used() {
    let fixture = Fixture::new(settings(1)).await;
    assert!(PgControlPlaneStore::with_connection_settings(
        fixture.store.pool().clone(),
        settings(2)
    )
    .is_err());
    fixture.close().await;
}
