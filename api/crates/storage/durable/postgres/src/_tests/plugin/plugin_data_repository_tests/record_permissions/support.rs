use super::*;

pub(super) struct Fixture {
    pub store: PgControlPlaneStore,
    pub pool: PgPool,
    pub actor: ActorContext,
    pub workspace: Uuid,
    pub other_workspace: Uuid,
    pub role: Uuid,
    pub department_role: Uuid,
    pub foreign_role: Uuid,
    pub binding: PluginDataBinding,
}

pub(super) async fn fixture() -> Fixture {
    let (store, pool) = store().await;
    let tenant: Uuid = sqlx::query_scalar("select id from tenants where code='root-tenant'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let workspace = Uuid::now_v7();
    let other_workspace = Uuid::now_v7();
    for (id, name) in [
        (workspace, "Record caller"),
        (other_workspace, "Other caller"),
    ] {
        sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,$3)")
            .bind(id)
            .bind(tenant)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    let user = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,status) values($1,$2,$3,'hash','Record user','Record user','active')")
        .bind(user).bind(format!("record-{user}")).bind(format!("{user}@example.test")).execute(&pool).await.unwrap();
    for id in [workspace, other_workspace] {
        sqlx::query("insert into workspace_memberships(id,workspace_id,user_id) values($1,$2,$3)")
            .bind(Uuid::now_v7())
            .bind(id)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
    }
    let role = Uuid::now_v7();
    let department_role = Uuid::now_v7();
    let foreign_role = Uuid::now_v7();
    for (id, scope, code) in [
        (role, workspace, "operator"),
        (department_role, workspace, "department"),
        (foreign_role, other_workspace, "operator"),
    ] {
        sqlx::query("insert into roles(id,scope_id,scope_kind,workspace_id,code,name) values($1,$2,'workspace',$2,$3,$3)")
            .bind(id).bind(scope).bind(code).execute(&pool).await.unwrap();
    }
    for id in [role, department_role, foreign_role] {
        // Binding scope belongs to its role, including the role in other_workspace.
        let bound = sqlx::query("insert into user_role_bindings(id,user_id,role_id,scope_id) select $1,$2,id,scope_id from roles where id=$3")
            .bind(Uuid::now_v7())
            .bind(user)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(bound.rows_affected(), 1, "fixture role must exist");
    }
    let operations = vec![
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
    ];
    store
        .apply_managed_schema(&plan("record-v1", operations))
        .await
        .unwrap();
    let actor = ActorContext::scoped(user, workspace, "operator", Vec::<String>::new());
    let mut data_binding = binding(domain::SYSTEM_SCOPE_ID);
    data_binding.actor_id = Some(user.to_string());
    data_binding.record_permissions = Some(PluginDataRecordPermissionContext {
        caller_workspace_id: workspace.to_string(),
        actor_user_id: user.to_string(),
        active_role_code: "operator".into(),
        is_root: false,
        resources: vec![PluginDataRecordResource {
            resource_code: "hosts".into(),
            collection_code: "affinity".into(),
            id_field: "conversation".into(),
            action_code: "view".into(),
            record_id: None,
        }],
    });
    Fixture {
        store,
        pool,
        actor,
        workspace,
        other_workspace,
        role,
        department_role,
        foreign_role,
        binding: data_binding,
    }
}

pub(super) fn key(f: &Fixture, id: &str) -> RecordPermissionKey {
    RecordPermissionKey {
        workspace_id: f.workspace,
        record_id: id.into(),
        resource: RecordPermissionResourceKey {
            publisher_namespace: "acme".into(),
            plugin_code: "session".into(),
            resource_code: "hosts".into(),
        },
    }
}
pub(super) fn input(
    role: Option<Uuid>,
    actions: &[&str],
    revision: Option<i64>,
) -> ReplaceRecordPermissions {
    ReplaceRecordPermissions {
        mode: RecordPermissionMode::Restricted,
        grants: role
            .into_iter()
            .map(|role_id| RecordPermissionGrant {
                role_id,
                actions: actions.iter().map(|action| action.to_string()).collect(),
            })
            .collect(),
        expected_revision: revision,
    }
}
pub(super) async fn policy(
    f: &Fixture,
    id: &str,
    role: Option<Uuid>,
    actions: &[&str],
) -> RecordPermissionPolicy {
    f.store
        .replace_record_permissions(&key(f, id), &input(role, actions, None), f.actor.user_id)
        .await
        .unwrap()
}
pub(super) async fn seed(f: &Fixture, id: &str) {
    let mut legacy = f.binding.clone();
    legacy.record_permissions = None;
    f.store
        .execute(
            &legacy,
            &request(PluginDataOperation::Insert {
                target: owned_target(),
                values: values(id, "old"),
            }),
        )
        .await
        .unwrap();
}
pub(super) fn values(id: &str, provider: &str) -> BTreeMap<String, PluginDataValue> {
    BTreeMap::from([
        ("conversation".into(), PluginDataValue::String(id.into())),
        ("provider".into(), PluginDataValue::String(provider.into())),
    ])
}
pub(super) fn request(operation: PluginDataOperation) -> PluginDataRequest {
    PluginDataRequest {
        idempotency_key: None,
        operations: vec![operation],
    }
}
pub(super) fn action(f: &Fixture, action: &str, id: Option<&str>) -> PluginDataBinding {
    let mut binding = f.binding.clone();
    let resource = &mut binding.record_permissions.as_mut().unwrap().resources[0];
    resource.action_code = action.into();
    resource.record_id = id.map(str::to_string);
    binding
}
pub(super) fn count() -> PluginDataOperation {
    PluginDataOperation::Count {
        target: owned_target(),
        filters: vec![],
    }
}
pub(super) async fn count_with(f: &Fixture, binding: &PluginDataBinding) -> u64 {
    match &f
        .store
        .execute(binding, &request(count()))
        .await
        .unwrap()
        .results[0]
    {
        PluginDataOperationResult::Count { count } => *count,
        _ => panic!("count response"),
    }
}
pub(super) async fn department(f: &Fixture) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into departments(id,scope_id,tree_partition_id,sibling_rank,name) values($1,$2,$2,'a0','Record department')").bind(id).bind(f.workspace).execute(&f.pool).await.unwrap();
    sqlx::query("insert into department_role_bindings(id,scope_id,department_id,role_id) values($1,$2,$3,$4)").bind(Uuid::now_v7()).bind(f.workspace).bind(id).bind(f.department_role).execute(&f.pool).await.unwrap();
    sqlx::query("insert into user_department_bindings(id,scope_id,user_id,department_id,is_primary) values($1,$2,$3,$4,true)").bind(Uuid::now_v7()).bind(f.workspace).bind(f.actor.user_id).bind(id).execute(&f.pool).await.unwrap();
    id
}
