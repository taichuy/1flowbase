use super::*;
use control_plane_contracts::ports::{
    mcp_oauth::McpOAuthRepository, FrontstagePageRepository, ModelDefinitionRepository,
};
use control_plane_contracts::ControlPlaneContractError;

async fn department(
    store: &PgControlPlaneStore,
    scope: Uuid,
    parent: Option<Uuid>,
    rank: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into departments(id,scope_id,tree_partition_id,parent_id,sibling_rank,name) values($1,$2,$2,$3,$4,'Department')")
        .bind(id).bind(scope).bind(parent).bind(rank).execute(store.pool()).await.unwrap();
    id
}

async fn department_role(store: &PgControlPlaneStore, scope: Uuid, department: Uuid, role: Uuid) {
    sqlx::query("insert into department_role_bindings(id,scope_id,department_id,role_id) values($1,$2,$3,$4)")
        .bind(Uuid::now_v7()).bind(scope).bind(department).bind(role).execute(store.pool()).await.unwrap();
}

async fn join_department(
    store: &PgControlPlaneStore,
    scope: Uuid,
    user: Uuid,
    department: Uuid,
    primary: bool,
) {
    sqlx::query("insert into user_department_bindings(id,scope_id,user_id,department_id,is_primary) values($1,$2,$3,$4,$5)")
        .bind(Uuid::now_v7()).bind(scope).bind(user).bind(department).bind(primary).execute(store.pool()).await.unwrap();
}

#[tokio::test]
async fn department_roles_are_selectable_isolated_and_revoked_after_the_last_source() {
    let schema = isolated_database().await;
    let pool = schema.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = root_tenant_id(&store).await;
    let scope = insert_workspace(&store, tenant, "Department permissions").await;
    let other_scope = insert_workspace(&store, tenant, "Other department scope").await;
    let user = Uuid::now_v7();
    insert_user(&store, user, "department-user", "member").await;
    insert_membership(&store, scope, user).await;
    insert_membership(&store, other_scope, user).await;
    let member = insert_workspace_role(&store, scope, "member").await;
    let reader = insert_workspace_role(&store, scope, "reader").await;
    let ancestor_role = insert_workspace_role(&store, scope, "ancestor").await;
    let foreign_role = insert_workspace_role(&store, other_scope, "foreign").await;
    bind_role(&store, user, member).await;
    grant_permission(&store, scope, member, "department.test.base").await;
    grant_permission(&store, scope, reader, "department.test.read").await;
    grant_permission(&store, scope, ancestor_role, "department.test.ancestor").await;
    grant_permission(&store, other_scope, foreign_role, "department.test.foreign").await;
    let parent = department(&store, scope, None, "a0").await;
    let first = department(&store, scope, Some(parent), "a0").await;
    let second = department(&store, scope, None, "a1").await;
    let foreign = department(&store, other_scope, None, "a0").await;
    department_role(&store, scope, parent, ancestor_role).await;
    department_role(&store, scope, first, reader).await;
    department_role(&store, scope, second, reader).await;
    department_role(&store, other_scope, foreign, foreign_role).await;

    // Controlled negative: an inactive direct role alone must not grant its permission.
    let before = store
        .load_actor_context(user, tenant, scope, Some("member"))
        .await
        .unwrap();
    assert!(!before.has_permission("department.test.read"));
    join_department(&store, scope, user, first, true).await;
    join_department(&store, scope, user, second, false).await;
    join_department(&store, other_scope, user, foreign, true).await;
    let actor = store
        .load_actor_context(user, tenant, scope, Some("member"))
        .await
        .unwrap();
    assert_eq!(actor.effective_display_role, "member");
    assert!(actor.has_permission("department.test.base"));
    assert!(!actor.has_permission("department.test.read"));
    assert!(!actor.has_permission("department.test.ancestor"));
    assert!(!actor.has_permission("department.test.foreign"));
    assert!(!actor.is_root);
    let bound = store
        .load_actor_context_for_bound_role(user, tenant, scope, "reader")
        .await
        .unwrap();
    assert!(bound.has_permission("department.test.read"));
    assert!(!bound.has_permission("department.test.base"));
    assert_eq!(
        store
            .effective_role_ids(user, scope, "reader")
            .await
            .unwrap(),
        vec![reader]
    );
    assert!(store
        .effective_role_ids(user, scope, "foreign")
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .effective_role_ids(user, scope, "root")
        .await
        .unwrap()
        .is_empty());
    let root_id =
        match sqlx::query_scalar("select id from roles where scope_kind='system' and code='root'")
            .fetch_optional(store.pool())
            .await
            .unwrap()
        {
            Some(id) => id,
            None => insert_root_role(&store).await,
        };
    let denied = sqlx::query("insert into department_role_bindings(id,scope_id,department_id,role_id) values($1,$2,$3,$4)")
        .bind(Uuid::now_v7()).bind(scope).bind(first).bind(root_id).execute(store.pool()).await.unwrap_err();
    assert_eq!(
        denied.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    let in_use = store
        .delete_team_role(user, scope, "reader")
        .await
        .unwrap_err();
    assert!(matches!(
        in_use.downcast_ref::<ControlPlaneContractError>(),
        Some(ControlPlaneContractError::Conflict("role_in_use"))
    ));
    assert!(store
        .load_actor_context_for_bound_role(user, tenant, scope, "ancestor")
        .await
        .is_err());

    let console = store
        .load_console_policy_for_bound_role(user, scope, "member")
        .await
        .unwrap();
    assert_eq!(console.len(), 1);
    assert_eq!(console[0].role_id(), member);
    assert!(store
        .load_console_policy_for_bound_role(user, scope, "missing")
        .await
        .unwrap()
        .is_empty());
    for role in [member, reader, ancestor_role, foreign_role] {
        sqlx::query("insert into role_data_policies(id,role_id,can_view,default_view_scope) values($1,$2,true,'scope_all')")
            .bind(Uuid::now_v7()).bind(role).execute(store.pool()).await.unwrap();
    }
    let data = store
        .list_actor_role_data_policies(user, scope, "member", Uuid::now_v7())
        .await
        .unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].0.role_id, member);

    for role in [member, reader, ancestor_role] {
        sqlx::query("insert into frontstage_page_visibility_rules(id,workspace_id,role_id,visibility) values($1,$2,$3,'visible')")
            .bind(Uuid::now_v7()).bind(scope).bind(role).execute(store.pool()).await.unwrap();
    }
    let visibility = store
        .list_frontstage_page_visibility_rules_for_actor_roles(user, scope, "member")
        .await
        .unwrap();
    assert_eq!(visibility.len(), 1);
    assert_eq!(visibility[0].role_id, member);

    let stamp = store.oauth_authorization_stamp(user).await.unwrap();
    assert!(stamp["roles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|role| role["id"] == serde_json::json!(reader)));

    // Losing all department grants revokes the selected role immediately.
    sqlx::query("delete from department_role_bindings where scope_id=$1 and role_id=$2")
        .bind(scope)
        .bind(reader)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .effective_role_ids(user, scope, "reader")
        .await
        .unwrap()
        .is_empty());
    assert_ne!(store.oauth_authorization_stamp(user).await.unwrap(), stamp);
    department_role(&store, scope, first, reader).await;
    department_role(&store, scope, second, reader).await;

    // Dropping the direct binding still permits the department-only role to be selected.
    sqlx::query("delete from user_role_bindings where user_id=$1")
        .bind(user)
        .execute(store.pool())
        .await
        .unwrap();
    let loaded = store.find_user_by_id(user).await.unwrap().unwrap();
    assert!(loaded.roles.is_empty());
    assert_eq!(
        store
            .resolve_actor_role(user, tenant, scope, None)
            .await
            .unwrap()
            .effective_display_role,
        "reader"
    );
    assert_eq!(
        store
            .resolve_loaded_user_role(&loaded, tenant, scope, Some("reader"))
            .await
            .unwrap()
            .effective_display_role,
        "reader"
    );
    for actor in [
        store
            .load_actor_context(user, tenant, scope, Some("reader"))
            .await
            .unwrap(),
        store
            .load_actor_context_for_bound_role(user, tenant, scope, "reader")
            .await
            .unwrap(),
    ] {
        assert_eq!(actor.effective_display_role, "reader");
        assert!(actor.has_permission("department.test.read"));
        assert!(!actor.has_permission("department.test.base"));
    }

    sqlx::query("delete from user_department_bindings where user_id=$1 and department_id=$2")
        .bind(user)
        .bind(second)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .effective_role_ids(user, scope, "reader")
            .await
            .unwrap(),
        vec![reader]
    );
    bind_role(&store, user, reader).await;
    assert_eq!(
        store
            .effective_role_ids(user, scope, "reader")
            .await
            .unwrap(),
        vec![reader]
    );
    sqlx::query("delete from user_department_bindings where user_id=$1 and scope_id=$2")
        .bind(user)
        .bind(scope)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .effective_role_ids(user, scope, "reader")
            .await
            .unwrap(),
        vec![reader]
    );
    sqlx::query("delete from user_role_bindings where user_id=$1")
        .bind(user)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .effective_role_ids(user, scope, "reader")
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .load_actor_context_for_bound_role(user, tenant, scope, "reader")
        .await
        .is_err());
}
