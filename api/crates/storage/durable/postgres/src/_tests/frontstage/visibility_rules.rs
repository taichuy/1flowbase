use super::*;

#[tokio::test]
async fn migration_creates_frontstage_page_visibility_rules() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let table_exists: bool = sqlx::query_scalar(
        r#"
        select exists(
            select 1
            from information_schema.tables
            where table_schema = $1
              and table_name = 'frontstage_page_visibility_rules'
        )
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(table_exists);

    let visibility_check_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'frontstage_page_visibility_rules'
          and c.contype = 'c'
          and pg_get_constraintdef(c.oid) ilike '%visible%'
          and pg_get_constraintdef(c.oid) ilike '%hidden%'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(visibility_check_count, 1);

    let foreign_key_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'frontstage_page_visibility_rules'
          and c.contype = 'f'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(foreign_key_count, 4);

    let unique_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and tablename = 'frontstage_page_visibility_rules'
          and indexname in (
              'frontstage_page_visibility_rules_root_uidx',
              'frontstage_page_visibility_rules_page_uidx'
          )
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(unique_indexes.len(), 2);

    let root_index_definition: String = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and indexname = 'frontstage_page_visibility_rules_root_uidx'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(root_index_definition.contains("page_id IS NULL"));
    assert!(root_index_definition.contains("tab_id IS NULL"));
}

#[test]
fn migration_enforces_frontstage_root_slug_contract() {
    let migration =
        include_str!("../../../migrations/20260711113000_enforce_frontstage_root_slug.sql");
    assert!(migration.contains("frontstage_pages_workspace_slug_uidx"));
    assert!(migration.contains("frontstage_pages_root_slug_check"));
    assert!(migration.contains("parent_placement = 'topbar'"));
    assert!(migration.contains("new.placement = 'sidebar'"));
}
