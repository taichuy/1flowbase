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

async fn dynamic_physical_schema_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(
        r#"
        select jsonb_build_object(
            'constraints', coalesce((
                select jsonb_agg(
                    jsonb_build_array(tables.relname, constraints.conname, pg_get_constraintdef(constraints.oid))
                    order by tables.relname, constraints.conname
                )
                from pg_constraint constraints
                join pg_class tables on tables.oid = constraints.conrelid
                join pg_namespace schemas on schemas.oid = tables.relnamespace
                where schemas.nspname = current_schema()
                  and tables.relname in ('historical_main_records', 'historical_external_records')
            ), '[]'::jsonb),
            'indexes', coalesce((
                select jsonb_agg(jsonb_build_array(tablename, indexname, indexdef) order by tablename, indexname)
                from pg_indexes
                where schemaname = current_schema()
                  and tablename in ('historical_main_records', 'historical_external_records')
            ), '[]'::jsonb)
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn field_metadata_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(
        r#"
        select coalesce(
            jsonb_agg(to_jsonb(field_row) order by field_row.data_model_id, field_row.code),
            '[]'::jsonb
        )
        from (select * from model_fields) field_row
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn grant_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(
        r#"
        select coalesce(
            jsonb_agg(to_jsonb(grant_row) order by grant_row.data_model_id),
            '[]'::jsonb
        )
        from (select * from scope_data_model_grants) grant_row
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn dynamic_record_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(
        r#"
        select jsonb_build_object(
            'main', (select jsonb_agg(to_jsonb(record_row) order by record_row.id) from historical_main_records record_row),
            'external', (select jsonb_agg(to_jsonb(record_row) order by record_row.id) from historical_external_records record_row)
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn has_legacy_api_exposure_column(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        r#"
        select exists (
            select 1
            from information_schema.columns
            where table_schema = current_schema()
              and table_name = 'model_definitions'
              and column_name = 'api_exposure_status'
        )
        "#,
    )
    .fetch_one(pool)
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
    let main_scope_id = Uuid::now_v7();
    let external_scope_id = Uuid::now_v7();
    for (
        id,
        scope_id,
        code,
        title,
        description,
        source_kind,
        external_resource_key,
        external_table_id,
        external_capability_snapshot,
        physical_table_name,
        availability_status,
        status,
    ) in [
        (
            main_model_id,
            main_scope_id,
            "historical_main",
            "User Main Title",
            "User-authored main description",
            "main_source",
            None,
            None,
            None,
            "historical_main_records",
            "unavailable",
            "disabled",
        ),
        (
            external_model_id,
            external_scope_id,
            "historical_external",
            "User External Title",
            "User-authored external description",
            "external_source",
            Some("contacts"),
            Some("crm.contacts"),
            Some(serde_json::json!({
                "supports_list": true,
                "supports_get": true,
                "supports_create": false,
                "supports_update": false,
                "supports_delete": false
            })),
            "historical_external_records",
            "broken",
            "broken",
        ),
    ] {
        sqlx::query(
            r#"
            insert into model_definitions (
                id, scope_kind, scope_id, source_kind,
                external_resource_key, external_table_id, external_capability_snapshot,
                code, title, description, physical_table_name,
                acl_namespace, audit_namespace, availability_status, status
            ) values (
                $1, 'workspace', $2, $3,
                $4, $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13, $14
            )
            "#,
        )
        .bind(id)
        .bind(scope_id)
        .bind(source_kind)
        .bind(external_resource_key)
        .bind(external_table_id)
        .bind(external_capability_snapshot)
        .bind(code)
        .bind(title)
        .bind(description)
        .bind(physical_table_name)
        .bind(format!("state_model.{code}"))
        .bind(format!("audit.state_model.{code}"))
        .bind(availability_status)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
    }

    for (model_id, scope_id, code, title, description, external_field_key, display_options) in [
        (
            main_model_id,
            main_scope_id,
            "payload",
            "Main Payload Label",
            "User main field description",
            None,
            serde_json::json!({ "rows": 7, "tone": "quiet" }),
        ),
        (
            external_model_id,
            external_scope_id,
            "payload",
            "External Payload Label",
            "User external field description",
            Some("remote_payload"),
            serde_json::json!({ "rows": 11, "tone": "loud" }),
        ),
    ] {
        sqlx::query(
            r#"
            insert into model_fields (
                id, scope_id, data_model_id, code, title, description,
                physical_column_name, external_field_key, field_kind,
                is_system, is_writable, is_required, api_required, is_unique,
                display_interface, display_options, relation_options,
                sort_order, availability_status
            ) values (
                $1, $2, $3, $4, $5, $6,
                'payload', $7, 'string',
                false, true, true, true, false,
                'textarea', $8, '{"preserve":"relation-display"}'::jsonb,
                37, 'unavailable'
            )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(scope_id)
        .bind(model_id)
        .bind(code)
        .bind(title)
        .bind(description)
        .bind(external_field_key)
        .bind(display_options)
        .execute(&pool)
        .await
        .unwrap();
    }

    for (model_id, scope_id, enabled, permission_profile) in [
        (main_model_id, main_scope_id, false, "owner"),
        (external_model_id, external_scope_id, false, "system_all"),
    ] {
        sqlx::query(
            r#"
            insert into scope_data_model_grants (
                id, scope_kind, scope_id, data_model_id, enabled, permission_profile
            ) values ($1, 'workspace', $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(scope_id)
        .bind(model_id)
        .bind(enabled)
        .bind(permission_profile)
        .execute(&pool)
        .await
        .unwrap();
    }

    for table_name in ["historical_main_records", "historical_external_records"] {
        sqlx::query(&format!(
            r#"
            create table {table_name} (
                id uuid primary key,
                scope_id uuid not null,
                created_by uuid,
                updated_by uuid,
                created_at timestamptz not null default now(),
                updated_at timestamptz not null default now(),
                payload text not null
            )
            "#
        ))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "insert into historical_main_records (id, scope_id, payload) values ($1, $2, 'main-preserved')",
    )
    .bind(Uuid::now_v7())
    .bind(main_scope_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into historical_external_records (id, scope_id, payload) values ($1, $2, 'external-preserved')",
    )
    .bind(Uuid::now_v7())
    .bind(external_scope_id)
    .execute(&pool)
    .await
    .unwrap();

    let metadata_before = model_metadata_snapshot(&pool).await;
    let fields_before = field_metadata_snapshot(&pool).await;
    let grants_before = grant_snapshot(&pool).await;
    let schema_before = schema_snapshot(&pool).await;
    let dynamic_physical_schema_before = dynamic_physical_schema_snapshot(&pool).await;
    let records_before = dynamic_record_snapshot(&pool).await;
    // API exposure is now represented by the current status/permission contract; the legacy
    // api_exposure_status column was intentionally removed before this migration.
    assert!(!has_legacy_api_exposure_column(&pool).await);

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    assert_eq!(model_metadata_snapshot(&pool).await, metadata_before);
    assert_eq!(field_metadata_snapshot(&pool).await, fields_before);
    assert_eq!(grant_snapshot(&pool).await, grants_before);
    assert_eq!(schema_snapshot(&pool).await, schema_before);
    assert_eq!(
        dynamic_physical_schema_snapshot(&pool).await,
        dynamic_physical_schema_before
    );
    assert_eq!(dynamic_record_snapshot(&pool).await, records_before);
    assert!(!has_legacy_api_exposure_column(&pool).await);
    let identities: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        select id, template_provider, template_code, template_version
        from model_definitions
        where id in ($1, $2)
        order by code
        "#,
    )
    .bind(main_model_id)
    .bind(external_model_id)
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
    let builtin_identities: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        select template_provider, template_code, template_version
        from model_definitions
        where id not in ($1, $2)
        order by code
        "#,
    )
    .bind(main_model_id)
    .bind(external_model_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(builtin_identities.len(), 8);
    assert!(builtin_identities
        .iter()
        .all(|(provider, code, version)| provider == "core"
            && code == "general"
            && version == "v1"));
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
}
