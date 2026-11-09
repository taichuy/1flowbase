use super::*;

#[tokio::test]
async fn record_view_filters_find_count_and_pagination_in_physical_system_scope() {
    let f = fixture().await;
    for id in ["a-denied", "b-granted", "c-inherited"] {
        seed(&f, id).await;
    }
    policy(&f, "a-denied", None, &[]).await;
    policy(&f, "b-granted", Some(f.role), &["view"]).await;
    assert_eq!(count_with(&f, &f.binding).await, 2);
    for (offset, expected) in [(0, "b-granted"), (1, "c-inherited")] {
        let find = PluginDataOperation::Find {
            target: owned_target(),
            fields: vec!["conversation".into()],
            filters: vec![],
            order: vec![extension_contracts::PluginDataOrder {
                field: "conversation".into(),
                direction: extension_contracts::PluginDataOrderDirection::Ascending,
            }],
            page: PluginDataPage { limit: 1, offset },
        };
        let result = f.store.execute(&f.binding, &request(find)).await.unwrap();
        let PluginDataOperationResult::Rows { rows } = &result.results[0] else {
            panic!("rows")
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].values["conversation"],
            PluginDataValue::String(expected.into())
        );
    }
    let direct = request(PluginDataOperation::FindOne {
        target: owned_target(),
        fields: vec!["conversation".into()],
        filters: vec![eq(
            "conversation",
            PluginDataValue::String("a-denied".into()),
        )],
    });
    assert_eq!(
        f.store.execute(&f.binding, &direct).await.unwrap().results,
        vec![PluginDataOperationResult::OptionalRow { row: None }]
    );
    let mut root = f.binding.clone();
    root.record_permissions.as_mut().unwrap().is_root = true;
    assert_eq!(count_with(&f, &root).await, 3);
    let mut other = f.binding.clone();
    other
        .record_permissions
        .as_mut()
        .unwrap()
        .caller_workspace_id = f.other_workspace.to_string();
    assert_eq!(
        count_with(&f, &other).await,
        3,
        "caller policy workspace must be independent of physical system scope"
    );
    assert_eq!(f.binding.workspace_id, domain::SYSTEM_SCOPE_ID.to_string());
}

#[tokio::test]
async fn protected_mutations_and_upsert_cannot_skip_row_policy_or_change_stable_identity() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    policy(&f, "host-a", Some(f.role), &["view"]).await;
    let update = PluginDataOperation::Update {
        target: owned_target(),
        filters: vec![eq("conversation", PluginDataValue::String("host-a".into()))],
        values: BTreeMap::from([("provider".into(), PluginDataValue::String("changed".into()))]),
    };
    assert_eq!(
        f.store
            .execute(
                &action(&f, "update", Some("host-a")),
                &request(update.clone())
            )
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Mutation { affected: 0 }]
    );
    let upsert = PluginDataOperation::Upsert {
        target: owned_target(),
        identity: BTreeMap::from([(
            "conversation".into(),
            PluginDataValue::String("host-a".into()),
        )]),
        values: values("host-a", "changed"),
    };
    assert_eq!(
        f.store
            .execute(
                &action(&f, "update", Some("host-a")),
                &request(upsert.clone())
            )
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::PermissionDenied
    );
    let delete = PluginDataOperation::Delete {
        target: owned_target(),
        filters: vec![eq("conversation", PluginDataValue::String("host-a".into()))],
    };
    assert_eq!(
        f.store
            .execute(
                &action(&f, "delete", Some("host-a")),
                &request(delete.clone())
            )
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Mutation { affected: 0 }]
    );
    policy(&f, "host-a", Some(f.role), &["update"]).await;
    assert_eq!(
        f.store
            .execute(&action(&f, "update", Some("host-a")), &request(upsert))
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Mutation { affected: 1 }]
    );
    let rename = PluginDataOperation::Update {
        target: owned_target(),
        filters: vec![eq("conversation", PluginDataValue::String("host-a".into()))],
        values: BTreeMap::from([(
            "conversation".into(),
            PluginDataValue::String("other-host".into()),
        )]),
    };
    assert_eq!(
        f.store
            .execute(&action(&f, "update", Some("host-a")), &request(rename))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::PermissionDenied
    );
    seed(&f, "other-host").await;
    let cross_record = PluginDataOperation::Update {
        target: owned_target(),
        filters: vec![eq(
            "conversation",
            PluginDataValue::String("other-host".into()),
        )],
        values: BTreeMap::from([("provider".into(), PluginDataValue::String("escaped".into()))]),
    };
    assert_eq!(
        f.store
            .execute(
                &action(&f, "update", Some("host-a")),
                &request(cross_record)
            )
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Mutation { affected: 0 }]
    );
    policy(&f, "host-a", Some(f.role), &["delete"]).await;
    assert_eq!(
        f.store
            .execute(&action(&f, "delete", Some("host-a")), &request(delete))
            .await
            .unwrap()
            .results,
        vec![PluginDataOperationResult::Mutation { affected: 1 }]
    );
}

#[tokio::test]
async fn record_denied_later_operation_rolls_back_earlier_mutation_in_same_batch() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    seed(&f, "host-b").await;
    policy(&f, "host-b", None, &[]).await;
    let binding = action(&f, "update", None);
    let batch = PluginDataRequest {
        idempotency_key: None,
        operations: vec![
            PluginDataOperation::Update {
                target: owned_target(),
                filters: vec![eq("conversation", PluginDataValue::String("host-a".into()))],
                values: BTreeMap::from([(
                    "provider".into(),
                    PluginDataValue::String("must-rollback".into()),
                )]),
            },
            PluginDataOperation::Upsert {
                target: owned_target(),
                identity: BTreeMap::from([(
                    "conversation".into(),
                    PluginDataValue::String("host-b".into()),
                )]),
                values: BTreeMap::from([(
                    "provider".into(),
                    PluginDataValue::String("blocked".into()),
                )]),
            },
        ],
    };
    assert_eq!(
        f.store.execute(&binding, &batch).await.unwrap_err().kind,
        PluginDataErrorKind::PermissionDenied
    );
    let value: String = sqlx::query_scalar(
        "select provider from plg_acme_session_affinity where conversation='host-a'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(value, "old");
}
