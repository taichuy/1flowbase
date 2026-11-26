use control_plane_contracts::ports::{AuthRepository, ModelDefinitionRepository, RoleRepository};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

#[tokio::test]
async fn role_queries_respect_requested_workspace_instead_of_first_workspace() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let first_workspace_id = Uuid::now_v7();
    let second_workspace_id = Uuid::now_v7();
    let first_workspace_name = format!("Workspace A {}", first_workspace_id.simple());
    let second_workspace_name = format!("Workspace B {}", second_workspace_id.simple());

    sqlx::query(
        r#"
        insert into workspaces (id, tenant_id, name, created_by, updated_by)
        values ($1, $2, $3, null, null),
               ($4, $2, $5, null, null)
        "#,
    )
    .bind(first_workspace_id)
    .bind(tenant_id)
    .bind(&first_workspace_name)
    .bind(second_workspace_id)
    .bind(&second_workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable
        )
        values ($1, $2, 'workspace', $2, 'reviewer', 'Reviewer', '', false, true)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(second_workspace_id)
    .execute(store.pool())
    .await
    .unwrap();

    let roles = RoleRepository::list_roles(&store, second_workspace_id)
        .await
        .unwrap();

    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].code, "reviewer");
}

#[tokio::test]
async fn role_and_model_actor_loaders_respect_switched_request_workspace() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let user_id = Uuid::now_v7();
    let workspace_a = Uuid::now_v7();
    let workspace_b = Uuid::now_v7();

    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status)
         values ($1, 'workspace-switch-member', 'workspace-switch-member@example.test',
                 'unused', 'Workspace Switch Member', 'Workspace Switch Member', 'active')",
    )
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_at)
         values ($1, $3, 'Workspace A', now() - interval '1 day'),
                ($2, $3, 'Workspace B', now())",
    )
    .bind(workspace_a)
    .bind(workspace_b)
    .bind(tenant_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id, created_at)
         values ($1, $3, $5, now() - interval '1 day'), ($2, $4, $5, now())",
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::now_v7())
    .bind(workspace_a)
    .bind(workspace_b)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    assert_eq!(
        AuthRepository::default_scope_for_user(&store, user_id)
            .await
            .unwrap()
            .workspace_id,
        workspace_a,
    );
    let actor_in_b =
        AuthRepository::load_actor_context(&store, user_id, tenant_id, workspace_b, None)
            .await
            .unwrap();
    let scoped_store = store.for_actor(actor_in_b);
    let role_actor = RoleRepository::load_actor_context_for_user(&scoped_store, user_id).await;
    let model_actor =
        ModelDefinitionRepository::load_actor_context_for_user(&scoped_store, user_id).await;
    assert_eq!(role_actor.unwrap().current_workspace_id, workspace_b);
    assert_eq!(model_actor.unwrap().current_workspace_id, workspace_b);
}
