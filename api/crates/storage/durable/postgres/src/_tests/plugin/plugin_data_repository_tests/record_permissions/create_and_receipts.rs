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
async fn restoring_record_permissions_never_revives_an_obsolete_authorization_receipt() {
    use control_plane_contracts::system_backup::selective::{
        SelectiveBackupRepository, SelectiveBackupSelection,
    };
    use tokio::io::AsyncReadExt;

    let f = fixture().await;
    seed(&f, "host-a").await;
    policy(&f, "host-a", Some(f.role), &["view"]).await;
    let mut read = request(count());
    read.idempotency_key = Some("before-permission-restore".into());
    assert!(!f.store.execute(&f.binding, &read).await.unwrap().replayed);
    assert!(f.store.execute(&f.binding, &read).await.unwrap().replayed);
    let revision_sql = "select revision from plugin_record_permission_scope_revisions where workspace_id=$1 and publisher_namespace='acme' and plugin_code='session'";
    let archived_revision: i64 = sqlx::query_scalar(revision_sql)
        .bind(f.workspace)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let backup = crate::system_backup::PgSelectiveBackupRepository::new(f.pool.clone());
    let source = backup
        .source(vec![SelectiveBackupSelection {
            feature_id: "system.roles".into(),
            structure: true,
            data: true,
        }])
        .await
        .unwrap();
    let (writer, mut reader) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(async move { source.write_to(Box::pin(writer)).await.unwrap() });
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    task.await.unwrap();

    policy(&f, "host-a", None, &[]).await;
    assert_eq!(count_with(&f, &f.binding).await, 0);
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    // Empty policy storage also exercises the real BEFORE grant-scope trigger:
    // restore must write the policy first, rather than suppressing its rejection.
    sqlx::query("delete from plugin_record_permission_policies where workspace_id=$1 and publisher_namespace='acme' and plugin_code='session' and resource_code='hosts' and record_id='host-a'")
        .bind(f.workspace).execute(&f.pool).await.unwrap();
    let target_revision: i64 = sqlx::query_scalar(revision_sql)
        .bind(f.workspace)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    backup
        .restore(
            Box::pin(std::io::Cursor::new(bytes.clone())),
            "source",
            "target",
            true,
        )
        .await
        .unwrap();
    let restored_revision: i64 = sqlx::query_scalar(revision_sql)
        .bind(f.workspace)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert!(restored_revision > archived_revision && restored_revision > target_revision);
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
    );
    read.idempotency_key = Some("after-permission-restore".into());
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap().results,
        vec![PluginDataOperationResult::Count { count: 1 }]
    );
    assert!(f.store.execute(&f.binding, &read).await.unwrap().replayed);
    // An empty target counter must not recreate the archived receipt generation.
    sqlx::query("delete from plugin_record_permission_scope_revisions where workspace_id=$1 and publisher_namespace='acme' and plugin_code='session'")
        .bind(f.workspace).execute(&f.pool).await.unwrap();
    backup
        .restore(
            Box::pin(std::io::Cursor::new(bytes)),
            "source",
            "target",
            true,
        )
        .await
        .unwrap();
    let inserted_revision: i64 = sqlx::query_scalar(revision_sql)
        .bind(f.workspace)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert!(inserted_revision > archived_revision);
    read.idempotency_key = Some("before-permission-restore".into());
    assert_eq!(
        f.store.execute(&f.binding, &read).await.unwrap_err().kind,
        PluginDataErrorKind::Conflict
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
