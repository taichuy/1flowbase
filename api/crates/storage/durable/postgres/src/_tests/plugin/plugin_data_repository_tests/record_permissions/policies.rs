use super::*;

#[tokio::test]
async fn record_policy_workspace_role_validation_revision_and_atomic_failure() {
    let f = fixture().await;
    assert!(f
        .store
        .get_record_permissions(&key(&f, "host-a"))
        .await
        .unwrap()
        .is_none());
    let first = policy(&f, "host-a", Some(f.role), &["view", "open"]).await;
    assert_eq!(first.revision, 1);
    let roles = f
        .store
        .list_record_permission_roles(f.workspace)
        .await
        .unwrap();
    assert!(roles.iter().any(|role| role.role_id == f.role));
    assert!(!roles.iter().any(|role| role.role_id == f.foreign_role));
    let invalid = input(Some(f.foreign_role), &["open"], Some(1));
    assert!(f
        .store
        .replace_record_permissions(&key(&f, "host-a"), &invalid, f.actor.user_id)
        .await
        .is_err());
    assert_eq!(
        f.store
            .get_record_permissions(&key(&f, "host-a"))
            .await
            .unwrap(),
        Some(first.clone())
    );
    assert!(f
        .store
        .replace_record_permissions(
            &key(&f, "host-a"),
            &input(Some(f.role), &["open"], Some(0)),
            f.actor.user_id
        )
        .await
        .is_err());
    // Controlled late storage failure: header/grant deletion happened before this grant trigger fails.
    sqlx::query("create function fixture_reject_record_grant() returns trigger language plpgsql as $$ begin if 'delete'=any(new.actions) then raise exception 'fixture write failure'; end if; return new; end $$")
        .execute(&f.pool).await.unwrap();
    sqlx::query("create trigger fixture_reject_grant before insert on plugin_record_permission_grants for each row execute function fixture_reject_record_grant()")
        .execute(&f.pool).await.unwrap();
    assert!(f
        .store
        .replace_record_permissions(
            &key(&f, "host-a"),
            &input(Some(f.role), &["delete"], Some(1)),
            f.actor.user_id
        )
        .await
        .is_err());
    assert_eq!(
        f.store
            .get_record_permissions(&key(&f, "host-a"))
            .await
            .unwrap(),
        Some(first)
    );
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where event_code='plugin_record_permission.replaced'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        audits, 1,
        "failed writes must roll back policy and audit together"
    );
    let inherited = ReplaceRecordPermissions {
        mode: RecordPermissionMode::Inherit,
        grants: vec![],
        expected_revision: Some(1),
    };
    let restored = f
        .store
        .replace_record_permissions(&key(&f, "host-a"), &inherited, f.actor.user_id)
        .await
        .unwrap();
    assert_eq!(restored.revision, 2);
    assert!(restored.grants.is_empty());
    let mut foreign_key = key(&f, "host-a");
    foreign_key.workspace_id = f.other_workspace;
    assert!(f
        .store
        .get_record_permissions(&foreign_key)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn concurrent_initial_policy_writers_have_one_revision_winner() {
    let f = fixture().await;
    let key = key(&f, "host-a");
    let one = input(Some(f.role), &["view"], Some(0));
    let two = input(None, &[], Some(0));
    let (left, right) = tokio::join!(
        f.store
            .replace_record_permissions(&key, &one, f.actor.user_id),
        f.store
            .replace_record_permissions(&key, &two, f.actor.user_id)
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "advisory lock must serialize the no-header case"
    );
    assert_eq!(
        f.store
            .get_record_permissions(&key)
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    let headers: i64 = sqlx::query_scalar("select count(*) from plugin_record_permission_policies")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(headers, 1);
}

#[tokio::test]
async fn durable_effective_roles_include_departments_and_revoke_without_rebinding_or_code_aliasing()
{
    let f = fixture().await;
    seed(&f, "host-a").await;
    policy(&f, "host-a", Some(f.department_role), &["view"]).await;
    assert_eq!(
        count_with(&f, &f.binding).await,
        0,
        "inactive directly assigned role must not grant"
    );
    department(&f).await;
    assert_eq!(count_with(&f, &f.binding).await, 1);
    assert!(f
        .store
        .record_permission_effective_role_ids(&f.actor)
        .await
        .unwrap()
        .contains(&f.department_role));
    sqlx::query("delete from department_role_bindings where scope_id=$1 and role_id=$2")
        .bind(f.workspace)
        .bind(f.department_role)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        count_with(&f, &f.binding).await,
        0,
        "same captured context must re-read durable role grants"
    );
    let ids = f
        .store
        .record_permission_effective_role_ids(&f.actor)
        .await
        .unwrap();
    assert!(!ids.contains(&f.department_role));
    assert!(!ids.contains(&f.foreign_role));
    let mut active = f.binding.clone();
    active.record_permissions.as_mut().unwrap().active_role_code = "department".into();
    assert_eq!(
        count_with(&f, &active).await,
        1,
        "the selected directly assigned role is independently selectable"
    );
    sqlx::query("delete from user_role_bindings where user_id=$1 and role_id=$2")
        .bind(f.actor.user_id)
        .bind(f.department_role)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        count_with(&f, &active).await,
        0,
        "revoking the selected role invalidates its durable grant"
    );
}

#[tokio::test]
async fn plugin_version_reconcile_preserves_existing_record_policy_and_data() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    let saved = policy(&f, "host-a", None, &[]).await;
    let mut desired = plan(
        "record-v2",
        vec![
            ManagedSchemaOperation::EnsureOwnedCollection {
                logical_collection: "affinity".into(),
                physical_table: "plg_acme_session_affinity".into(),
            },
            ManagedSchemaOperation::EnsureOwnedField {
                logical_collection: "affinity".into(),
                logical_field: "conversation".into(),
                physical_table: "plg_acme_session_affinity".into(),
                physical_column: "conversation".into(),
                field_type: ManagedSchemaFieldType::String,
                nullable: false,
            },
            ManagedSchemaOperation::EnsureOwnedField {
                logical_collection: "affinity".into(),
                logical_field: "provider".into(),
                physical_table: "plg_acme_session_affinity".into(),
                physical_column: "provider".into(),
                field_type: ManagedSchemaFieldType::String,
                nullable: false,
            },
        ],
    );
    desired.owner_version = "2.0.0".into();
    f.store.apply_managed_schema(&desired).await.unwrap();
    let mut upgraded = f.binding.clone();
    upgraded.plugin_version = "2.0.0".into();
    assert_eq!(count_with(&f, &upgraded).await, 0);
    assert_eq!(
        f.store
            .get_record_permissions(&key(&f, "host-a"))
            .await
            .unwrap(),
        Some(saved)
    );
    upgraded.record_permissions.as_mut().unwrap().is_root = true;
    assert_eq!(
        count_with(&f, &upgraded).await,
        1,
        "upgrade retained the physical record"
    );
}

#[tokio::test]
async fn policy_resource_identity_and_role_deletion_remain_fail_closed() {
    let f = fixture().await;
    seed(&f, "host-a").await;
    for component in ["publisher", "plugin", "resource"] {
        let mut other = key(&f, "host-a");
        match component {
            "publisher" => other.resource.publisher_namespace = "other".into(),
            "plugin" => other.resource.plugin_code = "other".into(),
            _ => other.resource.resource_code = "other".into(),
        };
        f.store
            .replace_record_permissions(&other, &input(None, &[], Some(0)), f.actor.user_id)
            .await
            .unwrap();
    }
    assert_eq!(
        count_with(&f, &f.binding).await,
        1,
        "unrelated resource policy must not leak into this owner"
    );
    policy(&f, "host-a", Some(f.role), &["view"]).await;
    let id:Uuid=sqlx::query_scalar("select id from plugin_record_permission_policies where publisher_namespace='acme' and plugin_code='session' and resource_code='hosts' and record_id='host-a'").fetch_one(&f.pool).await.unwrap();
    assert!(sqlx::query("insert into plugin_record_permission_grants(policy_id,role_id,actions) values($1,$2,array['view'])").bind(id).bind(f.foreign_role).execute(&f.pool).await.is_err(),"durable trigger rejects alternate cross-workspace writer");
    assert_eq!(count_with(&f, &f.binding).await, 1);
    sqlx::query("delete from roles where id=$1")
        .bind(f.role)
        .execute(&f.pool)
        .await
        .unwrap();
    let policy = f
        .store
        .get_record_permissions(&key(&f, "host-a"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(policy.mode, RecordPermissionMode::Restricted);
    assert!(policy.grants.is_empty());
    assert_eq!(
        count_with(&f, &f.binding).await,
        0,
        "deleting the last role must preserve an empty restricted header"
    );
}
