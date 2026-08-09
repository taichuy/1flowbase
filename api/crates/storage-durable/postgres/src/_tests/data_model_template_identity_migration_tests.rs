use std::borrow::Cow;

use serde_json::Value;
use sqlx::migrate::Migrator;
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260810100000;
const MIGRATION_SQL: &str =
    include_str!("../../migrations/20260810100000_add_data_model_template_identity.sql");

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

fn before_template_identity_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn model_metadata_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(
        r#"
        select coalesce(
            jsonb_agg(
                to_jsonb(model_row)
                  - 'template_provider'
                  - 'template_code'
                  - 'template_version'
                order by model_row.code
            ),
            '[]'::jsonb
        )
        from (select * from model_definitions) model_row
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn schema_snapshot(pool: &sqlx::PgPool) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        r#"
        select table_name, column_name, data_type, is_nullable
        from information_schema.columns
        where table_schema = current_schema()
          and not (
            table_name = 'model_definitions'
            and column_name in ('template_provider', 'template_code', 'template_version')
          )
        order by table_name, ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[test]
fn ac_001_template_identity_migration_sql_only_changes_model_metadata() {
    let normalized = MIGRATION_SQL.to_ascii_lowercase();
    assert!(normalized.contains("update model_definitions"));
    assert!(normalized.contains("template_provider = 'core'"));
    assert!(normalized.contains("template_code = 'general'"));
    assert!(normalized.contains("template_version = 'v1'"));
    assert!(!normalized.contains(" default "));
    for forbidden in ["model_fields", "runtime_records", "physical_table_name"] {
        assert!(
            !normalized.contains(forbidden),
            "migration must not touch {forbidden}"
        );
    }
    let backfill = normalized.find("update model_definitions").unwrap();
    let not_null = normalized.find("set not null").unwrap();
    assert!(backfill < not_null, "backfill must precede NOT NULL");
}

#[tokio::test]
async fn ac_001_historical_models_gain_only_core_general_v1_metadata() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_template_identity_migrator()
        .run(&pool)
        .await
        .unwrap();

    let main_model_id = Uuid::now_v7();
    let external_model_id = Uuid::now_v7();
    for (id, code, source_kind, physical_table_name) in [
        (
            main_model_id,
            "historical_main",
            "main_source",
            "historical_main_records",
        ),
        (
            external_model_id,
            "historical_external",
            "external_source",
            "historical_external_records",
        ),
    ] {
        sqlx::query(
            r#"
            insert into model_definitions (
                id, scope_kind, scope_id, source_kind, code, title,
                physical_table_name, acl_namespace, audit_namespace
            ) values ($1, 'workspace', $2, $3, $4, $4, $5, $6, $7)
            "#,
        )
        .bind(id)
        .bind(Uuid::now_v7())
        .bind(source_kind)
        .bind(code)
        .bind(physical_table_name)
        .bind(format!("state_model.{code}"))
        .bind(format!("audit.state_model.{code}"))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "create table historical_main_records (id uuid primary key, payload text not null)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let record_id = Uuid::now_v7();
    sqlx::query("insert into historical_main_records (id, payload) values ($1, 'preserved')")
        .bind(record_id)
        .execute(&pool)
        .await
        .unwrap();

    let metadata_before = model_metadata_snapshot(&pool).await;
    let schema_before = schema_snapshot(&pool).await;
    let record_before: (Uuid, String) =
        sqlx::query_as("select id, payload from historical_main_records")
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    assert_eq!(model_metadata_snapshot(&pool).await, metadata_before);
    assert_eq!(schema_snapshot(&pool).await, schema_before);
    let identities: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        select id, template_provider, template_code, template_version
        from model_definitions
        order by code
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        identities,
        vec![
            (
                external_model_id,
                "core".into(),
                "general".into(),
                "v1".into()
            ),
            (main_model_id, "core".into(), "general".into(), "v1".into()),
        ]
    );
    let identity_columns: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"
        select column_name, is_nullable, column_default
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'model_definitions'
          and column_name in ('template_provider', 'template_code', 'template_version')
        order by column_name
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(identity_columns.len(), 3);
    assert!(identity_columns
        .iter()
        .all(|(_, nullable, default)| nullable == "NO" && default.is_none()));
    assert_eq!(
        sqlx::query_as::<_, (Uuid, String)>("select id, payload from historical_main_records")
            .fetch_one(&pool)
            .await
            .unwrap(),
        record_before
    );
}
