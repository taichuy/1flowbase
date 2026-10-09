use super::*;
use control_plane_contracts::ports::{FrontstagePageRepository, MoveFrontstagePageInput};

#[tokio::test]
async fn relative_page_moves_handle_equal_ranks_and_repeated_tail_drops() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let ids = [
        Uuid::from_u128(100),
        Uuid::from_u128(200),
        Uuid::from_u128(300),
    ];
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("insert into frontstage_pages (id, workspace_id, kind, title, placement, slug, rank) values ($1, $2, 'group', 'Column', 'topbar', $3, '002500')")
            .bind(id).bind(workspace_id).bind(format!("column-{index}"))
            .execute(&pool).await.unwrap();
    }
    let sidebar_id = Uuid::from_u128(400);
    sqlx::query("insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'group', 'Sidebar', 'sidebar', '002500')")
        .bind(sidebar_id).bind(workspace_id).execute(&pool).await.unwrap();
    let store = crate::PgControlPlaneStore::new(pool.clone());
    for (source, target, after, expected) in [
        (ids[0], ids[2], true, vec![ids[1], ids[2], ids[0]]),
        (ids[1], ids[0], true, vec![ids[2], ids[0], ids[1]]),
        (ids[2], ids[1], true, vec![ids[0], ids[1], ids[2]]),
        (ids[2], ids[0], false, vec![ids[2], ids[0], ids[1]]),
    ] {
        store
            .move_frontstage_page(&MoveFrontstagePageInput {
                workspace_id,
                actor_user_id: Uuid::nil(),
                page_id: source,
                parent_id: None,
                rank: String::new(),
                before_id: (!after).then_some(target),
                after_id: after.then_some(target),
                placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
                slug: Some(format!(
                    "column-{}",
                    ids.iter().position(|id| *id == source).unwrap()
                )),
                descendant_placement: None,
            })
            .await
            .unwrap();
        let mut records = store.list_frontstage_pages(workspace_id).await.unwrap();
        records.retain(|record| ids.contains(&record.id));
        records.sort_by(|left, right| left.rank.cmp(&right.rank).then(left.id.cmp(&right.id)));
        assert_eq!(
            records.iter().map(|record| record.id).collect::<Vec<_>>(),
            expected
        );
        assert!(records.windows(2).all(|pair| pair[0].rank < pair[1].rank));
    }
    let before = store.list_frontstage_pages(workspace_id).await.unwrap();
    for target in [sidebar_id, ids[0], Uuid::from_u128(999)] {
        assert!(store
            .move_frontstage_page(&MoveFrontstagePageInput {
                workspace_id,
                actor_user_id: Uuid::nil(),
                page_id: ids[0],
                parent_id: None,
                rank: String::new(),
                before_id: None,
                after_id: Some(target),
                placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
                slug: Some("column-0".into()),
                descendant_placement: None,
            })
            .await
            .is_err());
    }
    let after = store.list_frontstage_pages(workspace_id).await.unwrap();
    assert_eq!(
        before
            .iter()
            .map(|record| (record.id, &record.rank))
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(|record| (record.id, &record.rank))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        store
            .get_frontstage_page(workspace_id, sidebar_id)
            .await
            .unwrap()
            .unwrap()
            .rank,
        "002500"
    );
}

#[tokio::test]
async fn relative_page_moves_reparent_and_order_atomically() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let source_group = Uuid::from_u128(500);
    let target_group = Uuid::from_u128(600);
    for id in [source_group, target_group] {
        sqlx::query("insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'group', 'Group', 'sidebar', '001000')")
            .bind(id).bind(workspace_id).execute(&pool).await.unwrap();
    }
    let source = Uuid::from_u128(700);
    let first = Uuid::from_u128(800);
    let second = Uuid::from_u128(900);
    for (id, parent) in [
        (source, source_group),
        (first, target_group),
        (second, target_group),
    ] {
        sqlx::query("insert into frontstage_pages (id, workspace_id, parent_id, kind, title, placement, rank) values ($1, $2, $3, 'page', 'Page', 'sidebar', '002500')")
            .bind(id).bind(workspace_id).bind(parent).execute(&pool).await.unwrap();
    }
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let moved = store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: Some(target_group),
            rank: String::new(),
            before_id: Some(second),
            after_id: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Sidebar,
            slug: None,
            descendant_placement: None,
        })
        .await
        .unwrap();
    assert_eq!(moved.parent_id, Some(target_group));
    let mut records = store.list_frontstage_pages(workspace_id).await.unwrap();
    records.retain(|record| record.parent_id == Some(target_group));
    records.sort_by(|left, right| left.rank.cmp(&right.rank).then(left.id.cmp(&right.id)));
    assert_eq!(
        records.iter().map(|record| record.id).collect::<Vec<_>>(),
        vec![first, source, second]
    );
    assert!(records.windows(2).all(|pair| pair[0].rank < pair[1].rank));
    let original_rank = moved.rank.clone();
    assert!(store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: Some(source_group),
            rank: String::new(),
            before_id: None,
            after_id: Some(first),
            placement: domain::frontstage::FrontstageNavigationPlacement::Sidebar,
            slug: None,
            descendant_placement: None,
        })
        .await
        .is_err());
    let unchanged = store
        .get_frontstage_page(workspace_id, source)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.parent_id, Some(target_group));
    assert_eq!(unchanged.rank, original_rank);
}

#[tokio::test]
async fn whole_group_moves_convert_navigation_without_losing_descendants() {
    use domain::frontstage::FrontstageNavigationPlacement::{Sidebar, Topbar};
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let source = Uuid::from_u128(1000);
    let destination = Uuid::from_u128(1100);
    let nested = Uuid::from_u128(1200);
    let leaf = Uuid::from_u128(1300);
    let topbar_leaf = Uuid::from_u128(1400);
    for (id, parent, kind, placement, slug) in [
        (source, None, "group", "topbar", Some("move-source")),
        (
            destination,
            None,
            "group",
            "topbar",
            Some("move-destination"),
        ),
        (nested, Some(source), "group", "sidebar", None),
        (leaf, Some(nested), "page", "sidebar", None),
        (topbar_leaf, Some(source), "page", "topbar", None),
    ] {
        sqlx::query("insert into frontstage_pages (id, workspace_id, parent_id, kind, title, placement, slug, rank) values ($1, $2, $3, $4, 'Move fixture', $5, $6, '001000')")
            .bind(id).bind(workspace_id).bind(parent).bind(kind).bind(placement).bind(slug)
            .execute(&pool).await.unwrap();
    }
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let moved = store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: Some(destination),
            rank: "001000".into(),
            before_id: None,
            after_id: None,
            placement: Sidebar,
            slug: None,
            descendant_placement: Some(Sidebar),
        })
        .await
        .unwrap();
    assert_eq!(moved.parent_id, Some(destination));
    assert_eq!(moved.placement, Sidebar);
    assert_eq!(moved.slug, None);
    let records = store.list_frontstage_pages(workspace_id).await.unwrap();
    for (id, parent) in [(nested, source), (leaf, nested), (topbar_leaf, source)] {
        let record = records.iter().find(|record| record.id == id).unwrap();
        assert_eq!(record.parent_id, Some(parent));
        assert_eq!(record.placement, Sidebar);
        assert_eq!(record.slug, None);
    }
    // Promotion back to a route changes only the group root's navigation layer.
    let promoted = store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: None,
            rank: String::new(),
            before_id: None,
            after_id: Some(destination),
            placement: Topbar,
            slug: Some("promoted-group".into()),
            descendant_placement: Some(Sidebar),
        })
        .await
        .unwrap();
    assert_eq!(promoted.slug.as_deref(), Some("promoted-group"));
    assert_eq!(promoted.placement, Topbar);
    // A deferred invariant failure rolls back both reparenting and ordering.
    assert!(store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: Some(nested),
            rank: "009000".into(),
            before_id: None,
            after_id: None,
            placement: Topbar,
            slug: None,
            descendant_placement: Some(Sidebar),
        })
        .await
        .is_err());
    let unchanged = store
        .get_frontstage_page(workspace_id, source)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.parent_id, None);
    assert_eq!(unchanged.rank, promoted.rank);
    assert_eq!(unchanged.slug, promoted.slug);
}
