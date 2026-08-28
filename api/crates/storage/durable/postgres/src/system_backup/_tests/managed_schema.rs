use storage_durable_postgres::{managed_schema_backup_inventory, run_migrations};

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn pool() -> sqlx::PgPool {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn pdm_011_backup_inventory_contains_owned_objects_and_retained_data() {
    let pool = pool().await;
    sqlx::query("create table plugin_backup_fixture (id uuid primary key, value text)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into plugin_schema_ownership (
            ownership_key, owner_id, owner_version, object_kind, logical_name,
            physical_table, physical_column, field_type, nullable, active, plan_fingerprint
        ) values
            ('table:plugin_backup_fixture', 'fixture/backup', '1.0.0', 'owned_collection',
             'records', 'plugin_backup_fixture', null, null, null, false, 'fixture'),
            ('column:plugin_backup_fixture.value', 'fixture/backup', '1.0.0', 'owned_field',
             'records.value', 'plugin_backup_fixture', 'value', 'string', true, false, 'fixture')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let inventory = managed_schema_backup_inventory(&pool).await.unwrap();
    assert_eq!(inventory.len(), 2);
    assert!(inventory.iter().all(|object| !object.active));
}

#[tokio::test]
async fn pdm_011_backup_inventory_rejects_missing_physical_objects() {
    let pool = pool().await;
    sqlx::query(
        r#"
        insert into plugin_schema_ownership (
            ownership_key, owner_id, owner_version, object_kind, logical_name,
            physical_table, physical_column, field_type, nullable, active, plan_fingerprint
        ) values ('table:plugin_missing_fixture', 'fixture/backup', '1.0.0',
                  'owned_collection', 'missing', 'plugin_missing_fixture', null, null, null, true, 'fixture')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(managed_schema_backup_inventory(&pool).await.is_err());
}
