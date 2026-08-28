use std::collections::{BTreeMap, BTreeSet};

use control_plane_contracts::ports::{
    ManagedSchemaFieldType, ManagedSchemaOperation, ManagedSchemaPlan, ManagedSchemaRepository,
};
use extension_contracts::{
    PluginDataBinding, PluginDataErrorKind, PluginDataFilter, PluginDataFilterOperator,
    PluginDataOperation, PluginDataOperationResult, PluginDataPage, PluginDataPermission,
    PluginDataPort, PluginDataRequest, PluginDataTarget, PluginDataValue,
};
use sqlx::PgPool;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
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

fn binding(workspace_id: Uuid) -> PluginDataBinding {
    PluginDataBinding {
        publisher_namespace: "acme".to_string(),
        plugin_code: "session".to_string(),
        plugin_version: "1.0.0".to_string(),
        storage_binding: "main".to_string(),
        workspace_id: workspace_id.to_string(),
        actor_id: Some(Uuid::now_v7().to_string()),
        provider_instance_id: "provider-1".to_string(),
        permissions: BTreeSet::from([PluginDataPermission::Read, PluginDataPermission::Write]),
        deadline_unix_ms: i64::try_from(
            (OffsetDateTime::now_utc() + time::Duration::minutes(1)).unix_timestamp_nanos()
                / 1_000_000,
        )
        .unwrap(),
    }
}

fn plan(fingerprint: &str, operations: Vec<ManagedSchemaOperation>) -> ManagedSchemaPlan {
    ManagedSchemaPlan {
        owner_id: "acme/session".to_string(),
        owner_version: "1.0.0".to_string(),
        fingerprint: fingerprint.to_string(),
        max_target_table_bytes: u64::MAX,
        lock_timeout_ms: 1_000,
        operations,
    }
}

fn owned_target() -> PluginDataTarget {
    PluginDataTarget::OwnedCollection {
        collection_code: "affinity".to_string(),
    }
}

fn eq(field: &str, value: PluginDataValue) -> PluginDataFilter {
    PluginDataFilter {
        field: field.to_string(),
        operator: PluginDataFilterOperator::Equal,
        value: Some(value),
    }
}

#[tokio::test]
async fn pdp_004_007_owned_data_is_scoped_atomic_and_idempotent() {
    let (store, pool) = store().await;
    store
        .apply_managed_schema(&plan(
            "owned-v1",
            vec![
                ManagedSchemaOperation::EnsureOwnedCollection {
                    logical_collection: "affinity".to_string(),
                    physical_table: "plg_acme_session_affinity".to_string(),
                },
                ManagedSchemaOperation::EnsureOwnedField {
                    logical_collection: "affinity".to_string(),
                    logical_field: "conversation".to_string(),
                    physical_table: "plg_acme_session_affinity".to_string(),
                    physical_column: "conversation".to_string(),
                    field_type: ManagedSchemaFieldType::String,
                    nullable: false,
                },
                ManagedSchemaOperation::EnsureOwnedField {
                    logical_collection: "affinity".to_string(),
                    logical_field: "provider".to_string(),
                    physical_table: "plg_acme_session_affinity".to_string(),
                    physical_column: "provider".to_string(),
                    field_type: ManagedSchemaFieldType::String,
                    nullable: false,
                },
            ],
        ))
        .await
        .unwrap();
    let workspace = Uuid::now_v7();
    let first = PluginDataRequest {
        idempotency_key: Some("upsert-1".to_string()),
        operations: vec![PluginDataOperation::Upsert {
            target: owned_target(),
            identity: BTreeMap::from([(
                "conversation".to_string(),
                PluginDataValue::String("conversation-1".to_string()),
            )]),
            values: BTreeMap::from([(
                "provider".to_string(),
                PluginDataValue::String("provider-a".to_string()),
            )]),
        }],
    };
    let applied = store.execute(&binding(workspace), &first).await.unwrap();
    assert!(!applied.replayed);
    let replay = store.execute(&binding(workspace), &first).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.results, applied.results);

    let changed = PluginDataRequest {
        idempotency_key: Some("upsert-1".to_string()),
        operations: vec![PluginDataOperation::Count {
            target: owned_target(),
            filters: vec![],
        }],
    };
    assert_eq!(
        store
            .execute(&binding(workspace), &changed)
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::Conflict
    );

    let other_workspace = Uuid::now_v7();
    let count = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Count {
            target: owned_target(),
            filters: vec![],
        }],
    };
    assert_eq!(
        store
            .execute(&binding(other_workspace), &count)
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Count { count: 0 }]
    );

    let atomic_failure = PluginDataRequest {
        idempotency_key: None,
        operations: vec![
            PluginDataOperation::Insert {
                target: owned_target(),
                values: BTreeMap::from([
                    (
                        "conversation".to_string(),
                        PluginDataValue::String("rolled-back".to_string()),
                    ),
                    (
                        "provider".to_string(),
                        PluginDataValue::String("provider-b".to_string()),
                    ),
                ]),
            },
            PluginDataOperation::Update {
                target: owned_target(),
                filters: vec![],
                values: BTreeMap::from([(
                    "provider".to_string(),
                    PluginDataValue::String("never".to_string()),
                )]),
            },
        ],
    };
    assert!(store
        .execute(&binding(workspace), &atomic_failure)
        .await
        .is_err());
    let rolled_back: i64 = sqlx::query_scalar(
        "select count(*) from plg_acme_session_affinity where conversation = 'rolled-back'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rolled_back, 0);
}

#[tokio::test]
async fn pdp_004_005_extension_projection_exposes_only_identity_and_owned_fields() {
    let (store, pool) = store().await;
    sqlx::query(
        "create table fixture_projection (id uuid primary key, scope_id uuid not null, core_value text not null)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"insert into model_definitions (
            id, scope_kind, scope_id, code, title, physical_table_name,
            acl_namespace, audit_namespace, template_provider, template_code, template_version
        ) values ($1, 'system', $2, 'fixture_projection', 'Fixture projection',
            'fixture_projection', 'fixture.projection', 'fixture.projection', 'core',
            'fixture_projection', '1.0.0')"#,
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::nil())
    .execute(&pool)
    .await
    .unwrap();
    store
        .apply_managed_schema(&plan(
            "extension-v1",
            vec![ManagedSchemaOperation::EnsureExtensionField {
                target_table: "fixture_projection".to_string(),
                logical_field: "score".to_string(),
                physical_column: "ext_acme_session_score".to_string(),
                field_type: ManagedSchemaFieldType::Number,
            }],
        ))
        .await
        .unwrap();
    let workspace = Uuid::now_v7();
    let id = Uuid::now_v7();
    sqlx::query(
        "insert into fixture_projection (id, scope_id, core_value) values ($1, $2, 'secret')",
    )
    .bind(id)
    .bind(workspace)
    .execute(&pool)
    .await
    .unwrap();
    let target = PluginDataTarget::ExtensionProjection {
        target_table: "fixture_projection".to_string(),
    };
    let update = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Update {
            target: target.clone(),
            filters: vec![eq("id", PluginDataValue::Uuid(id.to_string()))],
            values: BTreeMap::from([(
                "score".to_string(),
                PluginDataValue::Number("8.5".to_string()),
            )]),
        }],
    };
    store.execute(&binding(workspace), &update).await.unwrap();
    let read = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Find {
            target: target.clone(),
            fields: vec!["id".to_string(), "score".to_string()],
            filters: vec![eq("id", PluginDataValue::Uuid(id.to_string()))],
            order: vec![],
            page: PluginDataPage::default(),
        }],
    };
    let result = store.execute(&binding(workspace), &read).await.unwrap();
    let PluginDataOperationResult::Rows { rows } = &result.results[0] else {
        panic!()
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values.get("score"),
        Some(&PluginDataValue::Number("8.5".to_string()))
    );

    let core_escape = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Find {
            target,
            fields: vec!["core_value".to_string()],
            filters: vec![],
            order: vec![],
            page: PluginDataPage::default(),
        }],
    };
    assert_eq!(
        store
            .execute(&binding(workspace), &core_escape)
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::OwnershipDenied
    );
}
