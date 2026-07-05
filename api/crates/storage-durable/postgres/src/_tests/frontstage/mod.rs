use sqlx::PgPool;
use storage_postgres::{connect, run_migrations};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

#[tokio::test]
async fn migration_creates_frontstage_page_visibility_rules() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    assert_eq!(foreign_key_count, 3);

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
}
