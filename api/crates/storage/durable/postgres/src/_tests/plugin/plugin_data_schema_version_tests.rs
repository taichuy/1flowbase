use super::*;
use extension_contracts::{PluginDataOrder, PluginDataOrderDirection};

const TABLE: &str = "plg_acme_session_versions";
fn schema(version: &str, fields: &[&str], retained: &[&str]) -> ManagedSchemaPlan {
    let mut operations = vec![ManagedSchemaOperation::EnsureOwnedCollection {
        logical_collection: "affinity".into(),
        physical_table: TABLE.into(),
    }];
    operations.extend(
        fields
            .iter()
            .map(|name| ManagedSchemaOperation::EnsureOwnedField {
                logical_collection: "affinity".into(),
                logical_field: (*name).into(),
                physical_table: TABLE.into(),
                physical_column: (*name).into(),
                field_type: ManagedSchemaFieldType::String,
                nullable: true,
            }),
    );
    operations.extend(
        retained
            .iter()
            .map(|name| ManagedSchemaOperation::RetainInactive {
                ownership_key: format!("column:{TABLE}.{name}"),
            }),
    );
    let mut value = plan(
        &format!("schema-{version}-{}", retained.join("-")),
        operations,
    );
    value.owner_version = version.into();
    value
}
fn one(operation: PluginDataOperation) -> PluginDataRequest {
    PluginDataRequest {
        idempotency_key: None,
        operations: vec![operation],
    }
}
fn find(fields: &[&str]) -> PluginDataOperation {
    PluginDataOperation::Find {
        target: owned_target(),
        fields: fields.iter().map(|s| (*s).into()).collect(),
        filters: vec![],
        order: vec![],
        page: PluginDataPage::default(),
    }
}
fn insert(field: &str, value: &str) -> PluginDataOperation {
    PluginDataOperation::Insert {
        target: owned_target(),
        values: [(field.into(), PluginDataValue::String(value.into()))]
            .into_iter()
            .collect(),
    }
}

#[tokio::test]
async fn schema_versions_preserve_old_reads_writes_and_isolate_added_removed_fields() {
    let (store, _) = store().await;
    store
        .apply_managed_schema(&schema("1.0.0", &["shared", "legacy"], &[]))
        .await
        .unwrap();
    let old = binding(Uuid::now_v7());
    store
        .execute(&old, &one(insert("legacy", "before")))
        .await
        .unwrap();
    // Merely applying the new package schema must not revoke the old version's projection.
    store
        .apply_managed_schema(&schema("2.0.0", &["shared", "new_field"], &["legacy"]))
        .await
        .unwrap();
    let mut new = old.clone();
    new.plugin_version = "2.0.0".into();
    store
        .execute(&old, &one(insert("legacy", "after")))
        .await
        .unwrap();
    let rows = store.execute(&old, &one(find(&["legacy"]))).await.unwrap();
    let PluginDataOperationResult::Rows { rows } = &rows.results[0] else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    store
        .execute(&new, &one(insert("new_field", "v2")))
        .await
        .unwrap();
    store
        .execute(&new, &one(find(&["shared", "new_field"])))
        .await
        .unwrap();

    // Selection, filters, sorting and writes must all use the caller version's field set.
    for (caller, forbidden) in [(&old, "new_field"), (&new, "legacy")] {
        for operation in [
            find(&[forbidden]),
            PluginDataOperation::Count {
                target: owned_target(),
                filters: vec![eq(forbidden, PluginDataValue::String("escape".into()))],
            },
            PluginDataOperation::Find {
                target: owned_target(),
                fields: vec!["id".into()],
                filters: vec![],
                order: vec![PluginDataOrder {
                    field: forbidden.into(),
                    direction: PluginDataOrderDirection::Ascending,
                }],
                page: PluginDataPage::default(),
            },
            PluginDataOperation::Update {
                target: owned_target(),
                filters: vec![eq("id", PluginDataValue::Uuid(Uuid::now_v7().to_string()))],
                values: [(forbidden.into(), PluginDataValue::String("escape".into()))]
                    .into_iter()
                    .collect(),
            },
        ] {
            assert_eq!(
                store
                    .execute(caller, &one(operation))
                    .await
                    .unwrap_err()
                    .kind,
                PluginDataErrorKind::OwnershipDenied
            );
        }
    }
    let all = store
        .execute(&old, &one(find(&["id", "shared", "legacy"])))
        .await
        .unwrap();
    let PluginDataOperationResult::Rows { rows } = &all.results[0] else {
        panic!()
    };
    assert!(rows.iter().all(|r| !r.values.contains_key("new_field")));
    // Reapplying V1 during rollback leaves both immutable declarations intact.
    store
        .apply_managed_schema(&schema("1.0.0", &["shared", "legacy"], &["new_field"]))
        .await
        .unwrap();
    store.execute(&old, &one(find(&["legacy"]))).await.unwrap();
    store
        .execute(&new, &one(find(&["new_field"])))
        .await
        .unwrap();
    let mut unknown = old.clone();
    unknown.plugin_version = "9.0.0".into();
    assert_eq!(
        store
            .execute(&unknown, &one(find(&["id"])))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::OwnershipDenied
    );
    let mut foreign = old.clone();
    foreign.publisher_namespace = "foreign".into();
    assert_eq!(
        store
            .execute(&foreign, &one(find(&["id"])))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::OwnershipDenied
    );
    let mut retained = schema("1.0.0", &[], &["shared", "legacy", "new_field"]);
    retained.operations.remove(0);
    retained
        .operations
        .push(ManagedSchemaOperation::RetainInactive {
            ownership_key: format!("table:{TABLE}"),
        });
    retained.fingerprint = "uninstalled".into();
    store.apply_managed_schema(&retained).await.unwrap();
    for caller in [&old, &new] {
        assert_eq!(
            store
                .execute(caller, &one(find(&["id"])))
                .await
                .unwrap_err()
                .kind,
            PluginDataErrorKind::OwnershipDenied
        );
    }
}
