use super::*;

#[tokio::test]
async fn create_only_inserts_genuinely_new_record_and_cannot_read_or_mutate_existing_rows() {
    let f = fixture().await;
    let create = action(&f, "create", None);
    let insert = request(PluginDataOperation::Insert {
        target: owned_target(),
        values: values("new-host", "new"),
    });
    assert_eq!(
        f.store.execute(&create, &insert).await.unwrap().results,
        vec![PluginDataOperationResult::Mutation { affected: 1 }]
    );
    assert_eq!(
        f.store.execute(&create, &insert).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    policy(
        &f,
        "new-host",
        Some(f.role),
        &["view", "update", "open", "delete"],
    )
    .await;
    for operation in [
        count(),
        PluginDataOperation::FindOne {
            target: owned_target(),
            fields: vec!["conversation".into()],
            filters: vec![],
        },
        PluginDataOperation::Find {
            target: owned_target(),
            fields: vec!["conversation".into()],
            filters: vec![],
            order: vec![],
            page: PluginDataPage::default(),
        },
        PluginDataOperation::Update {
            target: owned_target(),
            filters: vec![eq(
                "conversation",
                PluginDataValue::String("new-host".into()),
            )],
            values: BTreeMap::from([(
                "provider".into(),
                PluginDataValue::String("changed".into()),
            )]),
        },
        PluginDataOperation::Delete {
            target: owned_target(),
            filters: vec![eq(
                "conversation",
                PluginDataValue::String("new-host".into()),
            )],
        },
        PluginDataOperation::Upsert {
            target: owned_target(),
            identity: BTreeMap::from([(
                "conversation".into(),
                PluginDataValue::String("another-new".into()),
            )]),
            values: BTreeMap::from([(
                "provider".into(),
                PluginDataValue::String("changed".into()),
            )]),
        },
    ] {
        assert_eq!(
            f.store
                .execute(&create, &request(operation))
                .await
                .unwrap_err()
                .kind,
            PluginDataErrorKind::PermissionDenied
        );
    }
    // A pre-existing restricted identity is not a new record merely because its data was deleted.
    policy(&f, "deleted-host", Some(f.role), &["update"]).await;
    let resurrect = request(PluginDataOperation::Insert {
        target: owned_target(),
        values: values("deleted-host", "new"),
    });
    assert_eq!(
        f.store.execute(&create, &resurrect).await.unwrap_err().kind,
        PluginDataErrorKind::PermissionDenied
    );
    assert!(f
        .store
        .replace_record_permissions(
            &key(&f, "new-host"),
            &input(Some(f.role), &["create"], None),
            f.actor.user_id
        )
        .await
        .is_err());
    assert_eq!(count_with(&f, &f.binding).await, 1);
    let blank = action(&f, "", None);
    assert_eq!(
        f.store
            .execute(&blank, &request(count()))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::PermissionDenied
    );
}

#[tokio::test]
async fn concurrent_creates_with_distinct_row_uuids_have_one_stable_record_identity() {
    let f = fixture().await;
    let create = action(&f, "create", None);
    let insert = request(PluginDataOperation::Insert {
        target: owned_target(),
        values: values("new-host", "new"),
    });
    let (one, two) = tokio::join!(
        f.store.execute(&create, &insert),
        f.store.execute(&create, &insert)
    );
    assert_ne!(one.is_ok(), two.is_ok());
    assert_eq!(count_with(&f, &f.binding).await, 1);
}

#[tokio::test]
async fn receipts_never_replay_rows_after_policy_change_or_effective_role_revocation() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    let mut read = request(count());
    read.idempotency_key = Some("acl-list".into());
    let first = f.store.execute(&f.binding, &read).await.unwrap();
    assert!(!first.replayed);
    assert!(f.store.execute(&f.binding, &read).await.unwrap().replayed);
    policy(&f, "host-a", None, &[]).await;
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    assert_eq!(count_with(&f, &f.binding).await, 0);
    policy(&f, "host-a", Some(f.department_role), &["view"]).await;
    department(&f).await;
    read.idempotency_key = Some("department-list".into());
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap().results,
        vec![PluginDataOperationResult::Count { count: 1 }]
    );
    sqlx::query("delete from department_role_bindings where scope_id=$1 and role_id=$2")
        .bind(f.workspace)
        .bind(f.department_role)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    assert_eq!(count_with(&f, &f.binding).await, 0);
    let mut other = f.binding.clone();
    other
        .record_permissions
        .as_mut()
        .unwrap()
        .caller_workspace_id = f.other_workspace.to_string();
    assert_eq!(
        f.store.execute(&other, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict,
        "receipt identity must include caller workspace even on shared physical scope"
    );
}

#[tokio::test]
async fn receipt_cannot_cross_record_action_or_root_identity_and_failed_batch_has_no_receipt() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    let mut read = request(count());
    read.idempotency_key = Some("caller-list".into());
    f.store.execute(&f.binding, &read).await.unwrap();
    for changed in [action(&f, "open", None), action(&f, "view", Some("host-a"))] {
        assert_eq!(
            f.store.execute(&changed, &read).await.unwrap_err().kind,
            PluginDataErrorKind::Conflict
        );
    }
    let mut root = f.binding.clone();
    root.record_permissions.as_mut().unwrap().is_root = true;
    assert_eq!(
        f.store.execute(&root, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    policy(&f, "host-a", None, &[]).await;
    let mut mutation = request(PluginDataOperation::Upsert {
        target: owned_target(),
        identity: BTreeMap::from([(
            "conversation".into(),
            PluginDataValue::String("host-a".into()),
        )]),
        values: BTreeMap::from([("provider".into(), PluginDataValue::String("denied".into()))]),
    });
    mutation.idempotency_key = Some("denied-write".into());
    assert_eq!(
        f.store
            .execute(&action(&f, "update", None), &mutation)
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::PermissionDenied
    );
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from plugin_data_idempotency_receipts where idempotency_key='denied-write'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(receipts, 0);
}
