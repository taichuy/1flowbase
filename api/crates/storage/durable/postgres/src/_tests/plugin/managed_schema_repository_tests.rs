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
    assert!(ownership.iter().any(|record| {
        record.object_kind.as_str() == "owned_field" && record.logical_name == "notes.title"
    }));

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

fn versioned_owned_plan(version: &str, fields: &[(&str, bool)]) -> ManagedSchemaPlan {
    let mut operations = vec![ManagedSchemaOperation::EnsureOwnedCollection {
        logical_collection: "records".into(),
        physical_table: "plugin_versioned_records".into(),
    }];
    operations.extend(fields.iter().map(|(field, nullable)| {
        ManagedSchemaOperation::EnsureOwnedField {
            logical_collection: "records".into(),
            logical_field: (*field).into(),
            physical_table: "plugin_versioned_records".into(),
            physical_column: (*field).into(),
            field_type: ManagedSchemaFieldType::String,
            nullable: *nullable,
        }
    }));
    let mut plan = plan(
        "acme/versioned",
        &format!("version-{version}"),
        u64::MAX,
        operations,
    );
    plan.owner_version = version.into();
    plan
}

async fn version_members(pool: &PgPool, version: &str) -> Vec<String> {
    sqlx::query_scalar("select ownership_key from plugin_schema_version_objects where owner_id='acme/versioned' and owner_version=$1 order by ownership_key")
        .bind(version).fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn managed_schema_versions_keep_exact_membership_through_upgrade_and_retirement() {
    let (store, pool) = store().await;
    let v1 = versioned_owned_plan("1.0.0", &[("title", false), ("legacy", true)]);
    store.apply_managed_schema(&v1).await.unwrap();
    let expected_v1 = vec![
        "column:plugin_versioned_records.legacy".to_owned(),
        "column:plugin_versioned_records.title".to_owned(),
        "table:plugin_versioned_records".to_owned(),
    ];
    assert_eq!(version_members(&pool, "1.0.0").await, expected_v1);
    let mut v2 = versioned_owned_plan("2.0.0", &[("title", false), ("extra", true)]);
    v2.operations.push(ManagedSchemaOperation::RetainInactive {
        ownership_key: "column:plugin_versioned_records.legacy".into(),
    });
    store.apply_managed_schema(&v2).await.unwrap();
    let expected_v2 = vec![
        "column:plugin_versioned_records.extra".to_owned(),
        "column:plugin_versioned_records.title".to_owned(),
        "table:plugin_versioned_records".to_owned(),
    ];
    assert_eq!(version_members(&pool, "1.0.0").await, expected_v1);
    assert_eq!(version_members(&pool, "2.0.0").await, expected_v2);
    assert!(
        !store
            .list_managed_schema_ownership()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.logical_name == "records.legacy")
            .unwrap()
            .active
    );
    // Removing or adding an ensured member under the same immutable version is rejected.
    for fields in [
        vec![("title", false)],
        vec![("title", false), ("extra", true), ("injected", true)],
    ] {
        let mut changed = versioned_owned_plan("2.0.0", &fields);
        changed.fingerprint = format!("changed-{}", fields.len());
        let error = store.apply_managed_schema(&changed).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("version object membership is immutable"));
    }
    let injected: bool = sqlx::query_scalar("select exists(select 1 from information_schema.columns where table_schema=current_schema() and table_name='plugin_versioned_records' and column_name='injected')")
        .fetch_one(&pool).await.unwrap();
    assert!(!injected);
    let mut retired = v2.clone();
    retired.fingerprint = "retired-family".into();
    retired.operations = store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .into_iter()
        .map(|row| ManagedSchemaOperation::RetainInactive {
            ownership_key: row.ownership_key,
        })
        .collect();
    store.apply_managed_schema(&retired).await.unwrap();
    assert!(store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .iter()
        .all(|row| !row.active));
    assert_eq!(version_members(&pool, "1.0.0").await, expected_v1);
    assert_eq!(version_members(&pool, "2.0.0").await, expected_v2);
    store.apply_managed_schema(&v1).await.unwrap();
    store.apply_managed_schema(&v2).await.unwrap();
    assert_eq!(version_members(&pool, "1.0.0").await, expected_v1);
    assert_eq!(version_members(&pool, "2.0.0").await, expected_v2);
}

#[tokio::test]
async fn managed_schema_rejects_required_addition_that_breaks_old_version_inserts() {
    let (store, pool) = store().await;
    let v1 = versioned_owned_plan("1.0.0", &[("title", false)]);
    // A first version can declare required fields before an older insert contract exists.
    store.apply_managed_schema(&v1).await.unwrap();
    let members = version_members(&pool, "1.0.0").await;
    let mut incompatible =
        versioned_owned_plan("2.0.0", &[("title", false), ("required_new", false)]);
    incompatible.operations.insert(
        0,
        ManagedSchemaOperation::EnsureOwnedCollection {
            logical_collection: "candidate_only".into(),
            physical_table: "plugin_candidate_only".into(),
        },
    );
    let error = store.apply_managed_schema(&incompatible).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("required field would break retained version inserts"));
    assert_eq!(version_members(&pool, "1.0.0").await, members);
    assert!(version_members(&pool, "2.0.0").await.is_empty());
    assert!(store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .iter()
        .all(|row| row.owner_version == "1.0.0" && row.active));
    let created: bool = sqlx::query_scalar("select exists(select 1 from information_schema.tables where table_schema=current_schema() and table_name='plugin_candidate_only') or exists(select 1 from information_schema.columns where table_schema=current_schema() and table_name='plugin_versioned_records' and column_name='required_new')")
        .fetch_one(&pool).await.unwrap();
    assert!(!created);
    let receipts: i64 = sqlx::query_scalar("select count(*) from plugin_schema_reconcile_receipts where owner_id='acme/versioned' and owner_version='2.0.0'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(receipts, 0);
    // A nullable addition remains compatible, and the old version can still insert.
    let compatible = versioned_owned_plan("2.0.0", &[("title", false), ("optional_new", true)]);
    store.apply_managed_schema(&compatible).await.unwrap();
    sqlx::query(
        "insert into plugin_versioned_records(id,scope_id,title) values($1,$2,'old version write')",
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn managed_schema_required_field_removal_rejects_only_still_declared_collection() {
    let (store, pool) = store().await;
    let v1 = versioned_owned_plan("1.0.0", &[("title", false), ("optional", true)]);
    store.apply_managed_schema(&v1).await.unwrap();
    let v1_members = version_members(&pool, "1.0.0").await;
    let mut removal = versioned_owned_plan("2.0.0", &[("optional", true)]);
    removal
        .operations
        .push(ManagedSchemaOperation::RetainInactive {
            ownership_key: "column:plugin_versioned_records.title".into(),
        });
    let error = store.apply_managed_schema(&removal).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("required field removal would break candidate inserts"));
    assert!(version_members(&pool, "2.0.0").await.is_empty());
    assert!(store
        .list_managed_schema_ownership()
        .await
        .unwrap()
        .iter()
        .all(|row| row.active));
    // A later version can stop exposing this entire collection while using another one.
    let mut no_old_collection = removal;
    no_old_collection.fingerprint = "version-2-without-old-collection".into();
    no_old_collection.operations = vec![ManagedSchemaOperation::EnsureOwnedCollection {
        logical_collection: "replacement".into(),
        physical_table: "plugin_replacement_records".into(),
    }];
    no_old_collection
        .operations
        .extend(
            v1_members
                .iter()
                .map(|key| ManagedSchemaOperation::RetainInactive {
                    ownership_key: key.clone(),
                }),
        );
    store
        .apply_managed_schema(&no_old_collection)
        .await
        .unwrap();
    assert_eq!(version_members(&pool, "1.0.0").await, v1_members);
    assert_eq!(
        version_members(&pool, "2.0.0").await,
        vec!["table:plugin_replacement_records".to_owned()]
    );
}
