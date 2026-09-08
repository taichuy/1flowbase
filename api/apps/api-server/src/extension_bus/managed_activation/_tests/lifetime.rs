use super::*;
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_retirement_saturation() {
    let lifetime = Arc::new(SnapshotLifetime::default());
    let retirement = lifetime.close().unwrap();
    retirement.retire();
    drop(retirement);
    let mut visible = ManagedSnapshots::default();
    for index in 0..MAX_RETIRED_TARGETS {
        visible
            .retired_targets
            .insert(format!("retired-{index}"), lifetime.clone());
    }
    assert!(visible
        .ensure_retirement_capacity("another-target")
        .is_err());
    assert!(visible.ensure_candidate_capacity(Uuid::now_v7()).is_err());
    assert_eq!(visible.retired_targets.len(), MAX_RETIRED_TARGETS);
    assert!(visible.retired_targets["retired-0"].ensure_open().is_err());
    assert!(visible.retired_targets["retired-0"].acquire().is_err());
    let active = Arc::new(SnapshotLifetime::default());
    let references = (0..256)
        .map(|_| active.acquire().unwrap())
        .collect::<Vec<_>>();
    assert!(active.acquire().is_err());
    active.close_for_shutdown();
    assert!(active.acquire().is_err());
    assert!(
        tokio::time::timeout(std::time::Duration::ZERO, active.wait_references())
            .await
            .is_err()
    );
    assert_eq!(active.reference_count(), 256);
    drop(references);
    active.wait_references().await;
    assert_eq!(
        visible.retired_targets.len(),
        MAX_RETIRED_TARGETS,
        "draining other resources never evicts retirement markers"
    );
}

#[test]
fn root_2007_ac_010_lane_budgets_snapshot_capacity() {
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
    let workspace = Uuid::now_v7();
    let mut visible = ManagedSnapshots::default();
    let current = snapshot(9999);
    visible.current.insert(workspace, current.clone());
    for i in 0..MAX_RETAINED_SNAPSHOTS {
        visible
            .retained
            .insert(format!("graph-{i}"), vec![snapshot(i)]);
    }
    assert!(visible
        .ensure_publication_capacity(&[(workspace, snapshot(10000))].into_iter().collect())
        .is_err());
    assert!(Arc::ptr_eq(&visible.current[&workspace], &current));
    assert_eq!(
        visible.retained.values().map(Vec::len).sum::<usize>(),
        MAX_RETAINED_SNAPSHOTS
    );
    assert!(
        visible
            .ensure_publication_capacity(&[(workspace, current.clone())].into_iter().collect())
            .is_ok(),
        "unchanged snapshot does not spend retention capacity"
    );
    for _ in 1..MAX_CURRENT_WORKSPACES {
        visible.current.insert(Uuid::now_v7(), current.clone());
    }
    assert!(visible.ensure_candidate_capacity(Uuid::now_v7()).is_err());
    assert!(visible
        .ensure_publication_capacity(&[(Uuid::now_v7(), current)].into_iter().collect())
        .is_err());
    assert_eq!(visible.current.len(), MAX_CURRENT_WORKSPACES);
}
