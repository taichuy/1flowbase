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
        assert!(
            store
                .move_frontstage_page(&MoveFrontstagePageInput {
                    workspace_id,
                    actor_user_id: Uuid::nil(),
                    page_id: ids[0],
                    parent_id: None,
                    rank: String::new(),
                    before_id: None,
                    after_id: Some(target),
                })
                .await
                .is_err()
        );
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
    assert!(
        store
            .move_frontstage_page(&MoveFrontstagePageInput {
                workspace_id,
                actor_user_id: Uuid::nil(),
                page_id: source,
                parent_id: Some(source_group),
                rank: String::new(),
                before_id: None,
                after_id: Some(first),
            })
            .await
            .is_err()
    );
    let unchanged = store
        .get_frontstage_page(workspace_id, source)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.parent_id, Some(target_group));
    assert_eq!(unchanged.rank, original_rank);
}
