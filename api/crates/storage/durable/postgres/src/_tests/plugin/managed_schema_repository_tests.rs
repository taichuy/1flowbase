use control_plane_contracts::ports::{
    ManagedSchemaFieldType, ManagedSchemaOperation, ManagedSchemaPlan, ManagedSchemaPreviewAction,
    ManagedSchemaRepository,
};
use sqlx::PgPool;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> (PgControlPlaneStore, PgPool) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    (PgControlPlaneStore::new(pool.clone()), pool)
}

async fn register_business_table(pool: &PgPool, table: &str) {
    sqlx::query(&format!(
        "create table \"{table}\" (id uuid primary key, value text)"
    ))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into model_definitions (
            id, scope_kind, scope_id, code, title, physical_table_name,
            acl_namespace, audit_namespace, template_provider, template_code, template_version
        ) values ($1, 'system', $2, $3, $4, $5, $6, $7, 'core', $3, '1.0.0')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::nil())
    .bind(format!("fixture_{table}"))
    .bind(format!("Fixture {table}"))
    .bind(table)
    .bind(format!("fixture.{table}"))
    .bind(format!("fixture.{table}"))
    .execute(pool)
    .await
    .unwrap();
}

fn plan(
    owner_id: &str,
    fingerprint: &str,
    max_target_table_bytes: u64,
    operations: Vec<ManagedSchemaOperation>,
) -> ManagedSchemaPlan {
    ManagedSchemaPlan {
        owner_id: owner_id.to_string(),
        owner_version: "1.0.0".to_string(),
        fingerprint: fingerprint.to_string(),
        max_target_table_bytes,
        lock_timeout_ms: 1_000,
        operations,
    }
}

#[tokio::test]
async fn pdm_003_005_owned_and_extension_objects_reconcile_idempotently() {
    let (store, pool) = store().await;
    register_business_table(&pool, "fixture_business_records").await;
    let desired = plan(
        "acme.analytics",
        "schema-v1",
        u64::MAX,
        vec![
            ManagedSchemaOperation::EnsureOwnedCollection {
                logical_collection: "notes".to_string(),
                physical_table: "plugin_acme_notes".to_string(),
            },
            ManagedSchemaOperation::EnsureOwnedField {
                logical_collection: "notes".to_string(),
                logical_field: "title".to_string(),
                physical_table: "plugin_acme_notes".to_string(),
                physical_column: "title".to_string(),
                field_type: ManagedSchemaFieldType::String,
                nullable: false,
            },
            ManagedSchemaOperation::EnsureExtensionField {
                target_table: "fixture_business_records".to_string(),
                logical_field: "priority".to_string(),
                physical_column: "x_acme_priority".to_string(),
                field_type: ManagedSchemaFieldType::Number,
            },
        ],
    );

    let preview = store.preview_managed_schema(&desired).await.unwrap();
    assert!(preview
        .entries
        .iter()
        .all(|entry| entry.action == ManagedSchemaPreviewAction::Create));

    let first = store.apply_managed_schema(&desired).await.unwrap();
    assert_eq!(first.created_objects, 3);
    let replay = store.apply_managed_schema(&desired).await.unwrap();
    assert_eq!(replay.receipt_id, first.receipt_id);

    let existing = store.preview_managed_schema(&desired).await.unwrap();
    assert!(existing
        .entries
        .iter()
        .all(|entry| entry.action == ManagedSchemaPreviewAction::AlreadyPresent));
    let ownership = store.list_managed_schema_ownership().await.unwrap();
    assert_eq!(ownership.len(), 3);
    assert!(ownership.iter().all(|record| record.active));
    assert!(ownership
        .iter()
        .all(|record| record.owner_id == "acme.analytics"));

    let retained = plan(
        "acme.analytics",
        "schema-v2",
        u64::MAX,
        vec![ManagedSchemaOperation::RetainInactive {
            ownership_key: "column:plugin_acme_notes.title".to_string(),
        }],
    );
    let receipt = store.apply_managed_schema(&retained).await.unwrap();
    assert_eq!(receipt.retained_objects, 1);
    let retained_record = store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .into_iter()
        .find(|record| record.ownership_key == "column:plugin_acme_notes.title")
        .unwrap();
    assert!(!retained_record.active);
    let column_still_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'plugin_acme_notes' and column_name = 'title')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(column_still_exists);
}

#[tokio::test]
async fn pdm_006_008_owner_and_column_contract_drift_fail_closed() {
    let (store, pool) = store().await;
    let owner_plan = plan(
        "owner.one",
        "owner-one-v1",
        u64::MAX,
        vec![ManagedSchemaOperation::EnsureOwnedCollection {
            logical_collection: "records".to_string(),
            physical_table: "plugin_owner_records".to_string(),
        }],
    );
    store.apply_managed_schema(&owner_plan).await.unwrap();
    let conflicting_owner = plan(
        "owner.two",
        "owner-two-v1",
        u64::MAX,
        vec![ManagedSchemaOperation::EnsureOwnedCollection {
            logical_collection: "records".to_string(),
            physical_table: "plugin_owner_records".to_string(),
        }],
    );
    let owner_error = store
        .apply_managed_schema(&conflicting_owner)
        .await
        .unwrap_err();
    assert!(owner_error.to_string().contains("another owner"));

    register_business_table(&pool, "fixture_drift_records").await;
    sqlx::query("alter table fixture_drift_records add column x_acme_score text not null")
        .execute(&pool)
        .await
        .unwrap();
    let drifted = plan(
        "acme.analytics",
        "drift-v1",
        u64::MAX,
        vec![ManagedSchemaOperation::EnsureExtensionField {
            target_table: "fixture_drift_records".to_string(),
            logical_field: "score".to_string(),
            physical_column: "x_acme_score".to_string(),
            field_type: ManagedSchemaFieldType::Number,
        }],
    );
    let drift_error = store.preview_managed_schema(&drifted).await.unwrap_err();
    assert!(drift_error
        .to_string()
        .contains("managed schema drift at fixture_drift_records.x_acme_score"));
    assert!(store.list_managed_schema_ownership().await.unwrap().len() == 1);
}

#[tokio::test]
async fn pdm_010_capacity_failure_rolls_back_the_entire_schema_plan() {
    let (store, pool) = store().await;
    register_business_table(&pool, "fixture_capacity_records").await;
    let oversized = plan(
        "acme.rollback",
        "rollback-v1",
        1,
        vec![
            ManagedSchemaOperation::EnsureOwnedCollection {
                logical_collection: "temporary".to_string(),
                physical_table: "plugin_rollback_temporary".to_string(),
            },
            ManagedSchemaOperation::EnsureExtensionField {
                target_table: "fixture_capacity_records".to_string(),
                logical_field: "marker".to_string(),
                physical_column: "x_rollback_marker".to_string(),
                field_type: ManagedSchemaFieldType::Boolean,
            },
        ],
    );

    let error = store.apply_managed_schema(&oversized).await.unwrap_err();
    assert!(error.to_string().contains("capacity preflight"));
    let table: Option<String> =
        sqlx::query_scalar("select to_regclass('plugin_rollback_temporary')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(table.is_none());
    assert!(store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .is_empty());
}
