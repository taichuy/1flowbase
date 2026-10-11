use super::*;
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_retirement_saturation() {
    let lifetime = Arc::new(SnapshotLifetime::default());
    let retirement = lifetime.close().unwrap();
    retirement.retire();
    drop(retirement);
    let mut visible = ManagedSnapshots::default();
    for index in 0..5000 {
        visible
            .retired_targets
            .insert(format!("retired-{index}"), lifetime.clone());
    }
    assert_eq!(visible.retired_targets.len(), 5000);
    assert!(visible.retired_targets["retired-0"].ensure_open().is_err());
    assert!(visible.retired_targets["retired-0"].acquire().is_err());
    let active = Arc::new(SnapshotLifetime::default());
    let references = (0..600)
        .map(|_| active.acquire().unwrap())
        .collect::<Vec<_>>();
    let retirement = active.close().unwrap();
    assert!(retirement.ensure_unreferenced().is_err());
    assert!(active.ensure_open().is_err());
    assert!(active.acquire().is_err());
    assert!(
        tokio::time::timeout(std::time::Duration::ZERO, active.wait_references())
            .await
            .is_err()
    );
    assert_eq!(active.reference_count(), 600);
    drop(references);
    active.wait_references().await;
    retirement.ensure_unreferenced().unwrap();
    retirement.retire();
    drop(retirement);
    assert!(active.ensure_open().is_err());
    assert!(active.acquire().is_err());
    assert_eq!(
        visible.retired_targets.len(),
        5000,
        "draining other resources never evicts retirement markers"
    );
}

#[tokio::test]
async fn root_2007_ac_010_lane_budgets_snapshot_capacity() {
    fn snapshot(id: usize) -> Arc<ManagedWorkspaceSnapshot> {
        Arc::new(ManagedWorkspaceSnapshot {
            lifetime: Default::default(),
            graph: Arc::new(EffectiveExtensionGraph::new(
                ExtensionBusVersion::V1,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ExtensionGraphFingerprint::new(format!("graph-{id}")),
            )),
            authority: ManagedGraphAuthority::new("fixture-policy".into()),
            bindings: BTreeMap::new(),
            lifecycle_plan: None,
        })
    }
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let composition = Arc::new(ManagedExtensionComposition::new(
        state.store.clone(),
        state.api_node_id.clone(),
        state.provider_runtime.runtime_backend().clone(),
        vec![],
    ));
    let workspace: Uuid = sqlx::query_scalar("select id from workspaces limit 1")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let user: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let installation = Uuid::now_v7();
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values($1,'runtime-extensions','acme','unbounded','1.0.0','acme.unbounded','1flowbase.extension-bus/v1','stdio_json','Unbounded','uploaded','unverified','valid','disabled','missing','{}',$2)").bind(installation).bind(user).execute(state.store.pool()).await.unwrap();
    sqlx::query("insert into plugin_assignments(id,installation_id,workspace_id,provider_code,assigned_by) values($1,$2,$3,'acme.unbounded',$4)").bind(Uuid::now_v7()).bind(installation).bind(workspace).bind(user).execute(state.store.pool()).await.unwrap();
    {
        let mut visible = composition.snapshots.lock().await;
        for i in 0..600 {
            visible.current.insert(Uuid::now_v7(), snapshot(i));
            visible
                .retained
                .insert(format!("retained-{i}"), vec![snapshot(1000 + i)]);
        }
        for i in 0..5000 {
            let lifetime = Arc::new(SnapshotLifetime::default());
            let retirement = lifetime.close().unwrap();
            retirement.retire();
            visible
                .retired_targets
                .insert(format!("retired-{i}"), lifetime);
        }
    }
    // Exercise actual candidate preparation beyond all previous host cardinality ceilings.
    composition
        .prepare_snapshot(
            workspace,
            &[],
            &[],
            &mut BTreeMap::new(),
            &mut Vec::new(),
            None,
        )
        .await
        .unwrap();
    composition
        .snapshots
        .lock()
        .await
        .current
        .insert(workspace, snapshot(9999));
    // Actual authority-locked publication retains its predecessor beyond the old ceiling.
    composition
        .rebuild_installation(installation)
        .await
        .unwrap();
    let candidate = composition.snapshot(workspace).await.unwrap();
    let references = (0..600)
        .map(|_| candidate.freeze_reference().unwrap())
        .collect::<Vec<_>>();
    let retirement = candidate.lifetime.close().unwrap();
    assert!(retirement.ensure_unreferenced().is_err());
    assert!(
        composition
            .prepare_snapshot(
                workspace,
                &[],
                &[],
                &mut BTreeMap::new(),
                &mut Vec::new(),
                None
            )
            .await
            .is_err(),
        "a closing exact snapshot cannot be rebuilt"
    );
    drop(references);
    retirement.ensure_unreferenced().unwrap();
    retirement.retire();
    drop(retirement);
    assert!(
        composition
            .prepare_snapshot(
                workspace,
                &[],
                &[],
                &mut BTreeMap::new(),
                &mut Vec::new(),
                None
            )
            .await
            .is_err(),
        "a retired exact snapshot cannot be rebuilt"
    );
    let visible = composition.snapshots.lock().await;
    assert_eq!(visible.current.len(), 601);
    assert_eq!(visible.retained.values().map(Vec::len).sum::<usize>(), 601);
    assert_eq!(visible.retired_targets.len(), 5000);
    assert!(visible.retired_targets["retired-0"].acquire().is_err());
    drop(visible);
    composition.close_owned_admission();
    composition
        .wait_for_shutdown(std::time::Duration::from_secs(5))
        .await
        .unwrap();
    composition.cleanup_after_shutdown().await;
    let visible = composition.snapshots.lock().await;
    assert!(
        visible.current.is_empty()
            && visible.retained.is_empty()
            && visible.retired_targets.is_empty()
    );
}

#[test]
fn root_2325_reference_overflow_preserves_count_and_recoverable_close() {
    let lifetime = Arc::new(SnapshotLifetime::default());
    lifetime.0.lock().unwrap().references = usize::MAX;
    assert!(lifetime.acquire().is_err());
    assert_eq!(lifetime.reference_count(), usize::MAX);
    lifetime.0.lock().unwrap().references = 0;
    let references = (0..600)
        .map(|_| lifetime.acquire().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lifetime.reference_count(), 600);
    let closing = lifetime.close().unwrap();
    assert!(closing.ensure_unreferenced().is_err());
    assert!(lifetime.acquire().is_err());
    drop(closing);
    lifetime.ensure_open().unwrap();
    drop(references);
    assert_eq!(lifetime.reference_count(), 0);
    let closing = lifetime.close().unwrap();
    closing.ensure_unreferenced().unwrap();
    closing.retire();
    drop(closing);
    assert!(lifetime.acquire().is_err());
}

#[tokio::test]
async fn hot_lifecycle_reference_release_retains_reclamation_wakeup() {
    let notify = Arc::new(tokio::sync::Notify::new());
    let lifetime = Arc::new(SnapshotLifetime::with_reclamation_notify(notify.clone()));
    let reference = lifetime.acquire().unwrap();
    drop(reference);
    // The drop precedes registration: notify_waiters would lose this required retry.
    tokio::time::timeout(std::time::Duration::from_secs(1), notify.notified())
        .await
        .unwrap();
    assert_eq!(lifetime.reference_count(), 0);
}

#[tokio::test]
async fn hot_lifecycle_automatic_reclamation_retries_after_reference_release() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let composition = Arc::new(ManagedExtensionComposition::new(
        state.store.clone(),
        state.api_node_id.clone(),
        state.provider_runtime.runtime_backend().clone(),
        vec![],
    ));
    let make_snapshot = |name: &str| {
        Arc::new(ManagedWorkspaceSnapshot {
            lifetime: Arc::new(SnapshotLifetime::with_reclamation_notify(
                composition.reclamation.notify.clone(),
            )),
            graph: Arc::new(EffectiveExtensionGraph::new(
                ExtensionBusVersion::V1,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ExtensionGraphFingerprint::new(name.to_owned()),
            )),
            authority: ManagedGraphAuthority::new("fixture-policy".into()),
            bindings: BTreeMap::new(),
            lifecycle_plan: None,
        })
    };
    let retained = make_snapshot("retained");
    let current = make_snapshot("current");
    let reference = retained.freeze_reference().unwrap();
    {
        let mut snapshots = composition.snapshots.lock().await;
        snapshots
            .retained
            .insert("retained".into(), vec![retained.clone()]);
        snapshots.current.insert(Uuid::now_v7(), current.clone());
    }
    // Deterministically prove a sweep protects the reference before starting its worker.
    composition.reclaim_retained_snapshots().await;
    assert_eq!(composition.snapshots.lock().await.retained.len(), 1);
    composition.start_reclamation();
    drop(reference);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !composition.snapshots.lock().await.retained.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(retained.freeze_reference().is_err());
    assert!(current.freeze_reference().is_ok());
    composition.close_owned_admission();
    composition
        .wait_owned_shutdown(std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
