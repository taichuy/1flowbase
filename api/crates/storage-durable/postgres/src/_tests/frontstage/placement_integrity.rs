use super::*;

#[tokio::test]
async fn placement_integrity_migration_rejects_dirty_history_before_installing_trigger() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_placement_integrity_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let (group_id, child_id) =
        insert_frontstage_group_and_page(&pool, workspace_id, "sidebar", "topbar").await;
    sqlx::query("update frontstage_pages set parent_id = $1 where id = $2")
        .bind(group_id)
        .bind(child_id)
        .execute(&pool)
        .await
        .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("frontstage placement integrity migration rejected dirty data"),
        "unexpected migration error: {error}"
    );
    let trigger_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_trigger where tgname = 'frontstage_pages_placement_integrity_trigger' and tgrelid = 'frontstage_pages'::regclass and not tgisinternal)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!trigger_exists);
}

#[tokio::test]
async fn placement_integrity_trigger_rejects_direct_sql_and_allows_cascade_delete() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let (group_id, child_id) =
        insert_frontstage_group_and_page(&pool, workspace_id, "sidebar", "sidebar").await;
    sqlx::query("update frontstage_pages set parent_id = $1 where id = $2")
        .bind(group_id)
        .bind(child_id)
        .execute(&pool)
        .await
        .unwrap();

    let child_update_error =
        sqlx::query("update frontstage_pages set placement = 'topbar' where id = $1")
            .bind(child_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        child_update_error
            .as_database_error()
            .and_then(|error| error.constraint()),
        Some("frontstage_pages_parent_child_placement")
    );

    sqlx::query(
        "update frontstage_pages set placement = 'topbar', slug = 'root-group' where id = $1",
    )
    .bind(group_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("delete from frontstage_pages where workspace_id = $1 and id = $2")
        .bind(workspace_id)
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_pages where workspace_id = $1 and id in ($2, $3)",
    )
    .bind(workspace_id)
    .bind(group_id)
    .bind(child_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn placement_integrity_serializes_parent_and_child_updates() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, slug, rank) values ($1, $2, 'group', 'Group', 'topbar', 'root-group', 'a')",
    )
        .bind(group_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut parent_tx = pool.begin().await.unwrap();
    sqlx::query("update frontstage_pages set placement = 'sidebar', slug = null where id = $1")
        .bind(group_id)
        .execute(&mut *parent_tx)
        .await
        .unwrap();

    let child_pool = pool.clone();
    let child_id = Uuid::now_v7();
    let child_insert = tokio::spawn(async move {
        sqlx::query(
            "insert into frontstage_pages (id, workspace_id, parent_id, kind, title, placement, rank) values ($1, $2, $3, 'page', 'Child', 'topbar', 'a')",
        )
            .bind(child_id)
            .bind(workspace_id)
            .bind(group_id)
            .execute(&child_pool)
            .await
            .unwrap_err()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!child_insert.is_finished());
    parent_tx.commit().await.unwrap();

    let error = child_insert.await.unwrap();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|database_error| database_error.constraint()),
        Some("frontstage_pages_parent_child_placement")
    );
}
